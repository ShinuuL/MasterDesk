//! `MastersysProvider` — implementação de `SupportSystemProvider` contra a API
//! do Mastersys Suporte (ADR-006).
//!
//! Este é o **único** módulo do MasterNote que conhece endpoints, JWT e o JSON
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
//! - `GET /api/tickets/paginated?assignedTo=<userId>` →
//!   `{success, data:[TicketDTO], pagination:{page,pageSize,total,totalPages}}`
//!
//! ## Dois filtros implícitos do servidor que moldam esta integração
//!
//! 1. `TaskRepository.findAll` aplica `AND t.status != 'finished'` quando
//!    nenhum status é pedido, e `AND t.is_internal = 0`. Ou seja: tarefa
//!    concluída **não chega** — ela simplesmente sai da resposta, e a
//!    reconciliação a trata como saída da fila (`MastersysSyncService`
//!    apaga o espelho, ou o marca concluído se tiver anotações). Tarefa
//!    interna nunca sincroniza, e o endpoint não tem parâmetro para pedi-la.
//!    O filtro é sobre `finished` apenas — **`canceled` chega normalmente**,
//!    e é por isso que `item.removed` funciona de fato.
//!
//! 2. `TicketRepository.findAll` (rota `GET /api/tickets`) não tem filtro de
//!    status padrão nem `LIMIT`: traria todo chamado já atribuído ao usuário,
//!    com a `description` inteira em cada um. Por isso usamos `/paginated`
//!    com janela por data — ver `MastersysProvider::fetch_tickets`.
//!
//! Prazo efetivo de uma tarefa: replica `getEffectiveDueDate`
//! (`modules/tasks/utils/overdue.ts`) — ver [`effective_due_date`].
//!
//! ## Escopo deliberado: somente leitura
//!
//! O provider nunca escreve no Mastersys. Fechar chamado, comentar ou
//! reatribuir continuam no sistema de origem. Isso mantém o MasterNote fora do
//! caminho crítico do suporte (CLAUDE §12/18).

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use masterdesk_domain::{
    ports::SupportSystemProvider, DomainError, DomainResult, ExternalKind, ExternalRef,
    ExternalSystem, ExternalWorkItem, Priority, SupportIdentity,
};
use serde::{Deserialize, Deserializer};
use tokio::sync::RwLock;

use crate::secret_store::{SecretKey, SecretResult, SecretStore, SecretStoreError};
use crate::sqlite_settings_repository::{SettingKey, SqliteSettingsRepository};
use crate::sqlite_status_catalog_repository::{
    MastersysTicketStatus, SqliteStatusCatalogRepository,
};

/// Timeout por requisição. Curto de propósito: a sincronização roda em segundo
/// plano e não pode prender a UI quando a VPN cai.
const REQUEST_TIMEOUT_SECS: u64 = 15;

/// Janela padrão de chamados, em dias. Ver `MastersysProvider::fetch_tickets`
/// para por que a janela é por data e não por status.
const DEFAULT_TICKET_WINDOW_DAYS: i64 = 90;

/// Máximo aceito por `filtersSchema.pageSize` no Mastersys (`.max(200)`).
/// Pedir mais que isso é erro de validação, não truncamento.
const TICKET_PAGE_SIZE: u32 = 200;

/// Teto de páginas por sincronização. Guarda contra uma instalação onde a
/// janela ainda deixe muitos chamados: é melhor sincronizar um subconjunto
/// grande do que prender a UI puxando páginas indefinidamente.
const TICKET_MAX_PAGES: u32 = 25;

/// Mínimo aceito por `GET /api/tickets/search` — abaixo disso o servidor
/// devolve `[]` sem consultar (`TicketService.searchTickets`). Checamos aqui
/// para dar mensagem em vez de um resultado vazio inexplicável.
const TICKET_SEARCH_MIN_CHARS: usize = 3;

pub struct MastersysProvider {
    settings: Arc<SqliteSettingsRepository>,
    /// Catálogo de status espelhado. Mora no provider porque ele é o único que
    /// conhece o endpoint da origem, e ele já escreve configuração local
    /// (identidade do usuário) pelo mesmo motivo.
    status_catalog: Arc<SqliteStatusCatalogRepository>,
    secrets: SecretStore,
    http: reqwest::Client,
    /// Access token vive só em memória — é de curta duração e não vale o custo
    /// de escrever no cofre a cada refresh.
    access_token: RwLock<Option<String>>,
}

impl MastersysProvider {
    pub fn new(
        settings: Arc<SqliteSettingsRepository>,
        status_catalog: Arc<SqliteStatusCatalogRepository>,
    ) -> Self {
        Self::with_secret_store(settings, status_catalog, SecretStore::new())
    }

    pub fn with_secret_store(
        settings: Arc<SqliteSettingsRepository>,
        status_catalog: Arc<SqliteStatusCatalogRepository>,
        secrets: SecretStore,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(concat!("MasterNote/", env!("CARGO_PKG_VERSION")))
            .build()
            // `Client::builder().build()` só falha por TLS indisponível no
            // sistema; nesse caso nada da integração funcionaria mesmo.
            .unwrap_or_default();
        Self {
            settings,
            status_catalog,
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
            // Motivo do servidor quando houver: aqui ele distingue "Invalid
            // refresh token" de "User account is inactive" e "User not found",
            // que pedem ações bem diferentes do usuário.
            return Err(DomainError::Unauthorized(
                unauthorized_reason(
                    response,
                    "sua sessão do Mastersys expirou — conecte novamente",
                )
                .await,
            ));
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
            // Token recém-renovado e ainda recusado: não é expiração, é o
            // usuário não ter acesso a este recurso.
            return Err(DomainError::Unauthorized(
                unauthorized_reason(
                    retried,
                    "seu usuário do Mastersys não tem acesso a esta consulta",
                )
                .await,
            ));
        }
        Ok(retried)
    }

    fn load_refresh_token(&self) -> DomainResult<Option<String>> {
        map_secret(self.secrets.load(SecretKey::MastersysRefreshToken))
    }

    /// Janela de chamados em dias, com o padrão aplicado quando não
    /// configurada. Um valor gravado inválido cai no padrão em vez de
    /// derrubar a sincronização.
    pub async fn ticket_window_days(&self) -> DomainResult<i64> {
        Ok(self
            .settings
            .get(SettingKey::MastersysTicketWindowDays)
            .await?
            .and_then(|v| v.trim().parse::<i64>().ok())
            .filter(|d| *d > 0)
            .unwrap_or(DEFAULT_TICKET_WINDOW_DAYS))
    }

    pub async fn set_ticket_window_days(&self, days: i64) -> DomainResult<()> {
        if days <= 0 {
            return Err(DomainError::Validation(
                "a janela de chamados deve ser de pelo menos 1 dia".into(),
            ));
        }
        self.settings
            .set(SettingKey::MastersysTicketWindowDays, &days.to_string())
            .await
    }

    /// Chamados atribuídos ao usuário dentro da janela configurada.
    ///
    /// ## Por que janela por data, e não por status
    ///
    /// O filtro natural seria "só os abertos", mas não há como expressá-lo com
    /// segurança: `ticket_statuses` (o cadastro de status do Mastersys) tem
    /// `value/label/color/display_order/is_active` e **nenhuma flag de status
    /// terminal**, e `TicketStatus` aceita qualquer string porque o cliente
    /// cadastra status próprios em Configurações. Uma allowlist de slugs
    /// abertos perderia chamados em silêncio no dia em que alguém criasse um
    /// status novo — falha pior que uma resposta grande.
    ///
    /// `createdAtStart` não depende do vocabulário de status. O custo é que um
    /// chamado aberto mais antigo que a janela não aparece; aceitável porque a
    /// fila de trabalho real são as tarefas, e este ramo só cobre chamados que
    /// ainda não têm tarefa no quadro.
    async fn fetch_tickets(&self, base: &str, user_id: &str) -> DomainResult<Vec<MastersysTicket>> {
        let window = self.ticket_window_days().await?;
        // `created_at >= ?` no servidor; data-só basta e evita depender do
        // fuso do backend para o recorte.
        let since = (Utc::now() - chrono::Duration::days(window))
            .format("%Y-%m-%d")
            .to_string();

        let mut all: Vec<MastersysTicket> = Vec::new();
        for page in 1..=TICKET_MAX_PAGES {
            let path = format!(
                "/api/tickets/paginated?assignedTo={user_id}&createdAtStart={since}&page={page}&pageSize={TICKET_PAGE_SIZE}"
            );
            let page_response: Paginated<Vec<MastersysTicket>> =
                parse_json(self.authed_get(base, &path).await?).await?;
            let received = page_response.data.len() as u32;
            all.extend(page_response.data);
            // Confia no tamanho recebido, não em `totalPages`: chamado criado
            // entre duas páginas mudaria o total no meio do caminho.
            if received < TICKET_PAGE_SIZE {
                break;
            }
        }
        Ok(all)
    }

    /// Busca o catálogo de status da origem, grava localmente e devolve o
    /// conjunto de status parados.
    ///
    /// **Nunca falha para cima.** Se a origem não responder ou o payload vier
    /// estranho, cai no catálogo já gravado; se nem esse existir, devolve
    /// conjunto vazio e todo item vira "ativo" — que é como o app se comportava
    /// antes deste recurso. O catálogo governa rótulo, cor e filtro padrão:
    /// perder isso degrada a apresentação, e derrubar o sync por causa dele
    /// custaria o trabalho do usuário.
    async fn refresh_status_catalog(&self, base: &str) -> ParkedStatuses {
        match self.fetch_status_catalog(base).await {
            Ok(statuses) => {
                let _ = self.status_catalog.replace_all(&statuses).await;
                statuses.into_iter().collect()
            }
            Err(_) => self
                .status_catalog
                .list()
                .await
                .unwrap_or_default()
                .into_iter()
                .collect(),
        }
    }

    /// `GET /api/ticket-statuses` — array cru, sem envelope, exigindo apenas
    /// autenticação (`ticketStatusesRouter.use(authMiddleware)` sem
    /// `requirePermission` no `list`).
    async fn fetch_status_catalog(&self, base: &str) -> DomainResult<Vec<MastersysTicketStatus>> {
        parse_json_raw(self.authed_get(base, "/api/ticket-statuses").await?).await
    }

    /// Catálogo gravado localmente, para a UI montar filtro e selo de status
    /// sem tocar a rede.
    pub async fn status_catalog(&self) -> DomainResult<Vec<MastersysTicketStatus>> {
        self.status_catalog.list().await
    }

    /// Busca ao vivo no acervo de chamados da origem.
    ///
    /// Complementa o filtro local: o quadro só tem o que está atribuído a você,
    /// e às vezes se quer consultar um chamado antes de assumi-lo.
    ///
    /// `GET /api/tickets/search?q=` cobre id, título, descrição, comentários e
    /// nome do cliente, com `LIMIT` no servidor.
    ///
    /// ## O resultado é consulta, não espelho
    ///
    /// Um chamado achado aqui que não esteja atribuído a você **não pode** ser
    /// gravado como espelho: a sincronização seguinte não o veria na sua fila e
    /// o `retire_mirror` o apagaria. Quem chama deve tratar isto como leitura.
    pub async fn search_tickets(&self, query: &str) -> DomainResult<Vec<ExternalWorkItem>> {
        let base = Self::required_base_url(self.base_url().await?)?;
        let q = query.trim();
        if q.chars().count() < TICKET_SEARCH_MIN_CHARS {
            return Err(DomainError::Validation(format!(
                "digite ao menos {TICKET_SEARCH_MIN_CHARS} caracteres para buscar"
            )));
        }

        let encoded = urlencode(q);
        let envelope: Envelope<Vec<MastersysTicket>> = parse_json(
            self.authed_get(&base, &format!("/api/tickets/search?q={encoded}"))
                .await?,
        )
        .await?;

        // Reusa o catálogo já gravado em vez de buscar de novo: a busca é
        // interativa e não vale um round-trip extra a cada tecla.
        let parked: ParkedStatuses = self
            .status_catalog
            .list()
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

        envelope
            .data
            .iter()
            .map(|t| t.to_work_item(&parked))
            .collect()
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
            return Err(DomainError::Unauthorized(
                unauthorized_reason(response, "usuário ou senha do Mastersys incorretos").await,
            ));
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

        // 2) Catálogo de status, para saber quais status são "parados". Best
        //    effort: sem ele os itens vêm com `status_parked = false`, que é o
        //    comportamento antigo. O catálogo decide rótulo, cor e filtro
        //    padrão — não pode derrubar a sincronização do trabalho em si.
        let parked = self.refresh_status_catalog(&base).await;

        let mut items: Vec<ExternalWorkItem> = Vec::with_capacity(tasks.len());
        let mut ticket_ids_with_task: Vec<i64> = Vec::new();
        for task in &tasks {
            if let Some(ticket_id) = task.ticket_id {
                ticket_ids_with_task.push(ticket_id);
            }
            items.push(task.to_work_item(&parked)?);
        }

        // 3) Chamados atribuídos ao usuário que ainda NÃO têm tarefa no quadro.
        //    Sem esse filtro o mesmo chamado apareceria duas vezes no
        //    MasterNote (uma como tarefa, outra como chamado).
        let tickets = self.fetch_tickets(&base, &user_id).await?;
        for ticket in tickets {
            if ticket_ids_with_task.contains(&ticket.id) {
                continue;
            }
            items.push(ticket.to_work_item(&parked)?);
        }

        Ok(items)
    }
}

/// Percent-encoding de um valor de query string.
///
/// Escrito à mão em vez de somar a crate `percent-encoding`: é a única query
/// dinâmica do provider (as outras são id e data, já seguras por construção), e
/// termo de busca digitado carrega espaço, acento e `&` — concatenar cru
/// produziria URL inválida ou, no caso do `&`, um parâmetro extra inventado.
///
/// Mantém sem escapar apenas os "unreserved" da RFC 3986; todo o resto vira
/// `%XX` sobre os bytes UTF-8, inclusive espaço (`%20`, não `+`, porque isto é
/// query de URL e não corpo de formulário).
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Conjunto de slugs de status que a origem considera parados.
///
/// Existe como tipo próprio para deixar explícito nos `to_work_item` que a
/// ausência de um slug significa "não sei, assuma ativo" — e não "ativo".
#[derive(Debug, Default)]
pub struct ParkedStatuses(std::collections::HashSet<String>);

impl ParkedStatuses {
    fn contains(&self, status: &str) -> bool {
        self.0.contains(status)
    }
}

impl FromIterator<MastersysTicketStatus> for ParkedStatuses {
    fn from_iter<I: IntoIterator<Item = MastersysTicketStatus>>(iter: I) -> Self {
        Self(
            iter.into_iter()
                .filter(|s| s.is_parked())
                .map(|s| s.value)
                .collect(),
        )
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
/// MasterNote dispara na hora certa (CLAUDE §19: cálculo de deadline é
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

/// Prioridade do chamado → prioridade do MasterNote.
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

/// Envelope `{success, data}` usado por `/api/auth/*`.
/// `/api/tasks/users/:id` NÃO usa envelope — ver `parse_json_raw`.
/// `String` tolerante a `null`.
///
/// `#[serde(default)]` cobre campo **ausente**, não `null` explícito — e
/// `tasks.description` é `TEXT` nulo no schema do Mastersys, repassado cru por
/// `TaskRepository.mapToDTO`. Sem isso, uma única tarefa sem descrição fazia a
/// desserialização do array inteiro falhar e a sincronização não trazia nada.
fn null_as_empty<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    data: T,
}

/// Envelope de `/api/tickets/paginated`: `{success, data, pagination}`.
/// `pagination` é ignorado de propósito — a parada é decidida pelo tamanho da
/// página recebida, que não corre risco de mudar no meio da paginação.
#[derive(Debug, Deserialize)]
struct Paginated<T> {
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
    #[serde(default, deserialize_with = "null_as_empty")]
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
    /// Status do CHAMADO vinculado, quando existe.
    ///
    /// Campo acrescentado ao `TaskDTO` do Mastersys em 2026-09-03 justamente
    /// para cá. `Option` e não `String` porque instalação do Mastersys mais
    /// antiga que essa mudança simplesmente não o envia — e o MasterNote tem de
    /// continuar funcionando contra ela, apenas sem a cor e sem detectar item
    /// parado por este caminho.
    #[serde(default)]
    ticket_status: Option<String>,
}

impl MastersysTask {
    /// Status que representa este item para o usuário.
    ///
    /// O do chamado quando há um; o da tarefa quando não. Ver o comentário em
    /// [`MastersysTask::to_work_item`] para o porquê da precedência.
    fn effective_status(&self) -> &str {
        self.ticket_status
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.status)
    }

    fn to_work_item(&self, parked: &ParkedStatuses) -> DomainResult<ExternalWorkItem> {
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
        // Qual status exibir e usar para "parado", quando a tarefa tem chamado.
        //
        // Prefere o status do CHAMADO. Três razões:
        //
        // 1. O item é apresentado como "Chamado" (ver `kind` acima), então
        //    mostrar ao lado dele um slug de tarefa (`in_progress`) é rótulo
        //    errado — o atendente lê "Chamado · em andamento" pensando no
        //    chamado.
        // 2. Status de tarefa NÃO existe em `ticket_statuses`, então o selo
        //    cairia sempre no estilo cinza sem cor. É o status do chamado que
        //    tem cor e rótulo em pt-BR no catálogo.
        // 3. É por aqui que `pos_atendimento` alcança os itens que têm tarefa
        //    no quadro. Antes deste campo existir, só chamados SEM tarefa eram
        //    detectados como parados — a maior parte do quadro ficava de fora.
        //
        // Sem chamado vinculado (ou contra um Mastersys anterior a esta
        // mudança) volta ao status da tarefa, que é o comportamento antigo.
        .with_status_label(Some(self.effective_status().to_string()))
        .with_status_parked(parked.contains(self.effective_status()));

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
        // `canceled` sai do MasterNote: é o mesmo tratamento que a integração
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
    #[serde(default, deserialize_with = "null_as_empty")]
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
    fn to_work_item(&self, parked: &ParkedStatuses) -> DomainResult<ExternalWorkItem> {
        let reference = ExternalRef::new(
            ExternalSystem::Mastersys,
            ExternalKind::Ticket,
            format!("ticket-{}", self.id),
        )?
        .with_client(self.client_name.clone())
        .with_ticket(Some(self.id.to_string()))
        .with_status_label(Some(self.status.clone()))
        // É aqui que `pos_atendimento` deixa de constar como atrasado: o
        // status vem de `ticket_statuses`, o mesmo cadastro que alimenta o
        // catálogo.
        .with_status_parked(parked.contains(&self.status));

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

/// Traduz o motivo real de um 401 do Mastersys para uma frase acionável.
///
/// ## Por que ler o corpo da resposta
///
/// O Mastersys responde `{success:false, error:{message}}`
/// (`shared/infra/http/middlewares/errorHandler.ts`), e o `AuthService` de lá
/// distingue situações que pedem **ações opostas** do usuário:
///
/// - `Invalid credentials` → corrigir e tentar de novo
/// - `User account is inactive` → falar com o administrador; tentar de novo
///   não resolve nunca
/// - `User not found` → a conta deixou de existir
/// - `Invalid refresh token` → reconectar
///
/// Descartar isso e mostrar só "unauthorized" fazia o usuário tentar a mesma
/// coisa repetidamente num caso em que tentar não podia dar certo.
///
/// ## Cuidados
///
/// A mensagem da origem é usada como **chave de tradução**, não repassada crua.
/// Duas razões: ela vem em inglês, e corpo de erro pode conter detalhe interno
/// que não deve ir para a tela (CLAUDE §13/18). Mensagem desconhecida cai no
/// `fallback` do chamador em vez de aparecer como está.
async fn unauthorized_reason(response: reqwest::Response, fallback: &str) -> String {
    let Ok(body) = response.json::<ErrorEnvelope>().await else {
        // Corpo ausente, vazio ou em outro formato: proxy no caminho, ou versão
        // do Mastersys que responde diferente.
        return fallback.to_string();
    };
    let Some(message) = body.error.map(|e| e.message) else {
        return fallback.to_string();
    };

    translate_unauthorized(&message, fallback)
}

/// A tradução em si, separada para ser testável sem construir um
/// `reqwest::Response`.
fn translate_unauthorized(message: &str, fallback: &str) -> String {
    match message.trim().to_ascii_lowercase().as_str() {
        "invalid credentials" => "usuário ou senha do Mastersys incorretos".into(),
        "user account is inactive" => {
            "sua conta no Mastersys está inativa — fale com o administrador; tentar de novo não vai resolver".into()
        }
        "user not found" => {
            "seu usuário não existe mais no Mastersys — fale com o administrador".into()
        }
        "invalid refresh token" => "sua sessão do Mastersys expirou — conecte novamente".into(),
        "no token provided" | "invalid token" => {
            "sua sessão do Mastersys não é mais válida — conecte novamente".into()
        }
        _ => fallback.to_string(),
    }
}

/// Formato de erro do Mastersys: `{success:false, error:{message}}`.
#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    #[serde(default)]
    error: Option<ErrorBody>,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    #[serde(default)]
    message: String,
}

async fn ensure_success(response: reqwest::Response) -> DomainResult<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(DomainError::Unauthorized(
            unauthorized_reason(response, "sua sessão do Mastersys não é mais válida").await,
        ));
    }
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        // `/api/auth/login` tem limite de 10 tentativas por minuto por IP
        // (`users/routes.ts`). O `/refresh` não tem, então a renovação em
        // segundo plano nunca cai aqui — isto é a mensagem do login manual.
        return Err(DomainError::Integration(
            "muitas tentativas em sequência — aguarde um minuto e tente de novo".into(),
        ));
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
        let item = task.to_work_item(&ParkedStatuses::default()).unwrap();

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
        let item = item.to_work_item(&ParkedStatuses::default()).unwrap();

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
                .to_work_item(&ParkedStatuses::default())
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
        let item = ticket.to_work_item(&ParkedStatuses::default()).unwrap();

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
            .to_work_item(&ParkedStatuses::default())
            .unwrap();
        assert!(item.completed);
    }

    #[test]
    fn canceled_ticket_is_removed() {
        let json = r#"{"id":9,"title":"x","status":"cancelado","priority":"low"}"#;
        let item = serde_json::from_str::<MastersysTicket>(json)
            .unwrap()
            .to_work_item(&ParkedStatuses::default())
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
            .to_work_item(&ParkedStatuses::default())
            .unwrap();
        let b = serde_json::from_str::<MastersysTicket>(ticket_json)
            .unwrap()
            .to_work_item(&ParkedStatuses::default())
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
        assert!(task.to_work_item(&ParkedStatuses::default()).is_ok());
    }

    #[test]
    fn null_description_does_not_break_the_whole_sync() {
        // `tasks.description` é TEXT nulo no Mastersys e `mapToDTO` repassa o
        // null cru. Antes desta tolerância, uma tarefa sem descrição fazia o
        // array inteiro falhar na desserialização e o sync não trazia nada.
        let json = r#"{"id":1,"title":"t","status":"pending","description":null}"#;
        let task: MastersysTask = serde_json::from_str(json)
            .expect("description null tem que desserializar, não só ausente");
        assert_eq!(task.description, "");

        // E um array misto continua inteiro, não parcial.
        let list = r#"[
            {"id":1,"title":"a","status":"pending","description":null},
            {"id":2,"title":"b","status":"pending","description":"tem texto"}
        ]"#;
        let tasks: Vec<MastersysTask> = serde_json::from_str(list).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[1].description, "tem texto");
    }

    #[test]
    fn paginated_ticket_envelope_is_parsed_and_pagination_ignored() {
        // Formato de `/api/tickets/paginated`: o `pagination` existe mas não é
        // usado — a parada é pelo tamanho da página recebida.
        let json = r#"{
            "success": true,
            "data": [{"id": 9, "title": "chamado", "description": "d", "status": "novo"}],
            "pagination": {"page":1,"pageSize":200,"total":1,"totalPages":1}
        }"#;
        let page: Paginated<Vec<MastersysTicket>> = serde_json::from_str(json).unwrap();
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.data[0].id, 9);
    }

    /// `ticketStatus` foi acrescentado ao `TaskDTO` do Mastersys em
    /// 2026-09-03 para fechar esta lacuna: antes, itens que chegavam pelo ramo
    /// de TAREFAS nunca eram detectados como parados, porque o status da tarefa
    /// (`pending`/`in_progress`) é outro vocabulário. Como a maior parte do
    /// quadro chega por esse ramo, era a maior parte que ficava de fora.
    #[test]
    fn a_linked_ticket_status_wins_over_the_tasks_own_status() {
        let json = r#"{
            "id": 1, "title": "chamado com tarefa", "status": "in_progress",
            "ticketId": 75071, "ticketStatus": "pos_atendimento"
        }"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.effective_status(), "pos_atendimento");

        let parked: ParkedStatuses = [MastersysTicketStatus {
            value: "pos_atendimento".into(),
            label: "Pós Atendimento".into(),
            color: "#0ea5e9".into(),
            default_filter: false,
            is_final: false,
            pauses_sla: false,
            display_order: 7,
        }]
        .into_iter()
        .collect();

        let item = task.to_work_item(&parked).unwrap();
        assert!(
            item.reference.status_parked,
            "item com chamado em pós-atendimento tem de contar como parado"
        );
        assert_eq!(
            item.reference.status_label.as_deref(),
            Some("pos_atendimento"),
            "o selo mostra o status do CHAMADO, que é o que tem cor no catálogo"
        );
    }

    #[test]
    fn without_a_linked_ticket_the_tasks_own_status_is_used() {
        let json = r#"{"id":2,"title":"tarefa solta","status":"pending"}"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.effective_status(), "pending");
        let item = task.to_work_item(&ParkedStatuses::default()).unwrap();
        assert!(!item.reference.status_parked);
    }

    /// Mastersys anterior a 2026-09-03 não envia `ticketStatus`. O MasterNote
    /// tem de continuar funcionando contra ele — só sem cor e sem detectar
    /// parado por esta via.
    #[test]
    fn an_older_mastersys_without_the_field_still_works() {
        let json = r#"{"id":3,"title":"antigo","status":"in_progress","ticketId":9}"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.ticket_status, None);
        assert_eq!(task.effective_status(), "in_progress");
        assert!(task.to_work_item(&ParkedStatuses::default()).is_ok());
    }

    #[test]
    fn a_blank_ticket_status_falls_back_instead_of_showing_nothing() {
        // String vazia ou só espaços viraria um selo em branco.
        let json = r#"{"id":4,"title":"t","status":"pending","ticketId":9,"ticketStatus":"   "}"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.effective_status(), "pending");
    }

    #[test]
    fn a_null_ticket_status_falls_back_too() {
        // `mapToDTO` manda `?? null` para tarefa sem chamado.
        let json = r#"{"id":5,"title":"t","status":"pending","ticketStatus":null}"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        assert_eq!(task.effective_status(), "pending");
    }

    // -----------------------------------------------------------------------
    // Tradução de 401 — "por que não consegui" em vez de "unauthorized"
    // -----------------------------------------------------------------------

    #[test]
    fn translates_the_reasons_the_origin_actually_returns() {
        let fb = "fallback";
        assert_eq!(
            translate_unauthorized("Invalid credentials", fb),
            "usuário ou senha do Mastersys incorretos"
        );
        // O caso que mais importa: tentar de novo NUNCA resolve, e a mensagem
        // tem de dizer isso, senão o usuário fica repetindo a senha.
        assert!(translate_unauthorized("User account is inactive", fb).contains("inativa"));
        assert!(
            translate_unauthorized("User account is inactive", fb).contains("administrador"),
            "conta inativa precisa direcionar a quem resolve"
        );
        assert!(translate_unauthorized("Invalid refresh token", fb).contains("expirou"));
        assert!(translate_unauthorized("No token provided", fb).contains("conecte novamente"));
    }

    #[test]
    fn reason_matching_ignores_case_and_padding() {
        assert_eq!(
            translate_unauthorized("  INVALID CREDENTIALS  ", "fb"),
            "usuário ou senha do Mastersys incorretos"
        );
    }

    #[test]
    fn an_unknown_reason_falls_back_instead_of_leaking_the_raw_message() {
        // Mensagem desconhecida pode conter detalhe interno; nunca vai crua
        // para a tela (CLAUDE §13/18).
        let out = translate_unauthorized(
            "Some internal detail at /srv/app/x.ts:42",
            "mensagem segura",
        );
        assert_eq!(out, "mensagem segura");
        assert!(!out.contains("/srv"));
    }

    #[test]
    fn an_empty_reason_falls_back() {
        assert_eq!(translate_unauthorized("", "fb"), "fb");
        assert_eq!(translate_unauthorized("   ", "fb"), "fb");
    }

    #[test]
    fn error_envelope_parses_the_shape_the_origin_sends() {
        // `{success:false, error:{message}}` do errorHandler do Mastersys.
        let json = r#"{"success":false,"error":{"message":"User account is inactive"}}"#;
        let parsed: ErrorEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed.error.map(|e| e.message).as_deref(),
            Some("User account is inactive")
        );
    }

    #[test]
    fn error_envelope_tolerates_a_body_without_the_error_key() {
        // Proxy ou versão diferente pode responder outro formato.
        let parsed: ErrorEnvelope = serde_json::from_str(r#"{"success":false}"#).unwrap();
        assert!(parsed.error.is_none());
    }

    #[test]
    fn task_with_blank_title_is_rejected_instead_of_imported() {
        let json = r#"{"id":1,"title":"   ","status":"pending"}"#;
        let task: MastersysTask = serde_json::from_str(json).unwrap();
        assert!(
            task.to_work_item(&ParkedStatuses::default()).is_err(),
            "item sem título viraria uma tarefa sem nome no quadro"
        );
    }
}
