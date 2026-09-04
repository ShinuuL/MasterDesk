//! Vínculo com sistemas de suporte externos (seção 5 e 10 do CLAUDE.md).
//!
//! O domínio conhece *que existe* uma origem externa, mas nada sobre HTTP,
//! endpoints ou autenticação — isso mora em `infrastructure`. Uma `Task` com
//! `external == None` é 100% local e continua funcionando sem qualquer
//! integração ("Tasks must not require a Mastersys ticket").

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::entities::Priority;
use crate::errors::{DomainError, DomainResult};

/// Sistema de suporte de origem. Enum fechado de propósito: cada variante
/// exige um adapter validado (ADR-006). Não existe variante "genérica" para
/// impedir que um provider não revisado se apresente como origem confiável.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalSystem {
    /// Mastersys Suporte (gerenciador de relatórios/chamados).
    Mastersys,
}

impl ExternalSystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExternalSystem::Mastersys => "mastersys",
        }
    }

    pub fn parse(s: &str) -> DomainResult<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mastersys" => Ok(ExternalSystem::Mastersys),
            other => Err(DomainError::Validation(format!(
                "unknown external system: {other}"
            ))),
        }
    }
}

/// Natureza do item no sistema de origem. No Mastersys uma tarefa pode estar
/// vinculada a um chamado (`ticket_id`) ou ser uma tarefa solta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExternalKind {
    /// Tarefa do quadro, sem chamado vinculado.
    Task,
    /// Chamado (ticket) de suporte.
    Ticket,
}

impl ExternalKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExternalKind::Task => "task",
            ExternalKind::Ticket => "ticket",
        }
    }

    pub fn parse(s: &str) -> DomainResult<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "task" => Ok(ExternalKind::Task),
            "ticket" => Ok(ExternalKind::Ticket),
            other => Err(DomainError::Validation(format!(
                "unknown external kind: {other}"
            ))),
        }
    }
}

/// Referência imutável ao item de origem. Guarda apenas o mínimo necessário
/// para (a) reconhecer o mesmo item em sincronizações futuras e (b) mostrar
/// contexto ao usuário — seção 18: "minimize stored data".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalRef {
    pub system: ExternalSystem,
    pub kind: ExternalKind,
    /// Id no sistema de origem. String porque cada sistema tem seu formato
    /// (o Mastersys usa inteiros; outro futuro pode usar UUID).
    pub external_id: String,
    /// Nome do cliente, quando o item tem um. Só exibição.
    pub client: Option<String>,
    /// Número do chamado vinculado, quando existe.
    pub ticket: Option<String>,
    /// Rótulo de status cru do sistema de origem (ex. `aguardando_retorno_cliente`).
    /// Mantido como texto porque o Mastersys permite status customizados —
    /// mapear para um enum fechado inventaria valores (Regra 1).
    pub status_label: Option<String>,
    /// A origem considera este item **parado**: em espera, concluído ou
    /// cancelado, em vez de em andamento.
    ///
    /// Existe porque prazo de item parado não significa urgência. Um chamado em
    /// pós-atendimento com prazo vencido não está atrasado — está aguardando. Sem
    /// esta distinção o quadro o marcava como atrasado e o lembrete disparava.
    ///
    /// Genérico de propósito: quem traduz status da origem para esta flag é o
    /// adapter, não o domínio (CLAUDE §4). Um provedor que não saiba distinguir
    /// deixa `false`, e o comportamento é o de antes.
    #[serde(default)]
    pub status_parked: bool,
    /// O usuário é o **analista responsável** pelo item na origem.
    ///
    /// Existe porque a fila de uma pessoa no suporte tem dois papéis
    /// diferentes, e o card precisa dizer qual: no Mastersys `assigned_to` é o
    /// analista responsável e `created_by` é o atendente — quem abriu/assumiu o
    /// chamado (verificado em `TicketRepository.ts`, filtro `attendantId`).
    ///
    /// Os dois podem ser verdadeiros ao mesmo tempo (abri e sou o responsável),
    /// e os dois podem ser falsos: uma tarefa atribuída a mim cujo chamado é de
    /// outra dupla não diz nada sobre papel, e nesse caso o app não afirma nada
    /// em vez de inventar um papel.
    #[serde(default)]
    pub role_analyst: bool,
    /// O usuário é o **atendente** do item na origem (`created_by`).
    #[serde(default)]
    pub role_attendant: bool,
}

impl ExternalRef {
    pub fn new(
        system: ExternalSystem,
        kind: ExternalKind,
        external_id: impl Into<String>,
    ) -> DomainResult<Self> {
        let external_id = external_id.into().trim().to_string();
        if external_id.is_empty() {
            return Err(DomainError::Validation(
                "external_id must not be empty".into(),
            ));
        }
        if external_id.chars().count() > 64 {
            return Err(DomainError::Validation(
                "external_id must be <= 64 chars".into(),
            ));
        }
        Ok(Self {
            system,
            kind,
            external_id,
            client: None,
            ticket: None,
            status_label: None,
            status_parked: false,
            role_analyst: false,
            role_attendant: false,
        })
    }

    pub fn with_client(mut self, client: Option<String>) -> Self {
        self.client = client.and_then(non_empty).map(|s| truncate(s, 200));
        self
    }

    pub fn with_ticket(mut self, ticket: Option<String>) -> Self {
        self.ticket = ticket.and_then(non_empty).map(|s| truncate(s, 64));
        self
    }

    pub fn with_status_label(mut self, status: Option<String>) -> Self {
        self.status_label = status.and_then(non_empty).map(|s| truncate(s, 64));
        self
    }

    pub fn with_status_parked(mut self, parked: bool) -> Self {
        self.status_parked = parked;
        self
    }

    /// Papéis do usuário neste item, como a origem os enxerga.
    ///
    /// Um só método para os dois porque eles são decididos juntos, na mesma
    /// resposta da origem — separar convidaria a gravar só metade e exibir
    /// "atendente" num item onde o analista simplesmente não foi consultado.
    pub fn with_roles(mut self, analyst: bool, attendant: bool) -> Self {
        self.role_analyst = analyst;
        self.role_attendant = attendant;
        self
    }

    /// Chave estável de deduplicação entre sincronizações.
    pub fn dedup_key(&self) -> String {
        format!("{}:{}", self.system.as_str(), self.external_id)
    }
}

/// Item de trabalho normalizado que veio de um sistema de suporte.
///
/// É o formato que `SupportSystemProvider` devolve: já traduzido para o
/// vocabulário do MasterNote (`Priority`, `DateTime<Utc>`, `completed`), de
/// modo que a camada de aplicação nunca veja JSON nem status crus do
/// Mastersys. A tradução acontece no adapter, que é o único lugar que conhece
/// o contrato real da API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalWorkItem {
    pub reference: ExternalRef,
    pub title: String,
    pub description: String,
    pub priority: Priority,
    /// Prazo já resolvido pelo adapter (no Mastersys: agendamento da tarefa,
    /// previsão do chamado ou agendamento do chamado — nessa ordem).
    pub deadline: Option<DateTime<Utc>>,
    /// True quando o item está finalizado/cancelado na origem.
    pub completed: bool,
    /// True quando o item deixou de pertencer ao usuário na origem e deve
    /// sair do MasterNote.
    pub removed: bool,
}

impl ExternalWorkItem {
    pub fn new(reference: ExternalRef, title: impl Into<String>) -> DomainResult<Self> {
        let title = title.into().trim().to_string();
        if title.is_empty() {
            return Err(DomainError::Validation(
                "external item title must not be empty".into(),
            ));
        }
        Ok(Self {
            reference,
            title: truncate(title, 200),
            description: String::new(),
            priority: Priority::default(),
            deadline: None,
            completed: false,
            removed: false,
        })
    }
}

/// Identidade do usuário no sistema de suporte, depois de autenticar.
/// Não guarda token — credenciais são responsabilidade da infraestrutura
/// (seção 11: "Use secure credential storage where appropriate").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportIdentity {
    pub system: ExternalSystem,
    /// Id do usuário na origem, como texto (o Mastersys usa inteiro).
    pub user_id: String,
    pub display_name: String,
    pub email: Option<String>,
}

fn non_empty(s: String) -> Option<String> {
    let t = s.trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// Corta por *caractere*, não por byte — evita cortar no meio de um UTF-8
/// multibyte (nomes de cliente com acento são a regra, não a exceção).
fn truncate(s: String, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s;
    }
    s.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_and_kind_roundtrip() {
        assert_eq!(
            ExternalSystem::parse("MASTERSYS").unwrap(),
            ExternalSystem::Mastersys
        );
        assert_eq!(ExternalSystem::Mastersys.as_str(), "mastersys");
        assert!(ExternalSystem::parse("jira").is_err());

        assert_eq!(ExternalKind::parse("ticket").unwrap(), ExternalKind::Ticket);
        assert_eq!(ExternalKind::parse(" Task ").unwrap(), ExternalKind::Task);
        assert!(ExternalKind::parse("epic").is_err());
    }

    #[test]
    fn external_ref_requires_id() {
        assert!(ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Task, "  ").is_err());
        let long = "9".repeat(65);
        assert!(ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Task, long).is_err());
        assert!(ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Task, " 42 ").is_ok());
    }

    #[test]
    fn external_ref_trims_and_drops_blank_context() {
        let r = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Ticket, "10")
            .unwrap()
            .with_client(Some("   ".into()))
            .with_ticket(Some(" 991 ".into()))
            .with_status_label(None);
        assert_eq!(r.client, None);
        assert_eq!(r.ticket.as_deref(), Some("991"));
        assert_eq!(r.status_label, None);
        assert_eq!(r.dedup_key(), "mastersys:10");
    }

    #[test]
    fn roles_default_to_unknown_and_can_be_both() {
        let r = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Ticket, "10").unwrap();
        assert!(
            !r.role_analyst && !r.role_attendant,
            "sem informação da origem o app não afirma papel nenhum"
        );

        let both = r.clone().with_roles(true, true);
        assert!(both.role_analyst && both.role_attendant);

        let only_attendant = r.with_roles(false, true);
        assert!(!only_attendant.role_analyst);
        assert!(only_attendant.role_attendant);
    }

    #[test]
    fn truncate_respects_char_boundaries() {
        let acented = "ação".repeat(100); // 400 chars, 500+ bytes
        let cut = truncate(acented, 200);
        assert_eq!(cut.chars().count(), 200);
    }

    #[test]
    fn work_item_requires_title() {
        let r = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Task, "1").unwrap();
        assert!(ExternalWorkItem::new(r.clone(), "  ").is_err());
        let item = ExternalWorkItem::new(r, "Corrigir NF-e").unwrap();
        assert_eq!(item.title, "Corrigir NF-e");
        assert!(!item.completed);
        assert!(!item.removed);
    }
}
