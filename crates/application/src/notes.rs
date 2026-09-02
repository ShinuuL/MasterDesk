//! Casos de uso de Notes — Fase 2 (Local Notes).
//! Cada método valida regras de domínio via `Note` e persiste via `NoteRepository`.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use masterdesk_domain::{ports::NoteRepository, DomainError, DomainResult, Note, NoteId, Priority};

#[derive(Debug, Clone)]
pub struct CreateNoteInput {
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub priority: Option<Priority>,
    pub color: Option<String>,
    pub opacity: Option<f32>,
    pub position: Option<(f64, f64)>,
    pub size: Option<(f64, f64)>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateNoteInput {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub priority: Option<Priority>,
    pub color: Option<String>,
    pub opacity: Option<f32>,
    pub deadline: Option<Option<DateTime<Utc>>>,
    pub position: Option<(f64, f64)>,
    pub size: Option<(f64, f64)>,
    pub pinned: Option<bool>,
    pub always_on_top: Option<bool>,
}

pub struct NoteService {
    repository: Arc<dyn NoteRepository>,
}

impl NoteService {
    pub fn new(repository: Arc<dyn NoteRepository>) -> Self {
        Self { repository }
    }

    pub async fn create_note(&self, input: CreateNoteInput) -> DomainResult<Note> {
        let mut note = Note::new(input.title, input.content)?;
        if let Some(tags) = (!input.tags.is_empty()).then_some(input.tags) {
            note.set_tags(tags)?;
        }
        if let Some(p) = input.priority {
            note.set_priority(p);
        }
        if let Some(c) = input.color {
            note.set_color(c)?;
        }
        if let Some(o) = input.opacity {
            note.set_opacity(o)?;
        }
        if let Some(pos) = input.position {
            note.set_position(pos.0, pos.1)?;
        }
        if let Some(s) = input.size {
            note.set_size(s.0, s.1)?;
        }
        self.repository.save(&note).await?;
        Ok(note)
    }

    pub async fn update_note(&self, id: NoteId, input: UpdateNoteInput) -> DomainResult<Note> {
        let mut note = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;

        if let Some(t) = input.title {
            note.set_title(t)?;
        }
        if let Some(c) = input.content {
            note.set_content(c)?;
        }
        if let Some(tags) = input.tags {
            note.set_tags(tags)?;
        }
        if let Some(p) = input.priority {
            note.set_priority(p);
        }
        if let Some(col) = input.color {
            note.set_color(col)?;
        }
        if let Some(op) = input.opacity {
            note.set_opacity(op)?;
        }
        if let Some(dl) = input.deadline {
            note.set_deadline(dl);
        }
        if let Some(pos) = input.position {
            note.set_position(pos.0, pos.1)?;
        }
        if let Some(s) = input.size {
            note.set_size(s.0, s.1)?;
        }
        if let Some(pinned) = input.pinned {
            note.set_pinned(pinned);
        }
        if let Some(aot) = input.always_on_top {
            note.set_always_on_top(aot);
        }

        self.repository.save(&note).await?;
        Ok(note)
    }

    pub async fn archive_note(&self, id: NoteId) -> DomainResult<Note> {
        let mut note = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;
        note.archive();
        self.repository.save(&note).await?;
        Ok(note)
    }

    pub async fn unarchive_note(&self, id: NoteId) -> DomainResult<Note> {
        let mut note = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;
        note.unarchive();
        self.repository.save(&note).await?;
        Ok(note)
    }

    pub async fn delete_note(&self, id: NoteId) -> DomainResult<()> {
        // Verifica existência primeiro para retornar NotFound consistente
        let exists = self.repository.find_by_id(id).await?;
        if exists.is_none() {
            return Err(DomainError::NotFound);
        }
        self.repository.delete(id).await
    }

    pub async fn toggle_pin(&self, id: NoteId) -> DomainResult<Note> {
        let mut note = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;
        let new_val = !note.pinned;
        note.set_pinned(new_val);
        self.repository.save(&note).await?;
        Ok(note)
    }

    pub async fn set_always_on_top(&self, id: NoteId, enabled: bool) -> DomainResult<Note> {
        let mut note = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;
        note.set_always_on_top(enabled);
        self.repository.save(&note).await?;
        Ok(note)
    }

    pub async fn list_active_notes(&self) -> DomainResult<Vec<Note>> {
        self.repository.list_active().await
    }

    pub async fn list_archived_notes(&self) -> DomainResult<Vec<Note>> {
        self.repository.list_archived().await
    }

    pub async fn list_all_notes(&self) -> DomainResult<Vec<Note>> {
        self.repository.list_all().await
    }

    pub async fn get_note(&self, id: NoteId) -> DomainResult<Note> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    struct InMemoryRepo {
        store: Mutex<HashMap<NoteId, Note>>,
    }

    impl InMemoryRepo {
        fn new() -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl NoteRepository for InMemoryRepo {
        async fn save(&self, note: &Note) -> DomainResult<()> {
            self.store.lock().unwrap().insert(note.id, note.clone());
            Ok(())
        }
        async fn find_by_id(&self, id: NoteId) -> DomainResult<Option<Note>> {
            Ok(self.store.lock().unwrap().get(&id).cloned())
        }
        async fn list_active(&self) -> DomainResult<Vec<Note>> {
            Ok(self
                .store
                .lock()
                .unwrap()
                .values()
                .filter(|n| n.is_active())
                .cloned()
                .collect())
        }
        async fn list_archived(&self) -> DomainResult<Vec<Note>> {
            Ok(self
                .store
                .lock()
                .unwrap()
                .values()
                .filter(|n| n.archived)
                .cloned()
                .collect())
        }
        async fn list_all(&self) -> DomainResult<Vec<Note>> {
            Ok(self.store.lock().unwrap().values().cloned().collect())
        }
        async fn delete(&self, id: NoteId) -> DomainResult<()> {
            self.store.lock().unwrap().remove(&id);
            Ok(())
        }
    }

    fn repo() -> Arc<dyn NoteRepository> {
        Arc::new(InMemoryRepo::new())
    }

    #[tokio::test]
    async fn create_and_list() {
        let svc = NoteService::new(repo());
        let n = svc
            .create_note(CreateNoteInput {
                title: "t".into(),
                content: "c".into(),
                tags: vec!["rust".into()],
                priority: Some(Priority::High),
                color: None,
                opacity: None,
                position: None,
                size: None,
            })
            .await
            .unwrap();
        assert_eq!(n.title, "t");
        let all = svc.list_active_notes().await.unwrap();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn update_and_archive() {
        let svc = NoteService::new(repo());
        let n = svc
            .create_note(CreateNoteInput {
                title: "t".into(),
                content: "c".into(),
                tags: vec![],
                priority: None,
                color: None,
                opacity: None,
                position: None,
                size: None,
            })
            .await
            .unwrap();
        let updated = svc
            .update_note(
                n.id,
                UpdateNoteInput {
                    title: Some("novo".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.title, "novo");

        svc.archive_note(n.id).await.unwrap();
        assert_eq!(svc.list_active_notes().await.unwrap().len(), 0);
        assert_eq!(svc.list_archived_notes().await.unwrap().len(), 1);

        svc.unarchive_note(n.id).await.unwrap();
        assert_eq!(svc.list_active_notes().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn delete_not_found() {
        let svc = NoteService::new(repo());
        let id = uuid::Uuid::new_v4();
        assert!(matches!(
            svc.delete_note(id).await,
            Err(DomainError::NotFound)
        ));
    }

    #[tokio::test]
    async fn validation_bubbles() {
        let svc = NoteService::new(repo());
        let res = svc
            .create_note(CreateNoteInput {
                title: "".into(),
                content: "c".into(),
                tags: vec![],
                priority: None,
                color: None,
                opacity: None,
                position: None,
                size: None,
            })
            .await;
        assert!(matches!(res, Err(DomainError::Validation(_))));
    }
}
