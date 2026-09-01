//! Casos de uso de Tasks — Fase 3 (Tasks, Deadlines & Notificações).
//! Cada método valida regras de domínio via `Task` e persiste via `TaskRepository`.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use masterdesk_domain::{
    ports::{NotificationService, TaskRepository},
    DomainError, DomainResult, Priority, ReminderThreshold, Task, TaskId,
};

#[derive(Debug, Clone)]
pub struct CreateTaskInput {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub deadline: Option<DateTime<Utc>>,
    pub reminder_thresholds: Option<Vec<ReminderThreshold>>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTaskInput {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<Priority>,
    pub deadline: Option<Option<DateTime<Utc>>>,
    pub reminder_thresholds: Option<Vec<ReminderThreshold>>,
}

pub struct TaskService {
    task_repo: Arc<dyn TaskRepository>,
    notification_service: Option<Arc<dyn NotificationService>>,
}

impl TaskService {
    pub fn new(
        task_repo: Arc<dyn TaskRepository>,
        notification_service: Option<Arc<dyn NotificationService>>,
    ) -> Self {
        Self {
            task_repo,
            notification_service,
        }
    }

    pub async fn create_task(&self, input: CreateTaskInput) -> DomainResult<Task> {
        let mut task = Task::new(input.title)?;
        if let Some(d) = input.description {
            task.set_description(d)?;
        }
        if let Some(p) = input.priority {
            task.set_priority(p);
        }
        if let Some(dl) = input.deadline {
            task.set_deadline(Some(dl));
        }
        if let Some(thresholds) = input.reminder_thresholds {
            task.set_reminder_thresholds(thresholds)?;
        }
        self.task_repo.save(&task).await?;

        // Schedule reminders if we have a notification service and a deadline
        if let Some(ref ns) = self.notification_service {
            self.schedule_reminders_for_task(&task, ns.as_ref()).await;
        }

        Ok(task)
    }

    pub async fn update_task(&self, id: TaskId, input: UpdateTaskInput) -> DomainResult<Task> {
        let mut task = self
            .task_repo
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;

        if let Some(t) = input.title {
            task.set_title(t)?;
        }
        if let Some(d) = input.description {
            task.set_description(d)?;
        }
        if let Some(p) = input.priority {
            task.set_priority(p);
        }
        if let Some(dl) = input.deadline {
            task.set_deadline(dl);
        }
        if let Some(thresholds) = input.reminder_thresholds {
            task.set_reminder_thresholds(thresholds)?;
        }

        self.task_repo.save(&task).await?;

        // Re-schedule reminders after update
        if let Some(ref ns) = self.notification_service {
            // Cancel old reminders first
            let _ = ns.cancel_reminder(task.id).await;
            self.schedule_reminders_for_task(&task, ns.as_ref()).await;
        }

        Ok(task)
    }

    pub async fn complete_task(&self, id: TaskId) -> DomainResult<Task> {
        let mut task = self
            .task_repo
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;
        task.set_completed(true);
        self.task_repo.save(&task).await?;

        // Cancel reminders for completed task
        if let Some(ref ns) = self.notification_service {
            let _ = ns.cancel_reminder(task.id).await;
        }

        Ok(task)
    }

    pub async fn reopen_task(&self, id: TaskId) -> DomainResult<Task> {
        let mut task = self
            .task_repo
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)?;
        task.set_completed(false);
        self.task_repo.save(&task).await?;

        // Re-schedule reminders
        if let Some(ref ns) = self.notification_service {
            self.schedule_reminders_for_task(&task, ns.as_ref()).await;
        }

        Ok(task)
    }

    pub async fn delete_task(&self, id: TaskId) -> DomainResult<()> {
        let exists = self.task_repo.find_by_id(id).await?;
        if exists.is_none() {
            return Err(DomainError::NotFound);
        }

        // Cancel reminders before deleting
        if let Some(ref ns) = self.notification_service {
            let _ = ns.cancel_reminder(id).await;
        }

        self.task_repo.delete(id).await
    }

    pub async fn list_pending_tasks(&self) -> DomainResult<Vec<Task>> {
        self.task_repo.list_pending().await
    }

    pub async fn list_completed_tasks(&self) -> DomainResult<Vec<Task>> {
        self.task_repo.list_completed().await
    }

    pub async fn list_all_tasks(&self) -> DomainResult<Vec<Task>> {
        self.task_repo.list_all().await
    }

    pub async fn list_overdue_tasks(&self) -> DomainResult<Vec<Task>> {
        self.task_repo.list_overdue().await
    }

    pub async fn get_task(&self, id: TaskId) -> DomainResult<Task> {
        self.task_repo
            .find_by_id(id)
            .await?
            .ok_or(DomainError::NotFound)
    }

    /// Schedule reminders for a task. For each threshold, computes
    /// fire_at = deadline - threshold and schedules if still in the future.
    pub async fn schedule_reminders_for_task(
        &self,
        task: &Task,
        notification_service: &dyn NotificationService,
    ) {
        if task.completed || task.deadline.is_none() {
            return;
        }
        if let Some(fire_at) = task.next_reminder_fire_at() {
            let _ = notification_service
                .schedule_reminder(task.id, fire_at)
                .await;
        }
    }

    /// Convenience: snooze a task's reminder by 15 minutes.
    pub async fn snooze_task(&self, id: TaskId) -> DomainResult<()> {
        if let Some(ref ns) = self.notification_service {
            ns.snooze(id, 15).await
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // -----------------------------------------------------------------------
    // In-memory implementations for testing
    // -----------------------------------------------------------------------

    struct InMemoryTaskRepo {
        store: Mutex<HashMap<TaskId, Task>>,
    }

    impl InMemoryTaskRepo {
        fn new() -> Self {
            Self {
                store: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl TaskRepository for InMemoryTaskRepo {
        async fn save(&self, task: &Task) -> DomainResult<()> {
            self.store.lock().unwrap().insert(task.id, task.clone());
            Ok(())
        }
        async fn find_by_id(&self, id: TaskId) -> DomainResult<Option<Task>> {
            Ok(self.store.lock().unwrap().get(&id).cloned())
        }
        async fn list_pending(&self) -> DomainResult<Vec<Task>> {
            Ok(self
                .store
                .lock()
                .unwrap()
                .values()
                .filter(|t| !t.completed)
                .cloned()
                .collect())
        }
        async fn list_completed(&self) -> DomainResult<Vec<Task>> {
            Ok(self
                .store
                .lock()
                .unwrap()
                .values()
                .filter(|t| t.completed)
                .cloned()
                .collect())
        }
        async fn list_all(&self) -> DomainResult<Vec<Task>> {
            Ok(self.store.lock().unwrap().values().cloned().collect())
        }
        async fn list_overdue(&self) -> DomainResult<Vec<Task>> {
            Ok(self
                .store
                .lock()
                .unwrap()
                .values()
                .filter(|t| t.is_overdue())
                .cloned()
                .collect())
        }
        async fn delete(&self, id: TaskId) -> DomainResult<()> {
            self.store.lock().unwrap().remove(&id);
            Ok(())
        }
    }

    struct MockNotificationService {
        scheduled: Mutex<Vec<(TaskId, DateTime<Utc>)>>,
        cancelled: Mutex<Vec<TaskId>>,
    }

    impl MockNotificationService {
        fn new() -> Self {
            Self {
                scheduled: Mutex::new(Vec::new()),
                cancelled: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl NotificationService for MockNotificationService {
        async fn schedule_reminder(
            &self,
            task_id: TaskId,
            fire_at: DateTime<Utc>,
        ) -> DomainResult<()> {
            self.scheduled.lock().unwrap().push((task_id, fire_at));
            Ok(())
        }
        async fn cancel_reminder(&self, task_id: TaskId) -> DomainResult<()> {
            self.cancelled.lock().unwrap().push(task_id);
            Ok(())
        }
        async fn snooze(&self, _task_id: TaskId, _minutes: u32) -> DomainResult<()> {
            Ok(())
        }
    }

    fn setup() -> (
        Arc<dyn TaskRepository>,
        Arc<MockNotificationService>,
        TaskService,
    ) {
        let repo: Arc<dyn TaskRepository> = Arc::new(InMemoryTaskRepo::new());
        let mock = Arc::new(MockNotificationService::new());
        let ns: Arc<dyn NotificationService> = mock.clone();
        let svc = TaskService::new(repo.clone(), Some(ns));
        (repo, mock, svc)
    }

    #[tokio::test]
    async fn create_and_list() {
        let (_, _, svc) = setup();
        let t = svc
            .create_task(CreateTaskInput {
                title: "Buy milk".into(),
                description: Some("2% milk".into()),
                priority: Some(Priority::High),
                deadline: None,
                reminder_thresholds: None,
            })
            .await
            .unwrap();
        assert_eq!(t.title, "Buy milk");
        assert_eq!(t.description, "2% milk");
        assert_eq!(t.priority, Priority::High);

        let pending = svc.list_pending_tasks().await.unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[tokio::test]
    async fn complete_and_reopen() {
        let (_, _, svc) = setup();
        let t = svc
            .create_task(CreateTaskInput {
                title: "task".into(),
                description: None,
                priority: None,
                deadline: None,
                reminder_thresholds: None,
            })
            .await
            .unwrap();

        let completed = svc.complete_task(t.id).await.unwrap();
        assert!(completed.completed);
        assert_eq!(svc.list_pending_tasks().await.unwrap().len(), 0);
        assert_eq!(svc.list_completed_tasks().await.unwrap().len(), 1);

        let reopened = svc.reopen_task(t.id).await.unwrap();
        assert!(!reopened.completed);
    }

    #[tokio::test]
    async fn delete_not_found() {
        let (_, _, svc) = setup();
        let id = uuid::Uuid::new_v4();
        assert!(matches!(
            svc.delete_task(id).await,
            Err(DomainError::NotFound)
        ));
    }

    #[tokio::test]
    async fn update_task() {
        let (_, _, svc) = setup();
        let t = svc
            .create_task(CreateTaskInput {
                title: "old".into(),
                description: None,
                priority: None,
                deadline: None,
                reminder_thresholds: None,
            })
            .await
            .unwrap();

        let updated = svc
            .update_task(
                t.id,
                UpdateTaskInput {
                    title: Some("new".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.title, "new");
    }

    #[tokio::test]
    async fn validation_bubbles() {
        let (_, _, svc) = setup();
        let res = svc
            .create_task(CreateTaskInput {
                title: "".into(),
                description: None,
                priority: None,
                deadline: None,
                reminder_thresholds: None,
            })
            .await;
        assert!(matches!(res, Err(DomainError::Validation(_))));
    }

    #[tokio::test]
    async fn task_overdue_list() {
        let (_, _, svc) = setup();
        let t = svc
            .create_task(CreateTaskInput {
                title: "overdue".into(),
                description: None,
                priority: None,
                deadline: Some(Utc::now() - chrono::Duration::try_minutes(30).unwrap()),
                reminder_thresholds: None,
            })
            .await
            .unwrap();

        let overdue = svc.list_overdue_tasks().await.unwrap();
        assert_eq!(overdue.len(), 1);
        assert_eq!(overdue[0].id, t.id);
    }

    #[tokio::test]
    async fn reminder_scheduled_on_create_with_deadline() {
        let (_, mock, svc) = setup();
        let _t = svc
            .create_task(CreateTaskInput {
                title: "remind me".into(),
                description: None,
                priority: None,
                deadline: Some(Utc::now() + chrono::Duration::try_hours(1).unwrap()),
                reminder_thresholds: Some(vec![
                    ReminderThreshold::Minutes(5),
                    ReminderThreshold::Minutes(15),
                ]),
            })
            .await
            .unwrap();

        // The mock should have one schedule call (next_reminder_fire_at)
        let scheduled = mock.scheduled.lock().unwrap();
        assert_eq!(scheduled.len(), 1);
    }

    #[tokio::test]
    async fn reminder_cancelled_on_complete() {
        let (_, mock, svc) = setup();
        let t = svc
            .create_task(CreateTaskInput {
                title: "will complete".into(),
                description: None,
                priority: None,
                deadline: Some(Utc::now() + chrono::Duration::try_hours(1).unwrap()),
                reminder_thresholds: Some(vec![ReminderThreshold::Minutes(5)]),
            })
            .await
            .unwrap();

        let _ = svc.complete_task(t.id).await.unwrap();

        let cancelled = mock.cancelled.lock().unwrap();
        assert!(cancelled.contains(&t.id));
    }
}
