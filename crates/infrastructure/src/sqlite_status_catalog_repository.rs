//! Catálogo de status de chamado espelhado do Mastersys.
//!
//! Serve duas decisões da UI que antes eram chute:
//!
//! 1. **Rótulo e cor do selo de status.** Antes o frontend humanizava o slug à
//!    força (`pos_atendimento` → "pos atendimento") e não tinha cor nenhuma.
//!    A origem já tem "Pós Atendimento" e um hex por status.
//! 2. **Quais status vêm pré-marcados no filtro** (`default_filter`), que é
//!    também o sinal de item parado — ver [`MastersysTicketStatus::is_parked`].
//!
//! É dado de referência: pode ser apagado e reconstruído pela próxima
//! sincronização, e nada aqui é insumo do usuário.

use chrono::Utc;
use masterdesk_domain::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Um status de chamado como o Mastersys o cadastra.
///
/// Os nomes de campo espelham a resposta de `GET /api/ticket-statuses` para o
/// `Deserialize` sair de graça — o controller devolve as colunas do banco em
/// snake_case, sem envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MastersysTicketStatus {
    pub value: String,
    pub label: String,
    pub color: String,
    /// Vem pré-marcado no filtro de status da origem. É `false` exatamente para
    /// `finalizado`, `cancelado` e `pos_atendimento`.
    #[serde(default = "default_true")]
    pub default_filter: bool,
    /// Status terminal (`finalizado`, `cancelado`).
    #[serde(default)]
    pub is_final: bool,
    /// Congela a contagem de SLA. Opt-in do admin no cadastro, então costuma
    /// ser `false` mesmo em status parados.
    #[serde(default)]
    pub pauses_sla: bool,
    #[serde(default)]
    pub display_order: i64,
}

fn default_true() -> bool {
    true
}

impl MastersysTicketStatus {
    /// O item está **parado** na origem: em espera, concluído ou cancelado.
    ///
    /// ## Por que `!default_filter` entra na conta
    ///
    /// O sinal semanticamente correto seria `pauses_sla`, mas ele nasce
    /// desligado em todos os status e é opt-in do admin
    /// (`migrations.ts:4451` do Mastersys), então em instalação real vem
    /// `false` até para `pos_atendimento`. Já `default_filter` é posto em `0`
    /// deterministicamente para `finalizado`, `cancelado` e `pos_atendimento`
    /// (`migrations.ts:4472-4473`).
    ///
    /// Usar `default_filter` aqui estica sua semântica original ("vem
    /// pré-marcado no filtro") para "não é trabalho ativo". O desvio é
    /// assumido: é o único sinal determinístico que a origem oferece, e as
    /// duas leituras coincidem em todos os status semeados. Se um admin criar
    /// um status ativo fora do filtro padrão, ele será tratado como parado —
    /// consequência aceita, e visível porque o selo o marca como tal.
    pub fn is_parked(&self) -> bool {
        self.is_final || self.pauses_sla || !self.default_filter
    }
}

#[derive(Debug, sqlx::FromRow)]
struct StatusRow {
    value: String,
    label: String,
    color: String,
    default_filter: i64,
    is_final: i64,
    pauses_sla: i64,
    display_order: i64,
}

impl From<StatusRow> for MastersysTicketStatus {
    fn from(r: StatusRow) -> Self {
        Self {
            value: r.value,
            label: r.label,
            color: r.color,
            default_filter: r.default_filter != 0,
            is_final: r.is_final != 0,
            pauses_sla: r.pauses_sla != 0,
            display_order: r.display_order,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SqliteStatusCatalogRepository {
    pool: SqlitePool,
}

fn map_sqlx_err(_e: sqlx::Error) -> DomainError {
    // Nunca vazar detalhes de SQL para cima (CLAUDE §17/18).
    DomainError::Persistence
}

impl SqliteStatusCatalogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Catálogo inteiro, na ordem de exibição da origem — o filtro sai na mesma
    /// sequência que o atendente já conhece do suporte.
    pub async fn list(&self) -> DomainResult<Vec<MastersysTicketStatus>> {
        let rows: Vec<StatusRow> = sqlx::query_as(
            "SELECT value, label, color, default_filter, is_final, pauses_sla, display_order
             FROM mastersys_status_catalog
             ORDER BY display_order ASC, value ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Um status por slug. `None` quando o catálogo não conhece o valor — o que
    /// acontece de verdade: um status pode ser criado no Mastersys entre duas
    /// sincronizações, e a tarefa que o usa chega antes do catálogo.
    pub async fn get(&self, value: &str) -> DomainResult<Option<MastersysTicketStatus>> {
        let row: Option<StatusRow> = sqlx::query_as(
            "SELECT value, label, color, default_filter, is_final, pauses_sla, display_order
             FROM mastersys_status_catalog WHERE value = ?1",
        )
        .bind(value)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        Ok(row.map(Into::into))
    }

    /// Substitui o catálogo pelo que a origem acabou de informar.
    ///
    /// Roda em transação e apaga o que não veio: status excluído no Mastersys
    /// tem de sumir do filtro daqui também. Sem a transação, uma falha no meio
    /// deixaria o catálogo vazio e o quadro sem rótulo nem cor.
    ///
    /// Lista vazia é ignorada em vez de zerar a tabela — resposta vazia é mais
    /// provável ser problema de permissão ou proxy que um Mastersys de verdade
    /// sem nenhum status cadastrado.
    pub async fn replace_all(&self, statuses: &[MastersysTicketStatus]) -> DomainResult<()> {
        if statuses.is_empty() {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await.map_err(map_sqlx_err)?;

        sqlx::query("DELETE FROM mastersys_status_catalog")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_err)?;

        for s in statuses {
            sqlx::query(
                r#"
                INSERT INTO mastersys_status_catalog
                    (value, label, color, default_filter, is_final, pauses_sla,
                     display_order, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT(value) DO UPDATE SET
                    label          = excluded.label,
                    color          = excluded.color,
                    default_filter = excluded.default_filter,
                    is_final       = excluded.is_final,
                    pauses_sla     = excluded.pauses_sla,
                    display_order  = excluded.display_order,
                    updated_at     = excluded.updated_at
                "#,
            )
            .bind(&s.value)
            .bind(&s.label)
            .bind(&s.color)
            .bind(i64::from(s.default_filter))
            .bind(i64::from(s.is_final))
            .bind(i64::from(s.pauses_sla))
            .bind(s.display_order)
            .bind(&now)
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_err)?;
        }

        tx.commit().await.map_err(map_sqlx_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh() -> SqliteStatusCatalogRepository {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        SqliteStatusCatalogRepository::new(pool)
    }

    fn status(value: &str, default_filter: bool, is_final: bool) -> MastersysTicketStatus {
        MastersysTicketStatus {
            value: value.into(),
            label: value.into(),
            color: "#3b82f6".into(),
            default_filter,
            is_final,
            pauses_sla: false,
            display_order: 1,
        }
    }

    // -----------------------------------------------------------------------
    // is_parked — a regra que faz pós-atendimento parar de mentir que atrasou
    // -----------------------------------------------------------------------

    #[test]
    fn pos_atendimento_is_parked_via_default_filter() {
        // O caso do melhoria.png: pauses_sla desligado (opt-in do admin), mas
        // default_filter = 0 na origem.
        assert!(status("pos_atendimento", false, false).is_parked());
    }

    #[test]
    fn active_statuses_are_not_parked() {
        for value in ["novo", "em_atendimento", "aguardando_retorno_cliente"] {
            assert!(
                !status(value, true, false).is_parked(),
                "{value} está em andamento e não pode contar como parado"
            );
        }
    }

    #[test]
    fn terminal_statuses_are_parked() {
        assert!(status("finalizado", false, true).is_parked());
        assert!(status("cancelado", false, true).is_parked());
    }

    #[test]
    fn pauses_sla_alone_is_enough() {
        // Se o admin ligar pauses_sla num status que segue no filtro padrão,
        // ainda é item parado.
        let mut s = status("aguardando_atendimento_pausado", true, false);
        s.pauses_sla = true;
        assert!(s.is_parked());
    }

    // -----------------------------------------------------------------------
    // Persistência
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn replace_all_round_trips_and_orders_by_display_order() {
        let repo = fresh().await;
        let mut a = status("em_atendimento", true, false);
        a.display_order = 4;
        let mut b = status("novo", true, false);
        b.display_order = 1;

        repo.replace_all(&[a, b]).await.unwrap();

        let all = repo.list().await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].value, "novo", "display_order manda na ordem");
        assert_eq!(all[1].value, "em_atendimento");
    }

    #[tokio::test]
    async fn replace_all_drops_statuses_that_left_the_origin() {
        let repo = fresh().await;
        repo.replace_all(&[status("novo", true, false), status("extinto", true, false)])
            .await
            .unwrap();
        repo.replace_all(&[status("novo", true, false)])
            .await
            .unwrap();

        let all = repo.list().await.unwrap();
        assert_eq!(
            all.len(),
            1,
            "status excluído na origem tem de sair do filtro"
        );
        assert_eq!(all[0].value, "novo");
    }

    #[tokio::test]
    async fn empty_response_does_not_wipe_the_catalog() {
        // Resposta vazia é mais provável ser proxy/permissão que um Mastersys
        // sem status nenhum — zerar deixaria o quadro sem rótulo nem cor.
        let repo = fresh().await;
        repo.replace_all(&[status("novo", true, false)])
            .await
            .unwrap();
        repo.replace_all(&[]).await.unwrap();
        assert_eq!(repo.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn get_returns_none_for_unknown_slug() {
        let repo = fresh().await;
        assert!(repo.get("status_que_nao_existe").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn deserializes_the_origin_payload_shape() {
        // Formato real de GET /api/ticket-statuses: array cru, snake_case,
        // com campos extras que não nos interessam.
        // r##"…"## e não r#"…"#: a cor hex contém `"#`, que fecharia o raw
        // string no meio do JSON.
        let json = r##"[
            {"id":3,"value":"pos_atendimento","label":"Pós Atendimento","color":"#0ea5e9",
             "description":null,"flow_group":"support","is_final":false,"pauses_sla":false,
             "default_filter":false,"max_hours":null,"display_order":7,"is_active":true}
        ]"##;
        let parsed: Vec<MastersysTicketStatus> = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].label, "Pós Atendimento");
        assert!(!parsed[0].default_filter);
        assert!(parsed[0].is_parked());
    }
}
