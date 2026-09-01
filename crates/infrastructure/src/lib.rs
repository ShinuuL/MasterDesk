//! `masterdesk-infrastructure` — implementações concretas dos ports do
//! domínio (SQLite via sqlx, plugins do Tauri, etc.).

pub mod local_auth_repository;
pub mod notification_service;
pub mod sqlite_note_repository;
pub mod sqlite_task_repository;

pub use local_auth_repository::LocalAuthRepository;
pub use notification_service::NotificationService;
pub use sqlite_note_repository::SqliteNoteRepository;
pub use sqlite_task_repository::SqliteTaskRepository;
