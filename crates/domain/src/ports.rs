//! Ports (traits) que a camada `application` consome e que `infrastructure`
//! implementa. Nenhuma implementação real mora aqui — apenas os contratos.
//! Ver seção 4 do CLAUDE.md.

use async_trait::async_trait;

use crate::entities::{Note, NoteId, Task, TaskId, User};
use crate::errors::DomainResult;

#[async_trait]
pub trait NoteRepository: Send + Sync {
    async fn save(&self, note: &Note) -> DomainResult<()>;
    async fn find_by_id(&self, id: NoteId) -> DomainResult<Option<Note>>;
    async fn list_active(&self) -> DomainResult<Vec<Note>>;
    async fn list_archived(&self) -> DomainResult<Vec<Note>>;
    async fn list_all(&self) -> DomainResult<Vec<Note>>;
    async fn delete(&self, id: NoteId) -> DomainResult<()>;
}

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn save(&self, task: &Task) -> DomainResult<()>;
    async fn find_by_id(&self, id: TaskId) -> DomainResult<Option<Task>>;
    async fn list_pending(&self) -> DomainResult<Vec<Task>>;
    async fn list_completed(&self) -> DomainResult<Vec<Task>>;
    async fn list_all(&self) -> DomainResult<Vec<Task>>;
    async fn list_overdue(&self) -> DomainResult<Vec<Task>>;
    async fn delete(&self, id: TaskId) -> DomainResult<()>;
}

/// Agendamento/disparo de lembretes. A implementação concreta (Fase 3, ADR-004)
/// decide se usa `tauri-plugin-notification` puro ou o fork comunitário — o
/// domínio não sabe disso.
#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn schedule_reminder(
        &self,
        task_id: TaskId,
        fire_at: chrono::DateTime<chrono::Utc>,
    ) -> DomainResult<()>;
    async fn cancel_reminder(&self, task_id: TaskId) -> DomainResult<()>;
    async fn snooze(&self, task_id: TaskId, minutes: u32) -> DomainResult<()>;
}

/// Controle de janela (posição, always-on-top, opacidade). Implementação real
/// depende da API do Tauri e é OS-specific (ver ADR-002, Fase 2).
pub trait WindowService: Send + Sync {
    fn set_always_on_top(&self, note_id: NoteId, enabled: bool) -> DomainResult<()>;
    fn set_opacity(&self, note_id: NoteId, opacity: f32) -> DomainResult<()>;
    fn set_position(&self, note_id: NoteId, x: f64, y: f64) -> DomainResult<()>;
}

/// Autenticação local (Fase 4) — isolada do Mastersys. O mecanismo externo de
/// autenticação (Mastersys) entra na Fase 5 (ADR-006); este port permanece como
/// contrato genérico e a implementação local é trocável.
///
/// Contrato (Fase 4):
/// - `register`: cria um usuário local (senha é hasheada pela infraestrutura via
///   Argon2 — o domínio não vê plaintext além da entrada).
/// - `login`: verifica credenciais e abre sessão em memória.
/// - `logout`: encerra a sessão.
/// - `is_authenticated`: consulta se há sessão ativa.
///
/// A implementação decide a política de senha (Argon2) e o armazenamento; o
/// domínio apenas valida formato de username/senha em `User`.
#[async_trait]
pub trait AuthenticationProvider: Send + Sync {
    /// Registra um novo usuário local. Retorna `DomainError::Conflict` se o
    /// username já existir.
    async fn register(&self, username: &str, password: &str) -> DomainResult<User>;

    /// Autentica um usuário. Retorna `DomainError::Unauthorized` para credenciais
    /// inválidas.
    async fn login(&self, username: &str, password: &str) -> DomainResult<User>;

    /// Encerra a sessão atual (não-falha se não houver sessão).
    async fn logout(&self) -> DomainResult<()>;

    /// True se há uma sessão autenticada ativa em memória.
    async fn is_authenticated(&self) -> DomainResult<bool>;
}

/// Integração com sistema de suporte (Mastersys ou futuro). Nenhuma
/// implementação concreta antes de ADR-006 (Fase 5) — ver seção 10 do
/// CLAUDE.md: nunca implementar chamada real sem contrato validado.
#[async_trait]
pub trait SupportSystemProvider: Send + Sync {
    async fn is_configured(&self) -> bool;
}

/// Provider de IA — papel consultivo apenas (seção 12 do CLAUDE.md).
/// Bloqueado até ADR-007 (Fase 6).
#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn is_configured(&self) -> bool;
}
