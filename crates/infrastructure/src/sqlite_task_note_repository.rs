//! `SqliteTaskNoteRepository` — implementação do port `TaskNoteRepository`
//! sobre SQLite via sqlx (ADR-003). Não vaza `sqlx::Error` para o domínio.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use masterdesk_domain::{
    ports::TaskNoteRepository, DomainError, DomainResult, TaskId, TaskNote, TaskNoteId,
};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SqliteTaskNoteRepository {
    pool: SqlitePool,
}

impl SqliteTaskNoteRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[derive(Debug, sqlx::FromRow)]
struct TaskNoteRow {
    id: String,
    task_id: String,
    content: String,
    done: i64,
    created_at: String,
    updated_at: String,
}

fn row_to_task_note(row: TaskNoteRow) -> DomainResult<TaskNote> {
    let id = Uuid::parse_str(&row.id).map_err(|_| DomainError::Persistence)?;
    let task_id = Uuid::parse_str(&row.task_id).map_err(|_| DomainError::Persistence)?;
    let created_at = row
        .created_at
        .parse::<DateTime<Utc>>()
        .map_err(|_| DomainError::Persistence)?;
    let updated_at = row
        .updated_at
        .parse::<DateTime<Utc>>()
        .map_err(|_| DomainError::Persistence)?;

    TaskNote::reconstitute(
        id,
        task_id,
        row.content,
        row.done != 0,
        created_at,
        updated_at,
    )
}

fn map_sqlx_err(_e: sqlx::Error) -> DomainError {
    // Nunca vazar detalhes de SQL para o domínio / UI (seção 17/18 CLAUDE.md)
    DomainError::Persistence
}

#[async_trait]
impl TaskNoteRepository for SqliteTaskNoteRepository {
    async fn save(&self, note: &TaskNote) -> DomainResult<()> {
        sqlx::query(
            r#"
            INSERT INTO task_notes (id, task_id, content, done, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
                content = excluded.content,
                done = excluded.done,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(note.id.to_string())
        .bind(note.task_id.to_string())
        .bind(&note.content)
        .bind(if note.done { 1i64 } else { 0i64 })
        .bind(note.created_at.to_rfc3339())
        .bind(note.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        Ok(())
    }

    async fn find_by_id(&self, id: TaskNoteId) -> DomainResult<Option<TaskNote>> {
        let row: Option<TaskNoteRow> = sqlx::query_as("SELECT * FROM task_notes WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        match row {
            Some(r) => Ok(Some(row_to_task_note(r)?)),
            None => Ok(None),
        }
    }

    async fn list_by_task(&self, task_id: TaskId) -> DomainResult<Vec<TaskNote>> {
        // `id` como desempate: duas anotações criadas no mesmo milissegundo
        // precisam de ordem estável, senão a lista "pula" entre renders.
        let rows: Vec<TaskNoteRow> = sqlx::query_as(
            "SELECT * FROM task_notes WHERE task_id = ?1 ORDER BY created_at ASC, id ASC",
        )
        .bind(task_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        rows.into_iter().map(row_to_task_note).collect()
    }

    async fn count_by_task(&self, task_id: TaskId) -> DomainResult<u32> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM task_notes WHERE task_id = ?1")
            .bind(task_id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        Ok(count.max(0) as u32)
    }

    async fn counts_by_task(&self) -> DomainResult<Vec<(TaskId, u32)>> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT task_id, COUNT(*) FROM task_notes GROUP BY task_id")
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
        Ok(rows
            .into_iter()
            // Id ilegível é linha órfã de um schema anterior: ignorada em vez
            // de derrubar o contador do quadro inteiro.
            .filter_map(|(id, n)| TaskId::parse_str(&id).ok().map(|id| (id, n.max(0) as u32)))
            .collect())
    }

    async fn delete(&self, id: TaskNoteId) -> DomainResult<()> {
        sqlx::query("DELETE FROM task_notes WHERE id = ?1")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        Ok(())
    }

    async fn delete_by_task(&self, task_id: TaskId) -> DomainResult<()> {
        sqlx::query("DELETE FROM task_notes WHERE task_id = ?1")
            .bind(task_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use masterdesk_domain::{ports::TaskRepository, Task};

    use crate::sqlite_task_repository::SqliteTaskRepository;

    async fn fixtures() -> (SqliteTaskNoteRepository, SqliteTaskRepository, Task) {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        let task_repo = SqliteTaskRepository::new(pool.clone());
        let task = Task::new("tarefa com anotações").unwrap();
        task_repo.save(&task).await.unwrap();
        (SqliteTaskNoteRepository::new(pool), task_repo, task)
    }

    #[tokio::test]
    async fn crud_roundtrip() {
        let (repo, _tasks, task) = fixtures().await;

        let mut note = TaskNote::new(task.id, "liguei para o cliente").unwrap();
        repo.save(&note).await.unwrap();

        let fetched = repo.find_by_id(note.id).await.unwrap().unwrap();
        assert_eq!(fetched.content, "liguei para o cliente");
        assert_eq!(fetched.task_id, task.id);
        assert!(!fetched.done);

        note.set_done(true);
        note.set_content("liguei — vai retornar amanhã").unwrap();
        repo.save(&note).await.unwrap();

        let updated = repo.find_by_id(note.id).await.unwrap().unwrap();
        assert!(updated.done);
        assert_eq!(updated.content, "liguei — vai retornar amanhã");
        assert_eq!(
            updated.created_at, fetched.created_at,
            "created_at é imutável no upsert"
        );

        repo.delete(note.id).await.unwrap();
        assert!(repo.find_by_id(note.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_is_chronological_and_scoped_to_the_task() {
        let (repo, tasks, task_a) = fixtures().await;
        let task_b = Task::new("outra tarefa").unwrap();
        tasks.save(&task_b).await.unwrap();

        for text in ["primeira", "segunda", "terceira"] {
            repo.save(&TaskNote::new(task_a.id, text).unwrap())
                .await
                .unwrap();
        }
        repo.save(&TaskNote::new(task_b.id, "de outra").unwrap())
            .await
            .unwrap();

        let listed = repo.list_by_task(task_a.id).await.unwrap();
        assert_eq!(listed.len(), 3);
        assert_eq!(
            listed
                .iter()
                .map(|n| n.content.as_str())
                .collect::<Vec<_>>(),
            vec!["primeira", "segunda", "terceira"]
        );
        assert_eq!(repo.count_by_task(task_a.id).await.unwrap(), 3);
        assert_eq!(repo.count_by_task(task_b.id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn delete_by_task_clears_only_that_task() {
        let (repo, tasks, task_a) = fixtures().await;
        let task_b = Task::new("outra").unwrap();
        tasks.save(&task_b).await.unwrap();
        repo.save(&TaskNote::new(task_a.id, "a").unwrap())
            .await
            .unwrap();
        repo.save(&TaskNote::new(task_b.id, "b").unwrap())
            .await
            .unwrap();

        repo.delete_by_task(task_a.id).await.unwrap();
        assert_eq!(repo.count_by_task(task_a.id).await.unwrap(), 0);
        assert_eq!(repo.count_by_task(task_b.id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn count_of_task_without_notes_is_zero() {
        let (repo, _tasks, task) = fixtures().await;
        assert_eq!(repo.count_by_task(task.id).await.unwrap(), 0);
        assert!(repo.list_by_task(task.id).await.unwrap().is_empty());
    }

    /// A migration declara ON DELETE CASCADE. SQLite só honra FK quando a
    /// conexão liga `PRAGMA foreign_keys` — este teste prova o comportamento
    /// com o pragma ligado, que é como `src-tauri/src/lib.rs` abre o pool.
    #[tokio::test]
    async fn deleting_the_task_cascades_to_its_notes() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .connect_with(
                "sqlite::memory:"
                    .parse::<sqlx::sqlite::SqliteConnectOptions>()
                    .unwrap()
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();

        let tasks = SqliteTaskRepository::new(pool.clone());
        let notes = SqliteTaskNoteRepository::new(pool);
        let task = Task::new("some").unwrap();
        tasks.save(&task).await.unwrap();
        notes
            .save(&TaskNote::new(task.id, "anotação").unwrap())
            .await
            .unwrap();
        assert_eq!(notes.count_by_task(task.id).await.unwrap(), 1);

        tasks.delete(task.id).await.unwrap();
        assert_eq!(notes.count_by_task(task.id).await.unwrap(), 0);
    }
}
