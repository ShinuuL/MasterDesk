//! `masterdesk-infrastructure` — implementações concretas dos ports do
//! domínio (SQLite via sqlx, plugins do Tauri, etc.).

pub mod local_auth_repository;
pub mod mastersys_provider;
pub mod notification_service;
pub mod secret_store;
pub mod sqlite_note_repository;
pub mod sqlite_settings_repository;
pub mod sqlite_task_note_repository;
pub mod sqlite_task_repository;

pub use local_auth_repository::LocalAuthRepository;
pub use mastersys_provider::MastersysProvider;
pub use notification_service::NotificationService;
pub use secret_store::{SecretKey, SecretStore, SecretStoreError};
pub use sqlite_note_repository::SqliteNoteRepository;
pub use sqlite_settings_repository::{SettingKey, SqliteSettingsRepository};
pub use sqlite_task_note_repository::SqliteTaskNoteRepository;
pub use sqlite_task_repository::SqliteTaskRepository;
