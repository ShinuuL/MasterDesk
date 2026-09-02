//! `masterdesk-application` — orquestra casos de uso sobre os ports do
//! domínio.

pub mod auth;
pub mod mastersys;
pub mod notes;
pub mod task_notes;
pub mod tasks;

pub use auth::{AuthResult, AuthService, CreateUserInput, LoginInput, UserView};
pub use mastersys::{MastersysSyncService, SyncOptions, SyncReport};
pub use notes::{CreateNoteInput, NoteService, UpdateNoteInput};
pub use task_notes::TaskNoteService;
pub use tasks::{CreateTaskInput, TaskService, UpdateTaskInput};
