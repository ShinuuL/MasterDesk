//! `masterdesk-domain` — regras de negócio puras. Sem I/O, sem framework,
//! sem dependência de Tauri/sqlx/http. Ver seção 4 do CLAUDE.md.

pub mod entities;
pub mod errors;
pub mod external;
pub mod ports;
pub mod task_notes;

pub use entities::*;
pub use errors::{DomainError, DomainResult};
pub use external::{ExternalKind, ExternalRef, ExternalSystem, ExternalWorkItem, SupportIdentity};
pub use task_notes::{TaskNote, TaskNoteId, MAX_TASK_NOTE_CHARS};
