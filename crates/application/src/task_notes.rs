//! Casos de uso de anotações dentro de tarefas.
//!
//! Toda operação confirma que a tarefa existe antes de mexer nas anotações —
//! caso contrário o app acumularia anotações órfãs quando a UI trabalhasse com
//! um id obsoleto (uma tarefa deletada em outra janela, por exemplo).

use std::sync::Arc;

use masterdesk_domain::{
    ports::{TaskNoteRepository, TaskRepository},
    DomainError, DomainResult, TaskId, TaskNote, TaskNoteId,
};

pub struct TaskNoteService {
    task_repo: Arc<dyn TaskRepository>,
    note_repo: Arc<dyn TaskNoteRepository>,
}

impl TaskNoteService {
    pub fn new(task_repo: Arc<dyn TaskRepository>, note_repo: Arc<dyn TaskNoteRepository>) -> Self {
        Self {
            task_repo,
            note_repo,
        }
    }

    async fn ensure_task_exists(&self, task_id: TaskId) -> DomainResult<()> {
        self.task_repo
            .find_by_id(task_id)
            .await?
            .map(|_| ())
            .ok_or(DomainError::NotFound)
    }

    pub async fn add_note(
        &self,
        task_id: TaskId,
        content: impl Into<String>,
    ) -> DomainResult<TaskNote> {
        self.ensure_task_exists(task_id).await?;
        let note = TaskNote::new(task_id, content)?;
        self.note_repo.save(&note).await?;
        Ok(note)
    }

    pub async fn list_notes(&self, task_id: TaskId) -> DomainResult<Vec<TaskNote>> {
        self.ensure_task_exists(task_id).await?;
        self.note_repo.list_by_task(task_id).await
    }

    pub async fn count_notes(&self, task_id: TaskId) -> DomainResult<u32> {
        self.note_repo.count_by_task(task_id).await
    }

    /// Contador de todas as tarefas de uma vez — o que o quadro usa.
    pub async fn count_notes_by_task(&self) -> DomainResult<Vec<(TaskId, u32)>> {
        self.note_repo.counts_by_task().await
    }

    pub async fn update_note(
        &self,
        id: TaskNoteId,
        content: impl Into<String>,
    ) -> DomainResult<TaskNote> {
        let mut note = self
            .note_repo
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;
        note.set_content(content)?;
        self.note_repo.save(&note).await?;
        Ok(note)
    }

    pub async fn set_note_done(&self, id: TaskNoteId, done: bool) -> DomainResult<TaskNote> {
        let mut note = self
            .note_repo
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;
        note.set_done(done);
        self.note_repo.save(&note).await?;
        Ok(note)
    }

    pub async fn delete_note(&self, id: TaskNoteId) -> DomainResult<()> {
        if self.note_repo.find_by_id(id).await?.is_none() {
            return Err(DomainError::NotFound);
        }
        self.note_repo.delete(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use masterdesk_domain::{ExternalRef, ExternalSystem, Task};
    use std::collections::HashMap;
    use std::sync::Mutex;

    #[derive(Default)]
    struct InMemoryTasks {
        items: Mutex<HashMap<TaskId, Task>>,
    }

    #[async_trait]
    impl TaskRepository for InMemoryTasks {
        async fn save(&self, task: &Task) -> DomainResult<()> {
            self.items.lock().unwrap().insert(task.id, task.clone());
            Ok(())
        }
        async fn find_by_id(&self, id: TaskId) -> DomainResult<Option<Task>> {
            Ok(self.items.lock().unwrap().get(&id).cloned())
        }
        async fn list_pending(&self) -> DomainResult<Vec<Task>> {
            Ok(Vec::new())
        }
        async fn list_completed(&self) -> DomainResult<Vec<Task>> {
            Ok(Vec::new())
        }
        async fn list_all(&self) -> DomainResult<Vec<Task>> {
            Ok(self.items.lock().unwrap().values().cloned().collect())
        }
        async fn list_overdue(&self) -> DomainResult<Vec<Task>> {
            Ok(Vec::new())
        }
        async fn delete(&self, id: TaskId) -> DomainResult<()> {
            self.items.lock().unwrap().remove(&id);
            Ok(())
        }
        async fn find_by_external(&self, _r: &ExternalRef) -> DomainResult<Option<Task>> {
            Ok(None)
        }
        async fn list_by_external_system(&self, _s: ExternalSystem) -> DomainResult<Vec<Task>> {
            Ok(Vec::new())
        }
    }

    #[derive(Default)]
    struct InMemoryTaskNotes {
        items: Mutex<Vec<TaskNote>>,
    }

    #[async_trait]
    impl TaskNoteRepository for InMemoryTaskNotes {
        async fn save(&self, note: &TaskNote) -> DomainResult<()> {
            let mut items = self.items.lock().unwrap();
            match items.iter_mut().find(|n| n.id == note.id) {
                Some(existing) => *existing = note.clone(),
                None => items.push(note.clone()),
            }
            Ok(())
        }
        async fn find_by_id(&self, id: TaskNoteId) -> DomainResult<Option<TaskNote>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .iter()
                .find(|n| n.id == id)
                .cloned())
        }
        async fn list_by_task(&self, task_id: TaskId) -> DomainResult<Vec<TaskNote>> {
            let mut found: Vec<TaskNote> = self
                .items
                .lock()
                .unwrap()
                .iter()
                .filter(|n| n.task_id == task_id)
                .cloned()
                .collect();
            found.sort_by_key(|n| n.created_at);
            Ok(found)
        }
        async fn count_by_task(&self, task_id: TaskId) -> DomainResult<u32> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .iter()
                .filter(|n| n.task_id == task_id)
                .count() as u32)
        }
        async fn delete(&self, id: TaskNoteId) -> DomainResult<()> {
            self.items.lock().unwrap().retain(|n| n.id != id);
            Ok(())
        }
        async fn counts_by_task(&self) -> DomainResult<Vec<(TaskId, u32)>> {
            let mut acc: HashMap<TaskId, u32> = HashMap::new();
            for n in self.items.lock().unwrap().iter() {
                *acc.entry(n.task_id).or_insert(0) += 1;
            }
            Ok(acc.into_iter().collect())
        }
        async fn delete_by_task(&self, task_id: TaskId) -> DomainResult<()> {
            self.items.lock().unwrap().retain(|n| n.task_id != task_id);
            Ok(())
        }
    }

    async fn service_with_task() -> (TaskNoteService, TaskId) {
        let tasks = Arc::new(InMemoryTasks::default());
        let task = Task::new("tarefa").unwrap();
        tasks.save(&task).await.unwrap();
        let service = TaskNoteService::new(tasks, Arc::new(InMemoryTaskNotes::default()));
        (service, task.id)
    }

    #[tokio::test]
    async fn add_and_list_notes() {
        let (service, task_id) = service_with_task().await;

        service.add_note(task_id, "primeira").await.unwrap();
        service.add_note(task_id, "segunda").await.unwrap();

        let notes = service.list_notes(task_id).await.unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].content, "primeira");
        assert_eq!(service.count_notes(task_id).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn add_note_to_missing_task_is_not_found() {
        let (service, _) = service_with_task().await;
        let ghost = uuid::Uuid::new_v4();
        assert!(matches!(
            service.add_note(ghost, "conteúdo").await,
            Err(DomainError::NotFound)
        ));
    }

    #[tokio::test]
    async fn add_note_rejects_blank_content() {
        let (service, task_id) = service_with_task().await;
        assert!(matches!(
            service.add_note(task_id, "   ").await,
            Err(DomainError::Validation(_))
        ));
        assert_eq!(service.count_notes(task_id).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn update_and_toggle_done() {
        let (service, task_id) = service_with_task().await;
        let note = service.add_note(task_id, "rascunho").await.unwrap();

        let updated = service.update_note(note.id, "versão final").await.unwrap();
        assert_eq!(updated.content, "versão final");

        let done = service.set_note_done(note.id, true).await.unwrap();
        assert!(done.done);
        let undone = service.set_note_done(note.id, false).await.unwrap();
        assert!(!undone.done);
    }

    #[tokio::test]
    async fn update_missing_note_is_not_found() {
        let (service, _) = service_with_task().await;
        let ghost = uuid::Uuid::new_v4();
        assert!(matches!(
            service.update_note(ghost, "x").await,
            Err(DomainError::NotFound)
        ));
        assert!(matches!(
            service.set_note_done(ghost, true).await,
            Err(DomainError::NotFound)
        ));
        assert!(matches!(
            service.delete_note(ghost).await,
            Err(DomainError::NotFound)
        ));
    }

    #[tokio::test]
    async fn delete_note_removes_only_that_one() {
        let (service, task_id) = service_with_task().await;
        let keep = service.add_note(task_id, "fica").await.unwrap();
        let drop = service.add_note(task_id, "sai").await.unwrap();

        service.delete_note(drop.id).await.unwrap();

        let notes = service.list_notes(task_id).await.unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, keep.id);
    }
}
