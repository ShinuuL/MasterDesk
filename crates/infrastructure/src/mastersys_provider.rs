//! `MastersysProvider` — implementação de `SupportSystemProvider` contra a API
//! do Mastersys Suporte (ADR-006).
//!
//! Este é o **único** módulo do MasterDesk que conhece endpoints, JWT e o JSON
//! do Mastersys. Tudo acima daqui fala `ExternalWorkItem`/`SupportIdentity`.
//!
//! ## Contrato (validado no código do Mastersys, não suposto)
//!
//! Autenticação — `modules/users/{routes,controllers/AuthController,services/AuthService}.ts`:
//! - `POST /api/auth/login`   `{identifier, password}` → `{success, data:{accessToken, refreshToken, user:{id,name,email,...}}}`
//! - `POST /api/auth/refresh` `{refreshToken}`          → `{success, data:{accessToken}}`
//! - Rotas autenticadas exigem `Authorization: Bearer <accessToken>`
//!   (`shared/infra/http/middlewares/authMiddleware.ts`).
//!
//! Leitura — `modules/tasks/{routes,controllers/TaskController}.ts` e
//! `modules/tickets/{routes,controllers/TicketController}.ts`:
//! - `GET /api/tasks/users/:userId` → array **cru** de `TaskDTO` (sem envelope)
//! - `GET /api/tickets?assignedTo=<userId>` → `{success, data:[TicketDTO]}`
//!
//! Prazo efetivo de uma tarefa: replica `getEffectiveDueDate`
//! (`modules/tasks/utils/overdue.ts`) — ver [`effective_due_date`].
//!
//! ## Escopo deliberado: somente leitura
//!
//! O provider nunca escreve no Mastersys. Fechar chamado, comentar ou
//! reatribuir continuam no sistema de origem. Isso mantém o MasterDesk fora do
//! caminho crítico do suporte (CLAUDE §12/18).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use masterdesk_domain::{
    ports::SupportSystemProvider, DomainError, DomainResult, ExternalKind, ExternalRef,
    ExternalSystem, ExternalWorkItem, Priority, SupportIdentity,
};
use serde::Deserialize;
use tokio::sync::RwLock;

use crate::secret_store::{SecretKey, SecretResult, SecretStore, SecretStoreError};
use crate::sqlite_settings_repository::{SettingKey, SqliteSettingsRepository};

/// Timeout por requisição. Curto de propósito: a sincronização roda em segundo
/// plano e não pode prender a UI quando a VPN cai.
const REQUEST_TIMEOUT_SECS: u64 = 15;

pub struct MastersysProvider {
    settings: Arc<SqliteSettingsRepository>,
    secrets: SecretStore,
    http: reqwest::Client,
    /// Access token vive só em memória — é de curta duração e não vale o custo
    /// de escrever no cofre a cada refresh.
    access_token: RwLock<Option<String>>,
}

impl MastersysProvider {
    pub fn new(settings: Arc<SqliteSettingsRepository>) -> Self {
        Self::with_secret_store(settings, SecretStore::new())
    }

    pub fn with_secret_store(
        settings: Arc<SqliteSettingsRepository>,
        secrets: SecretStore,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(concat!("MasterDesk/", env!("CARGO_PKG_VERSION")))
            .build()
            // `Client::builder().build()` só falha por TLS indisponível no
            // sistema; nesse caso nada da integração funcionaria mesmo.
            .unwrap_or_default();
        Self {
            settings,
            secrets,
            http,
            access_token: RwLock::new(None),
        }
    }

    /// Endpoint configurado, já normalizado (sem `/` final).
    pub async fn base_url(&self) -> DomainResult<Option<String>> {
        Ok(self
            .settings
            .get(SettingKey::MastersysBaseUrl)
            .await?
            .map(|u| u.trim_end_matches('/').to_string()))
    }

    /// Grava o endpoint. Valida o esquema aqui porque um endpoint sem
    /// `http(s)://` faz o `reqwest` falhar com erro incompreensível na UI.
    pub async fn set_base_url(&self, base_url: &str) -> DomainResult<()> {
        let url = base_url.trim().trim_end_matches('/');
        if url.is_empty() {
            return Err(DomainError::Validation(
                "endereço do Mastersys não pode ser vazio".into(),
            ));
        }
        let parsed = reqwest::Url::parse(url)
            .map_err(|_| DomainError::Validation("endereço inválido — use https://host".into()))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(DomainError::Validation(
                "endereço deve começar com http:// ou https://".into(),
            ));
        }
        self.settings.set(SettingKey::MastersysBaseUrl, url).await
    }

    fn required_base_url(base: Option<String>) -> DomainResult<String> {
        base.ok_or(DomainError::IntegrationNotConfigured)
    }

    /// Access token válido, renovando pelo refresh token quando necessário.
    async fn access_token(&self) -> DomainResult<String> {
        if let Some(t) = self.access_token.read().await.clone() {
            return Ok(t);
        }
        self.refresh_access_token().await
    }

    async fn refresh_access_token(&self) -> DomainResult<String> {
        let base = Self::required_base_url(self.base_url().await?)?;
        let refresh = self
            .load_refresh_token()?
            .ok_or(DomainError::IntegrationNotConfigured)?;

        let response = self
            .http
            .post(format!("{base}/api/auth/refresh"))
            .json(&serde_json::json!({ "refreshToken": refresh }))
            .send()
            .await
            .map_err(map_reqwest_err)?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            // Refresh expirado/revogado: apaga para o app pedir login de novo
            // em vez de ficar tentando eternamente.
            let _ = self.secrets.delete(SecretKey::MastersysRefreshToken);
            *self.access_token.write().await = None;
            return Err(DomainError::Unauthorized);
        }
        let envelope: Envelope<RefreshData> = parse_json(response).await?;
        let token = envelope.data.access_token;
        *self.access_token.write().await = Some(token.clone());
        Ok(token)
    }

    /// Executa um GET autenticado, renovando o token uma única vez em 401.
    ///
    /// Uma tentativa só: se o token recém-renovado também for rejeitado, o
    /// problema é permissão e não expiração — repetir viraria laço.
    async fn authed_get(&self, base: &str, path: &str) -> DomainResult<reqwest::Response> {
        let url = format!("{base}{path}");
        let token = self.access_token().await?;
        let response = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(map_reqwest_err)?;

        if response.status() != reqwest::StatusCode::UNAUTHORIZED {
            return Ok(response);
        }

        *self.access_token.write().await = None;
        let token = self.refresh_access_token().await?;
        let retried = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(map_reqwest_err)?;
        if retried.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DomainError::Unauthorized);
        }
        Ok(retried)
    }

    fn load_refresh_token(&self) -> DomainResult<Option<String>> {
        map_secret(self.secrets.load(SecretKey::MastersysRefreshToken))
    }

    async fn stored_identity(&self) -> DomainResult<Option<SupportIdentity>> {
        let Some(user_id) = self.settings.get(SettingKey::MastersysUserId).await? else {
            return Ok(None);
        };
        let display_name = self
            .settings
            .get(SettingKey::MastersysUserName)
            .await?
            .unwrap_or_else(|| user_id.clone());
        let email = self.settings.get(SettingKey::MastersysUserEmail).await?;
        Ok(Some(SupportIdentity {
            system: ExternalSystem::Mastersys,
            user_id,
            display_name,
            email,
        }))
    }
}

#[async_trait]
impl SupportSystemProvider for MastersysProvider {
    async fn is_configured(&self) -> bool {
        let has_endpoint = matches!(self.base_url().await, Ok(Some(_)));
        let has_session = matches!(self.load_refresh_token(), Ok(Some(_)));
        let has_user = matches!(
            self.settings.get(SettingKey::MastersysUserId).await,
            Ok(Some(_))
        );
        has_endpoint && has_session && has_user
    }

    async fn authenticate(
        &self,
        identifier: &str,
        password: &str,
    ) -> DomainResult<SupportIdentity> {
        let base = Self::required_base_url(self.base_url().await?)?;
        if identifier.trim().is_empty() || password.is_empty() {
            return Err(DomainError::Validation(
                "usuário e senha são obrigatórios".into(),
            ));
        }

        let response = self
            .http
            .post(format!("{base}/api/auth/login"))
            .json(&serde_json::json!({
                "identifier": identifier.trim(),
                "password": password,
            }))
            .send()
            .await
            .map_err(map_reqwest_err)?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DomainError::Unauthorized);
        }
        let envelope: Envelope<LoginData> = parse_json(response).await?;
        let data = envelope.data;

        let identity = SupportIdentity {
            system: ExternalSystem::Mastersys,
            user_id: data.user.id.to_string(),
            display_name: data.user.name.clone(),
            email: data.user.email.clone(),
        };

        // Cofre primeiro: se o SO não tem cofre, falha ANTES de gravar a
        // identidade — senão o app pareceria conectado sem poder renovar.
        map_secret(
            self.secrets
                .store(SecretKey::MastersysRefreshToken, &data.refresh_token),
        )?;
        *self.access_token.write().await = Some(data.access_token);

        self.settings
            .set(SettingKey::MastersysUserId, &identity.user_id)
            .await?;
        self.settings
            .set(SettingKey::MastersysUserName, &identity.display_name)
            .await?;
        match identity.email.as_deref() {
            Some(email) => {
                self.settings
                    .set(SettingKey::MastersysUserEmail, email)
                    .await?
            }
            None => self.settings.remove(SettingKey::MastersysUserEmail).await?,
        }

        Ok(identity)
    }

    async fn current_identity(&self) -> DomainResult<Option<SupportIdentity>> {
        // Sem refresh token não há sessão, mesmo que o id do usuário tenha
        // sobrado no banco (ex. cofre limpo pelo usuário no SO).
        if self.load_refresh_token()?.is_none() {
            return Ok(None);
        }
        self.stored_identity().await
    }

    async fn sign_out(&self) -> DomainResult<()> {
        // Idempotente e best-effort: um cofre indisponível não pode impedir o
        // usuário de desconectar localmente.
        let _ = self.secrets.delete(SecretKey::MastersysRefreshToken);
        *self.access_token.write().await = None;
        self.settings.remove(SettingKey::MastersysUserId).await?;
        self.settings.remove(SettingKey::MastersysUserName).await?;
        self.settings.remove(SettingKey::MastersysUserEmail).await?;
        Ok(())
    }

    async fn fetch_assigned_work(&self) -> DomainResult<Vec<ExternalWorkItem>> {
        let base = Self::required_base_url(self.base_url().await?)?;
        let identity = self
            .current_identity()
            .await?
            .ok_or(DomainError::IntegrationNotConfigured)?;
        let user_id = identity.user_id;

        // 1) Tarefas do quadro — a fila real de trabalho do usuário.
        //    Resposta é um array cru (TaskController.getByUser faz res.json(tasks)).
        let tasks: Vec<MastersysTask> = parse_json_raw(
            self.authed_get(&base, &format!("/api/tasks/users/{user_id}"))
                .await?,
        )
        .await?;

        let mut items: Vec<ExternalWorkItem> = Vec::with_capacity(tasks.len());
        let mut ticket_ids_with_task: Vec<i64> = Vec::new();
        for task in &tasks {
            if let Some(ticket_id) = task.ticket_id {
                ticket_ids_with_task.push(ticket_id);
            }
            items.push(task.to_work_item()?);
        }

        // 2) Chamados atribuídos ao usuário que ainda NÃO têm tarefa no quadro.
        //    Sem esse filtro o mesmo chamado apareceria duas vezes no
        //    MasterDesk (uma como tarefa, outra como chamado).
        let tickets: Envelope<Vec<MastersysTicket>> = parse_json(
            self.authed_get(&base, &format!("/api/tickets?assignedTo={user_id}"))
                .await?,
        )
        .await?;
        for ticket in tickets.data {
            if ticket_ids_with_task.contains(&ticket.id) {
                continue;
            }
            items.push(ticket.to_work_item()?);
        }

        Ok(items)
    }
}

// ---------------------------------------------------------------------------
// Regras de tradução (puras — testadas sem rede)
// ---------------------------------------------------------------------------

/// Prazo efetivo de uma tarefa do Mastersys.
///
/// Replica `getEffectiveDueDate` de `modules/tasks/utils/overdue.ts`, que é a
/// fonte da verdade lá (usada pelo alerta de "tarefas atrasadas" do sistema).
/// A regra não é "a primeira data que existir": quando o chamado tem previsão
/// **e** agendamento, vale a mais próxima **entre as futuras**; se as duas já
/// passaram, vale a mais recente.
///
/// Manter a mesma regra importa porque é ela que decide se o lembrete do
/// MasterDesk dispara na hora certa (CLAUDE §19: cálculo de deadline é
/// business-critical).
pub fn effective_due_date(
    ticket_forecast: Option<DateTime<Utc>>,
    ticket_scheduled: Option<DateTime<Utc>>,
    task_scheduled: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    match (ticket_forecast, ticket_scheduled) {
        (Some(forecast), Some(scheduled)) => {
            let future_forecast = (forecast >= now).then_some(forecast);
            let future_scheduled = (scheduled >= now).then_some(scheduled);
            match (future_forecast, future_scheduled) {
                (Some(f), Some(s)) => Some(f.min(s)),
                (Some(f), None) => Some(f),
                (None, Some(s)) => Some(s),
                (None, None) => Some(forecast.max(scheduled)),
            }
        }
        (Some(forecast), None) => Some(forecast),
        (None, Some(scheduled)) => Some(scheduled),
        (None, None) => task_scheduled,
    }
}

/// Prioridade do chamado → prioridade do MasterDesk.
///
/// `critical` vira `Urgent` porque é o topo das duas escalas. Valor
/// desconhecido cai em `Medium` em vez de erro: o Mastersys pode ganhar uma
/// prioridade nova e isso não deve quebrar a sincronização inteira.
fn map_ticket_priority(raw: &str) -> Priority {
    match raw.trim().to_ascii_lowercase().as_str() {
        "low" => Priority::Low,
        "medium" => Priority::Medium,
        "high" => Priority::High,
        "critical" => Priority::Urgent,
        _ => Priority::Medium,
    }
}

// ---------------------------------------------------------------------------
// DTOs da API do Mastersys (privados — não vazam para cima)
// ---------------------------------------------------------------------------

/// Envelope `{success, data}` usado por `/api/auth/*` e `/api/tickets`.
/// `/api/tasks/users/:id` NÃO usa envelope — ver `parse_json_raw`.
#[derive(Debug, Deserialize)]
struct Envelope<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginData {
    access_token: String,
    refresh_token: String,
    user: LoginUser,
}

#[derive(Debug, Deserialize)]
struct LoginUser {
    id: i64,
    name: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshData {
    access_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MastersysTask {
    id: i64,
    title: String,
    #[serde(default)]
    description: String,
    status: String,
    #[serde(default)]
    ticket_id: Option<i64>,
    #[serde(default)]
    scheduled_at: Option<DateTime<Utc>>,
    #[serde(default)]
    ticket_forecast_date: Option<DateTime<Utc>>,
    #[serde(default)]
    ticket_scheduled_for: Option<DateTime<Utc>>,
    #[serde(default)]
    ticket_client_name: Option<String>,
}

impl MastersysTask {
    fn to_work_item(&self) -> DomainResult<ExternalWorkItem> {
        // Tarefa vinculada a chamado é apresentada como chamado — é assim que
        // o atendente pensa nela, e é o mesmo critério do `note_kind` usado
        // pela integração já existente do Mastersys.
        let kind = if self.ticket_id.is_some() {
            ExternalKind::Ticket
        } else {
            ExternalKind::Task
        };
        let reference = ExternalRef::new(
            ExternalSystem::Mastersys,
            kind,
            // Prefixo por origem: o id 12 de `tasks` e o id 12 de `tickets`
            // são itens diferentes e não podem colidir na chave única local.
            format!("task-{}", self.id),
        )?
        .with_client(self.ticket_client_name.clone())
        .with_ticket(self.ticket_id.map(|t| t.to_string()))
        .with_status_label(Some(self.status.clone()));

        let mut item = ExternalWorkItem::new(reference, &self.title)?;
        item.description = self.description.clone();
        // `TaskDTO` do Mastersys não tem prioridade — não existe campo para
        // derivar. Deixar `Medium` é honesto; inventar (ex. "atrasada = alta")
        // seria criar um dado que a origem não tem.
        item.priority = Priority::default();
        item.deadline = effective_due_date(
            self.ticket_forecast_date,
            self.ticket_scheduled_for,
            self.scheduled_at,
            Utc::now(),
        );
        item.completed = self.status == "finished";
        // `canceled` sai do MasterDesk: é o mesmo tratamento que a integração
        // NoteDesk do Mastersys dá (vai para a lixeira).
        item.removed = self.status == "canceled";
        Ok(item)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MastersysTicket {
    id: i64,
    title: String,
    #[serde(default)]
    description: String,
    status: String,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    forecast_date: Option<DateTime<Utc>>,
    #[serde(default)]
    scheduled_for: Option<DateTime<Utc>>,
    #[serde(default)]
    client_name: Option<String>,
    #[serde(default)]
    resolved_at: Option<DateTime<Utc>>,
    #[serde(default)]
    closed_at: Option<DateTime<Utc>>,
}

impl MastersysTicket {
    fn to_work_item(&self) -> DomainResult<ExternalWorkItem> {
        let reference = ExternalRef::new(
            ExternalSystem::Mastersys,
            ExternalKind::Ticket,
            format!("ticket-{}", self.id),
        )?
        .with_client(self.client_name.clone())
        .with_ticket(Some(self.id.to_string()))
        .with_status_label(Some(self.status.clone()));

        let mut item = ExternalWorkItem::new(reference, &self.title)?;
        item.description = self.description.clone();
        item.priority = map_ticket_priority(self.priority.as_deref().unwrap_or("medium"));
        item.deadline =
            effective_due_date(self.forecast_date, self.scheduled_for, None, Utc::now());
        // O Mastersys permite status customizados (`TicketStatus` aceita
        // qualquer string), então "concluído" é decidido pelos timestamps, que
        // são estáveis — não por uma lista de slugs que pode crescer.
        item.completed = self.closed_at.is_some() || self.resolved_at.is_some();
        item.removed = self.status == "cancelado";
        Ok(item)
    }
}

// ---------------------------------------------------------------------------
// Erros
// ---------------------------------------------------------------------------

/// Mensagens voltadas ao usuário. Nunca incluem URL, header ou corpo — o
/// `Display` do `reqwest::Error` pode conter a URL completa (CLAUDE §13/18).
fn map_reqwest_err(e: reqwest::Error) -> DomainError {
    if e.is_timeout() {
        return DomainError::Integration("o Mastersys não respondeu no tempo esperado".into());
    }
    if e.is_connect() {
        return DomainError::Integration(
            "não foi possível conectar ao Mastersys — verifique o endereço e a rede".into(),
        );
    }
    if e.is_decode() {
        return DomainError::Integration("o Mastersys respondeu em formato inesperado".into());
    }
    DomainError::Integration("falha de comunicação com o Mastersys".into())
}

fn map_secret<T>(r: SecretResult<T>) -> DomainResult<T> {
    r.map_err(|e| match e {
        SecretStoreError::Unavailable => {
            DomainError::Integration("o cofre de credenciais do sistema não está disponível".into())
        }
        SecretStoreError::Failed => {
            DomainError::Integration("falha ao acessar o cofre de credenciais".into())
        }
    })
}

async fn ensure_success(response: reqwest::Response) -> DomainResult<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(DomainError::Unauthorized);
    }
    if status == reqwest::StatusCode::FORBIDDEN {
        return Err(DomainError::Integration(
            "seu usuário do Mastersys não tem permissão para esta consulta".into(),
        ));
    }
    Err(DomainError::Integration(format!(
        "o Mastersys respondeu com erro {}",
        status.as_u16()
    )))
}

async fn parse_json<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> DomainResult<T> {
    ensure_success(response)
        .await?
        .json::<T>()
        .await
        .map_err(map_reqwest_err)
}

/// Igual a `parse_json`, mas para endpoints sem envelope `{success, data}`.
/// Existe como função separada para o call site declarar qual formato espera.
async fn parse_json_raw<T: serde::de::DeserializeOwned>(
    response: reqwest::Response,
) -> DomainResult<T> {
    parse_json(response).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(y: i32, mo: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, 0, 0).unwrap()
    }

    // -----------------------------------------------------------------------
    // effective_due_date — espelha getEffectiveDueDate do Mastersys
    // -----------------------------------------------------------------------

    #[test]
    fn falls_back_to_task_schedule_when_no_ticket_dates() {
        let now = at(2026, 9, 2, 12);
        let own = at(2026, 9, 5, 9);
        assert_eq!(effective_due_date(None, None, Some(own), now), Some(own));
        assert_eq!(effective_due_date(None, None, None, now), None);
    }

    #[test]
    fn ticket_dates_win_over_the_tasks_own_schedule() {
        let now = at(2026, 9, 2, 12);
        let forecast = at(2026, 9, 10, 9);
        let own = at(2026, 9, 3, 9);
        assert_eq!(
            effective_due_date(Some(forecast), None, Some(own), now),
            Some(forecast),
            "previsão do chamado tem precedência sobre o agendamento próprio"
        );
    }

    #[test]
    fn with_both_ticket_dates_the_nearest_future_wins() {
        let now = at(2026, 9, 2, 12);
        let forecast = at(2026, 9, 20, 9);
        let scheduled = at(2026, 9, 5, 9);
        assert_eq!(
            effective_due_date(Some(forecast), Some(scheduled), None, now),
            Some(scheduled)
        );
        // e a ordem dos argumentos não muda o resultado
        assert_eq!(
            effective_due_date(Some(scheduled), Some(forecast), None, now),
            Some(scheduled)
        );
    }

    #[test]
    fn with_one_ticket_date_in_the_past_the_future_one_wins() {
        let now = at(2026, 9, 2, 12);
        let past = at(2026, 8, 1, 9);
        let future = at(2026, 9, 9, 9);
        assert_eq!(
            effective_due_date(Some(past), Some(future), None, now),
            Some(future)
        );
        assert_eq!(
            effective_due_date(Some(future), Some(past), None, now),
            Some(future)
        );
    }

    #[test]
    fn with_both_ticket_dates_in_the_past_the_latest_wins() {
        let now = at(2026, 9, 2, 12);
        let older = at(2026, 7, 1, 9);
        let newer = at(2026, 8, 20, 9);
        assert_eq!(
            effective_due_date(Some(older), Some(newer), None, now),
            Some(newer),
            "atraso é medido pela data mais recente, como no Mastersys"
        );
    }

    // -----------------------------------------------------------------------
    // Prioridade
    // -----------------------------------------------------------------------

    #[test]
    fn ticket_priority_mapping() {
        assert_eq!(map_ticket_priority("low"), Priority::Low);
        assert_eq!(map_ticket_priority("Medium"), Priority::Medium);
        assert_eq!(map_ticket_priority(" HIGH "), Priority::High);
        assert_eq!(map_ticket_priority("critical"), Priority::Urgent);
        assert_eq!(
            map_ticket_priority("blocker"),
            Priority::Medium,
            "prioridade nova na origem não pode quebrar o sync"
        );
    }

    // -----------------------------------------------------------------------
    // Desserialização + tradução dos payloads reais
    // -----------------------------------------------------------------------

    #[test]
    fn task_without_ticket_becomes_a_task_kind_item() {
        let json = r#"{
            "id": 12,
            "title": "Revisar planilha",
            "description": "conferir totais",
            "status": "pending",
            "userId": 7,
            "creatorId": 7,
            "isInternal": false,
            "createdAt": "2026-09-01T10:00:00.000Z",
            "updatedAt": "2026-09-01T10:00:00.000Z"
        }"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        let item = task.to_work_item().unwrap();

        assert_eq!(item.reference.kind, ExternalKind::Task);
        assert_eq!(item.reference.external_id, "task-12");
        assert_eq!(item.reference.ticket, None);
        assert_eq!(item.reference.status_label.as_deref(), Some("pending"));
        assert_eq!(item.title, "Revisar planilha");
        assert_eq!(item.deadline, None);
        assert!(!item.completed);
        assert!(!item.removed);
    }

    #[test]
    fn task_with_ticket_becomes_a_ticket_kind_item_with_client() {
        let json = r#"{
            "id": 30,
            "title": "Erro na NF-e",
            "description": "",
            "status": "in_progress",
            "userId": 7,
            "creatorId": 3,
            "ticketId": 4821,
            "ticketClientName": "Padaria do Zé",
            "ticketForecastDate": "2026-09-10T12:00:00.000Z",
            "isInternal": false,
            "createdAt": "2026-09-01T10:00:00.000Z",
            "updatedAt": "2026-09-01T10:00:00.000Z"
        }"#;
        let item: MastersysTask = serde_json::from_str(json).unwrap();
        let item = item.to_work_item().unwrap();

        assert_eq!(item.reference.kind, ExternalKind::Ticket);
        assert_eq!(item.reference.external_id, "task-30");
        assert_eq!(item.reference.ticket.as_deref(), Some("4821"));
        assert_eq!(item.reference.client.as_deref(), Some("Padaria do Zé"));
        assert_eq!(item.deadline, Some(at(2026, 9, 10, 12)));
    }

    #[test]
    fn finished_task_is_completed_and_canceled_task_is_removed() {
        let base = |status: &str| {
            let json = format!(
                r#"{{"id":1,"title":"t","description":"","status":"{status}",
                     "userId":1,"creatorId":1,"isInternal":false,
                     "createdAt":"2026-09-01T10:00:00.000Z",
                     "updatedAt":"2026-09-01T10:00:00.000Z"}}"#
            );
            serde_json::from_str::<MastersysTask>(&json)
                .unwrap()
                .to_work_item()
                .unwrap()
        };

        let finished = base("finished");
        assert!(finished.completed);
        assert!(!finished.removed);

        let canceled = base("canceled");
        assert!(canceled.removed);

        let overdue = base("overdue");
        assert!(!overdue.completed);
        assert!(!overdue.removed);
    }

    #[test]
    fn ticket_is_translated_with_priority_and_forecast() {
        let json = r#"{
            "id": 4821,
            "title": "Instalar atualização",
            "description": "cliente pediu versão 3.2",
            "status": "aguardando_atualizacao",
            "priority": "critical",
            "forecastDate": "2026-09-12T18:00:00.000Z",
            "clientName": "Mercado Central",
            "resolvedAt": null,
            "closedAt": null
        }"#;
        let ticket: MastersysTicket = serde_json::from_str(json).unwrap();
        let item = ticket.to_work_item().unwrap();

        assert_eq!(item.reference.external_id, "ticket-4821");
        assert_eq!(item.reference.kind, ExternalKind::Ticket);
        assert_eq!(item.reference.ticket.as_deref(), Some("4821"));
        assert_eq!(item.reference.client.as_deref(), Some("Mercado Central"));
        assert_eq!(
            item.reference.status_label.as_deref(),
            Some("aguardando_atualizacao")
        );
        assert_eq!(item.priority, Priority::Urgent);
        assert_eq!(item.deadline, Some(at(2026, 9, 12, 18)));
        assert!(!item.completed);
    }

    #[test]
    fn ticket_completion_comes_from_timestamps_not_status_slug() {
        // Status customizado desconhecido, mas com closedAt preenchido.
        let json = r#"{
            "id": 9,
            "title": "x",
            "status": "um_status_customizado_qualquer",
            "priority": "low",
            "closedAt": "2026-09-01T10:00:00.000Z"
        }"#;
        let item = serde_json::from_str::<MastersysTicket>(json)
            .unwrap()
            .to_work_item()
            .unwrap();
        assert!(item.completed);
    }

    #[test]
    fn canceled_ticket_is_removed() {
        let json = r#"{"id":9,"title":"x","status":"cancelado","priority":"low"}"#;
        let item = serde_json::from_str::<MastersysTicket>(json)
            .unwrap()
            .to_work_item()
            .unwrap();
        assert!(item.removed);
    }

    #[test]
    fn task_and_ticket_ids_never_collide() {
        let task_json = r#"{"id":5,"title":"a","status":"pending","userId":1,
                            "creatorId":1,"isInternal":false,
                            "createdAt":"2026-09-01T10:00:00.000Z",
                            "updatedAt":"2026-09-01T10:00:00.000Z"}"#;
        let ticket_json = r#"{"id":5,"title":"b","status":"novo","priority":"low"}"#;
        let a = serde_json::from_str::<MastersysTask>(task_json)
            .unwrap()
            .to_work_item()
            .unwrap();
        let b = serde_json::from_str::<MastersysTicket>(ticket_json)
            .unwrap()
            .to_work_item()
            .unwrap();
        assert_ne!(a.reference.dedup_key(), b.reference.dedup_key());
    }

    #[test]
    fn login_envelope_deserializes() {
        let json = r#"{
            "success": true,
            "data": {
                "accessToken": "aaa",
                "refreshToken": "rrr",
                "user": {
                    "id": 7, "name": "Gabriel", "login": "gabriel",
                    "email": "g@exemplo.com", "role": "developer",
                    "roles": ["developer"], "isActive": true,
                    "canValidateRequests": false,
                    "createdAt": "2026-01-01T00:00:00.000Z"
                }
            }
        }"#;
        let env: Envelope<LoginData> = serde_json::from_str(json).unwrap();
        assert_eq!(env.data.access_token, "aaa");
        assert_eq!(env.data.refresh_token, "rrr");
        assert_eq!(env.data.user.id, 7);
        assert_eq!(env.data.user.name, "Gabriel");
    }

    #[test]
    fn task_payload_tolerates_unknown_and_missing_fields() {
        // A API do Mastersys evolui; campo novo não pode derrubar o sync, e
        // campos opcionais ausentes precisam cair no default.
        let json = r#"{
            "id": 1, "title": "t", "status": "pending",
            "campoQueAindaNaoExiste": {"a": 1}
        }"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.description, "");
        assert_eq!(task.ticket_id, None);
        assert!(task.to_work_item().is_ok());
    }

    #[test]
    fn task_with_blank_title_is_rejected_instead_of_imported() {
        let json = r#"{"id":1,"title":"   ","status":"pending"}"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        assert!(
            task.to_work_item().is_err(),
            "item sem título viraria uma tarefa sem nome no quadro"
        );
    }
}
