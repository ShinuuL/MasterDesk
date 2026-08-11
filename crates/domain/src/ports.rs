//! Ports (traits) que a camada `application` consome e que `infrastructure`
//! implementa. Nenhuma implementação real mora aqui — apenas os contratos.
//! Ver seção 4 do CLAUDE.md.

use async_trait::async_trait;

use crate::entities::{Note, NoteId, Task, TaskId};
use crate::errors::DomainResult;

#[async_trait]
pub trait NoteRepository: Send + Sync {
    async fn save(&self, note: &Note) -> DomainResult<()>;
    async fn find_by_id(&self, id: NoteId) -> DomainResult<Option<Note>>;
    async fn list_active(&self) -> DomainResult<Vec<Note>>;
    async fn delete(&self, id: NoteId) -> DomainResult<()>;
}

#[async_trait]
pub trait TaskRepository: Send + Sync {
    async fn save(&self, task: &Task) -> DomainResult<()>;
    async fn find_by_id(&self, id: TaskId) -> DomainResult<Option<Task>>;
    async fn list_pending(&self) -> DomainResult<Vec<Task>>;
    async fn delete(&self, id: TaskId) -> DomainResult<()>;
}

/// Agendamento/disparo de lembretes. A implementação concreta (Fase 3, ADR-004)
/// decide se usa `tauri-plugin-notification` puro ou o fork comunitário — o
/// domínio não sabe disso.
#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn schedule_reminder(&self, task_id: TaskId, fire_at: chrono::DateTime<chrono::Utc>) -> DomainResult<()>;
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

/// Autenticação. Mecanismo real bloqueado até ADR-005 (Fase 4).
#[async_trait]
pub trait AuthenticationProvider: Send + Sync {
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
