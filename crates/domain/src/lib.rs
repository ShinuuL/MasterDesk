//! `masterdesk-domain` — regras de negócio puras. Sem I/O, sem framework,
//! sem dependência de Tauri/sqlx/http. Ver seção 4 do CLAUDE.md.

pub mod entities;
pub mod errors;
pub mod ports;

pub use entities::*;
pub use errors::{DomainError, DomainResult};
