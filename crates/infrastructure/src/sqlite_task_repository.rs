//! `SqliteTaskRepository` — implementação real do port `TaskRepository`
//! sobre SQLite via sqlx (ADR-003). Não vaza `sqlx::Error` para o domínio;
//! mapeia tudo para `DomainError` na borda.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use masterdesk_domain::{
    ports::TaskRepository, DomainError, DomainResult, Priority, ReminderThreshold, Task, TaskId,
};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SqliteTaskRepository {
    pool: SqlitePool,
}

impl SqliteTaskRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

// ---------------------------------------------------------------------------
// Helpers de mapeamento DB <-> Domain
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct TaskRow {
    id: String,
    title: String,
    description: String,
    priority: String,
    deadline: Option<String>,
    reminder_thresholds: String,
    completed: i64,
    created_at: String,
    updated_at: String,
}

fn parse_priority(s: &str) -> Priority {
    match s {
        "Low" => Priority::Low,
        "Medium" => Priority::Medium,
        "High" => Priority::High,
        "Urgent" => Priority::Urgent,
        _ => Priority::Medium,
    }
}

fn priority_to_str(p: Priority) -> &'static str {
    match p {
        Priority::Low => "Low",
        Priority::Medium => "Medium",
        Priority::High => "High",
        Priority::Urgent => "Urgent",
    }
}

fn row_to_task(row: TaskRow) -> DomainResult<Task> {
    let id = Uuid::parse_str(&row.id).map_err(|_| DomainError::Persistence)?;
    let priority = parse_priority(&row.priority);
    let deadline: Option<DateTime<Utc>> = row
        .deadline
        .as_deref()
        .map(|s| {
            s.parse::<DateTime<Utc>>()
                .map_err(|_| DomainError::Persistence)
        })
        .transpose()?;
    let reminder_thresholds: Vec<ReminderThreshold> =
        serde_json::from_str(&row.reminder_thresholds).map_err(|_| DomainError::Persistence)?;
    let created_at = row
        .created_at
        .parse::<DateTime<Utc>>()
        .map_err(|_| DomainError::Persistence)?;
    let updated_at = row
        .updated_at
        .parse::<DateTime<Utc>>()
        .map_err(|_| DomainError::Persistence)?;

    Task::reconstitute(
        id,
        row.title,
        row.description,
        priority,
        deadline,
        reminder_thresholds,
        row.completed != 0,
        created_at,
        updated_at,
    )
}

fn map_sqlx_err(_e: sqlx::Error) -> DomainError {
    // Nunca vazar detalhes de SQL para o domínio / UI (seção 17/18 CLAUDE.md)
    DomainError::Persistence
}

// ---------------------------------------------------------------------------
// Trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl TaskRepository for SqliteTaskRepository {
    async fn save(&self, task: &Task) -> DomainResult<()> {
        let thresholds_json = serde_json::to_string(&task.reminder_thresholds)
            .map_err(|_| DomainError::Persistence)?;
        let deadline_str = task.deadline.map(|d| d.to_rfc3339());
        let now_str = task.updated_at.to_rfc3339();
        let created_str = task.created_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO tasks (
                id, title, description, priority, deadline,
                reminder_thresholds, completed, created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9
            )
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                description = excluded.description,
                priority = excluded.priority,
                deadline = excluded.deadline,
                reminder_thresholds = excluded.reminder_thresholds,
                completed = excluded.completed,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(task.id.to_string())
        .bind(&task.title)
        .bind(&task.description)
        .bind(priority_to_str(task.priority))
        .bind(deadline_str)
        .bind(thresholds_json)
        .bind(if task.completed { 1i64 } else { 0i64 })
        .bind(created_str)
        .bind(now_str)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        Ok(())
    }

    async fn find_by_id(&self, id: TaskId) -> DomainResult<Option<Task>> {
        let row: Option<TaskRow> = sqlx::query_as("SELECT * FROM tasks WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        match row {
            Some(r) => Ok(Some(row_to_task(r)?)),
            None => Ok(None),
        }
    }

    async fn list_pending(&self) -> DomainResult<Vec<Task>> {
        let rows: Vec<TaskRow> =
            sqlx::query_as("SELECT * FROM tasks WHERE completed = 0 ORDER BY deadline IS NULL, deadline ASC, updated_at DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
        rows.into_iter().map(row_to_task).collect()
    }

    async fn list_completed(&self) -> DomainResult<Vec<Task>> {
        let rows: Vec<TaskRow> =
            sqlx::query_as("SELECT * FROM tasks WHERE completed = 1 ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
        rows.into_iter().map(row_to_task).collect()
    }

    async fn list_all(&self) -> DomainResult<Vec<Task>> {
        let rows: Vec<TaskRow> = sqlx::query_as("SELECT * FROM tasks ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        rows.into_iter().map(row_to_task).collect()
    }

    async fn list_overdue(&self) -> DomainResult<Vec<Task>> {
        // overdue = not completed AND deadline not null AND deadline <= now
        let now = Utc::now().to_rfc3339();
        let rows: Vec<TaskRow> = sqlx::query_as(
            "SELECT * FROM tasks WHERE completed = 0 AND deadline IS NOT NULL AND deadline <= ?1 ORDER BY deadline ASC",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        rows.into_iter().map(row_to_task).collect()
    }

    async fn delete(&self, id: TaskId) -> DomainResult<()> {
        sqlx::query("DELETE FROM tasks WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn init_schema(pool: &SqlitePool) {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '',
                priority TEXT NOT NULL DEFAULT 'Medium',
                deadline TEXT,
                reminder_thresholds TEXT NOT NULL DEFAULT '[]',
                completed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn fresh_repo() -> SqliteTaskRepository {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_schema(&pool).await;
        SqliteTaskRepository::new(pool)
    }

    #[tokio::test]
    async fn crud_roundtrip() {
        let repo = fresh_repo().await;
        let mut task = masterdesk_domain::Task::new("Comprar leite").unwrap();
        task.set_description("desnatado").unwrap();
        task.set_reminder_thresholds(vec![
            ReminderThreshold::Minutes(5),
            ReminderThreshold::Hours(1),
        ])
        .unwrap();
        let deadline = Utc::now() + chrono::Duration::try_hours(2).unwrap();
        task.set_deadline(Some(deadline));
        task.set_priority(masterdesk_domain::Priority::High);
        repo.save(&task).await.unwrap();

        let fetched = repo.find_by_id(task.id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Comprar leite");
        assert_eq!(fetched.description, "desnatado");
        assert_eq!(fetched.reminder_thresholds.len(), 2);
        assert_eq!(
            fetched.reminder_thresholds[0].as_minutes(),
            ReminderThreshold::Minutes(5).as_minutes()
        );
        assert!(fetched.deadline.is_some());
        assert_eq!(fetched.priority, masterdesk_domain::Priority::High);
        assert!(!fetched.completed);

        // pending list
        assert_eq!(repo.list_pending().await.unwrap().len(), 1);
        assert_eq!(repo.list_completed().await.unwrap().len(), 0);

        // update to completed
        let mut updated = fetched.clone();
        updated.set_completed(true);
        repo.save(&updated).await.unwrap();
        assert_eq!(repo.list_pending().await.unwrap().len(), 0);
        assert_eq!(repo.list_completed().await.unwrap().len(), 1);
        assert_eq!(repo.list_all().await.unwrap().len(), 1);

        // delete
        repo.delete(task.id).await.unwrap();
        assert!(repo.find_by_id(task.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_overdue_filters() {
        let repo = fresh_repo().await;

        // Past deadline
        let mut overdue = masterdesk_domain::Task::new("atrasada").unwrap();
        overdue.set_deadline(Some(
            Utc::now() - chrono::Duration::try_minutes(30).unwrap(),
        ));
        repo.save(&overdue).await.unwrap();

        // Future deadline
        let mut future = masterdesk_domain::Task::new("futura").unwrap();
        future.set_deadline(Some(Utc::now() + chrono::Duration::try_hours(1).unwrap()));
        repo.save(&future).await.unwrap();

        // No deadline
        let nodeadline = masterdesk_domain::Task::new("sem deadline").unwrap();
        repo.save(&nodeadline).await.unwrap();

        let overdue_list = repo.list_overdue().await.unwrap();
        assert_eq!(overdue_list.len(), 1);
        assert_eq!(overdue_list[0].id, overdue.id);
    }
}
