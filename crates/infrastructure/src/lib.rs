//! `masterdesk-infrastructure` — implementações concretas dos ports do
//! domínio (SQLite via sqlx, plugins do Tauri, etc.).

pub mod local_auth_repository;
pub mod mastersys_provider;
pub mod mastersys_realtime;
pub mod notification_service;
pub mod secret_store;
pub mod sqlite_note_repository;
pub mod sqlite_settings_repository;
pub mod sqlite_status_catalog_repository;
pub mod sqlite_task_note_repository;
pub mod sqlite_task_repository;
pub mod sqlite_task_window_repository;

pub use local_auth_repository::LocalAuthRepository;
pub use mastersys_provider::MastersysProvider;
pub use mastersys_realtime::RealtimeConnection;
pub use notification_service::NotificationService;
pub use secret_store::{SecretKey, SecretStore, SecretStoreError};
pub use sqlite_note_repository::SqliteNoteRepository;
pub use sqlite_settings_repository::{SettingKey, SqliteSettingsRepository};
pub use sqlite_status_catalog_repository::{MastersysTicketStatus, SqliteStatusCatalogRepository};
pub use sqlite_task_note_repository::SqliteTaskNoteRepository;
pub use sqlite_task_repository::SqliteTaskRepository;
pub use sqlite_task_window_repository::{SqliteTaskWindowRepository, TaskWindowState};
