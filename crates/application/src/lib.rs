//! `masterdesk-application` — orquestra casos de uso sobre os ports do
//! domínio.

pub mod notes;
pub mod tasks;

pub use notes::{CreateNoteInput, NoteService, UpdateNoteInput};
pub use tasks::{CreateTaskInput, TaskService, UpdateTaskInput};
