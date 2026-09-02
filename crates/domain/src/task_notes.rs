//! Anotações dentro de uma tarefa.
//!
//! Diferente de `Note` (sticky note independente, com posição/cor/janela
//! própria), uma `TaskNote` é uma entrada de diário **pertencente** a uma
//! `Task`: append-only na prática, ordenada por tempo, sem atributos visuais.
//! É o registro de "o que eu já fiz/descobri nessa tarefa".
//!
//! Por que uma entidade separada em vez de crescer `Task.description`:
//! - `description` é o enunciado da tarefa (e, em itens vindos do Mastersys,
//!   é sobrescrito a cada sincronização). Anotações são do usuário e nunca
//!   podem ser perdidas por um sync.
//! - Cada anotação tem seu próprio `created_at`, o que dá uma linha do tempo.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::TaskId;
use crate::errors::{DomainError, DomainResult};

pub type TaskNoteId = Uuid;

/// Limite de conteúdo alinhado ao de `Note.content`/`Task.description`
/// (20000 caracteres) para o schema ter um CHECK coerente.
pub const MAX_TASK_NOTE_CHARS: usize = 20_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskNote {
    pub id: TaskNoteId,
    pub task_id: TaskId,
    pub content: String,
    /// True quando o usuário marcou a anotação como resolvida/concluída —
    /// permite usar as anotações como checklist sem criar subtarefas.
    pub done: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TaskNote {
    pub fn new(task_id: TaskId, content: impl Into<String>) -> DomainResult<Self> {
        let content = content.into();
        validate_content(&content)?;
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            task_id,
            content: content.trim().to_string(),
            done: false,
            created_at: now,
            updated_at: now,
        })
    }

    /// Reconstrói a partir de dados persistidos (usado por adapters).
    pub fn reconstitute(
        id: TaskNoteId,
        task_id: TaskId,
        content: String,
        done: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        validate_content(&content)?;
        Ok(Self {
            id,
            task_id,
            content: content.trim().to_string(),
            done,
            created_at,
            updated_at,
        })
    }

    pub fn set_content(&mut self, content: impl Into<String>) -> DomainResult<()> {
        let c = content.into();
        validate_content(&c)?;
        self.content = c.trim().to_string();
        self.touch();
        Ok(())
    }

    pub fn set_done(&mut self, done: bool) {
        self.done = done;
        self.touch();
    }

    /// Primeira linha, para usar como resumo em listas colapsadas.
    pub fn summary(&self) -> &str {
        self.content
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
    }

    fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

fn validate_content(content: &str) -> DomainResult<()> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(DomainError::Validation(
            "task note content must not be empty".into(),
        ));
    }
    if trimmed.chars().count() > MAX_TASK_NOTE_CHARS {
        return Err(DomainError::Validation(format!(
            "task note content must be <= {MAX_TASK_NOTE_CHARS} chars"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_id() -> TaskId {
        Uuid::new_v4()
    }

    #[test]
    fn new_trims_and_defaults_to_pending() {
        let n = TaskNote::new(task_id(), "  liguei para o cliente  ").unwrap();
        assert_eq!(n.content, "liguei para o cliente");
        assert!(!n.done);
        assert_eq!(n.created_at, n.updated_at);
    }

    #[test]
    fn empty_content_is_rejected() {
        assert!(TaskNote::new(task_id(), "").is_err());
        assert!(TaskNote::new(task_id(), "   \n\t ").is_err());
    }

    #[test]
    fn oversized_content_is_rejected() {
        let long = "x".repeat(MAX_TASK_NOTE_CHARS + 1);
        assert!(TaskNote::new(task_id(), long).is_err());
        let ok = "x".repeat(MAX_TASK_NOTE_CHARS);
        assert!(TaskNote::new(task_id(), ok).is_ok());
    }

    #[test]
    fn set_content_validates_and_touches() {
        let mut n = TaskNote::new(task_id(), "antes").unwrap();
        let before = n.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(n.set_content("").is_err());
        assert_eq!(
            n.content, "antes",
            "conteúdo inválido não deve ser aplicado"
        );
        n.set_content("depois").unwrap();
        assert_eq!(n.content, "depois");
        assert!(n.updated_at > before);
    }

    #[test]
    fn set_done_toggles() {
        let mut n = TaskNote::new(task_id(), "checar log").unwrap();
        n.set_done(true);
        assert!(n.done);
        n.set_done(false);
        assert!(!n.done);
    }

    #[test]
    fn summary_is_first_non_blank_line() {
        let n = TaskNote::new(task_id(), "\n\n  primeira  \nsegunda").unwrap();
        assert_eq!(n.summary(), "primeira");
    }

    #[test]
    fn summary_of_single_line_is_the_content() {
        let n = TaskNote::new(task_id(), "só isso").unwrap();
        assert_eq!(n.summary(), "só isso");
    }
}
