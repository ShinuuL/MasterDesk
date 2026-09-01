//! `SqliteNoteRepository` — implementação real do port `NoteRepository`
//! sobre SQLite via sqlx (ADR-003). Não vaza `sqlx::Error` para o domínio;
//! mapeia tudo para `DomainError` na borda.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use masterdesk_domain::{ports::NoteRepository, DomainError, DomainResult, Note, NoteId, Priority};
use sqlx::SqlitePool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SqliteNoteRepository {
    pool: SqlitePool,
}

impl SqliteNoteRepository {
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
struct NoteRow {
    id: String,
    title: String,
    content: String,
    tags: String,
    priority: String,
    deadline: Option<String>,
    color: String,
    opacity: f64,
    pinned: i64,
    always_on_top: i64,
    archived: i64,
    position_x: f64,
    position_y: f64,
    size_w: f64,
    size_h: f64,
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

fn row_to_note(row: NoteRow) -> DomainResult<Note> {
    let id = Uuid::parse_str(&row.id).map_err(|_| DomainError::Persistence)?;
    let tags: Vec<String> =
        serde_json::from_str(&row.tags).map_err(|_| DomainError::Persistence)?;
    let priority = parse_priority(&row.priority);
    let deadline: Option<DateTime<Utc>> = row
        .deadline
        .as_deref()
        .map(|s| {
            s.parse::<DateTime<Utc>>()
                .map_err(|_| DomainError::Persistence)
        })
        .transpose()?;
    let created_at = row
        .created_at
        .parse::<DateTime<Utc>>()
        .map_err(|_| DomainError::Persistence)?;
    let updated_at = row
        .updated_at
        .parse::<DateTime<Utc>>()
        .map_err(|_| DomainError::Persistence)?;

    Note::reconstitute(
        id,
        row.title,
        row.content,
        tags,
        priority,
        deadline,
        row.color,
        row.opacity as f32,
        row.pinned != 0,
        row.always_on_top != 0,
        row.archived != 0,
        (row.position_x, row.position_y),
        (row.size_w, row.size_h),
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
impl NoteRepository for SqliteNoteRepository {
    async fn save(&self, note: &Note) -> DomainResult<()> {
        let tags_json = serde_json::to_string(&note.tags).map_err(|_| DomainError::Persistence)?;
        let deadline_str = note.deadline.map(|d| d.to_rfc3339());
        let now_str = note.updated_at.to_rfc3339();
        let created_str = note.created_at.to_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO notes (
                id, title, content, tags, priority, deadline,
                color, opacity, pinned, always_on_top, archived,
                position_x, position_y, size_w, size_h,
                created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15,
                ?16, ?17
            )
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                content = excluded.content,
                tags = excluded.tags,
                priority = excluded.priority,
                deadline = excluded.deadline,
                color = excluded.color,
                opacity = excluded.opacity,
                pinned = excluded.pinned,
                always_on_top = excluded.always_on_top,
                archived = excluded.archived,
                position_x = excluded.position_x,
                position_y = excluded.position_y,
                size_w = excluded.size_w,
                size_h = excluded.size_h,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(note.id.to_string())
        .bind(&note.title)
        .bind(&note.content)
        .bind(tags_json)
        .bind(priority_to_str(note.priority))
        .bind(deadline_str)
        .bind(&note.color)
        .bind(note.opacity as f64)
        .bind(if note.pinned { 1i64 } else { 0i64 })
        .bind(if note.always_on_top { 1i64 } else { 0i64 })
        .bind(if note.archived { 1i64 } else { 0i64 })
        .bind(note.position.0)
        .bind(note.position.1)
        .bind(note.size.0)
        .bind(note.size.1)
        .bind(created_str)
        .bind(now_str)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        Ok(())
    }

    async fn find_by_id(&self, id: NoteId) -> DomainResult<Option<Note>> {
        let row: Option<NoteRow> = sqlx::query_as("SELECT * FROM notes WHERE id = ?1")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        match row {
            Some(r) => Ok(Some(row_to_note(r)?)),
            None => Ok(None),
        }
    }

    async fn list_active(&self) -> DomainResult<Vec<Note>> {
        let rows: Vec<NoteRow> =
            sqlx::query_as("SELECT * FROM notes WHERE archived = 0 ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
        rows.into_iter().map(row_to_note).collect()
    }

    async fn list_archived(&self) -> DomainResult<Vec<Note>> {
        let rows: Vec<NoteRow> =
            sqlx::query_as("SELECT * FROM notes WHERE archived = 1 ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
        rows.into_iter().map(row_to_note).collect()
    }

    async fn list_all(&self) -> DomainResult<Vec<Note>> {
        let rows: Vec<NoteRow> = sqlx::query_as("SELECT * FROM notes ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        rows.into_iter().map(row_to_note).collect()
    }

    async fn delete(&self, id: NoteId) -> DomainResult<()> {
        sqlx::query("DELETE FROM notes WHERE id = ?1")
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
            CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                tags TEXT NOT NULL DEFAULT '[]',
                priority TEXT NOT NULL DEFAULT 'Medium',
                deadline TEXT,
                color TEXT NOT NULL DEFAULT '#FFEB3B',
                opacity REAL NOT NULL DEFAULT 1.0,
                pinned INTEGER NOT NULL DEFAULT 0,
                always_on_top INTEGER NOT NULL DEFAULT 0,
                archived INTEGER NOT NULL DEFAULT 0,
                position_x REAL NOT NULL DEFAULT 100.0,
                position_y REAL NOT NULL DEFAULT 100.0,
                size_w REAL NOT NULL DEFAULT 300.0,
                size_h REAL NOT NULL DEFAULT 250.0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn fresh_repo() -> SqliteNoteRepository {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        init_schema(&pool).await;
        SqliteNoteRepository::new(pool)
    }

    #[tokio::test]
    async fn crud_roundtrip() {
        let repo = fresh_repo().await;
        let mut note = masterdesk_domain::Note::new("Título", "Conteúdo").unwrap();
        note.set_tags(vec!["rust".into()]).unwrap();
        repo.save(&note).await.unwrap();

        let fetched = repo.find_by_id(note.id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Título");
        assert_eq!(fetched.tags, vec!["rust"]);

        // update
        let mut updated = fetched.clone();
        updated.set_title("Novo").unwrap();
        repo.save(&updated).await.unwrap();
        let again = repo.find_by_id(note.id).await.unwrap().unwrap();
        assert_eq!(again.title, "Novo");

        // archive filtering
        let mut to_archive = again.clone();
        to_archive.archive();
        repo.save(&to_archive).await.unwrap();
        assert_eq!(repo.list_active().await.unwrap().len(), 0);
        assert_eq!(repo.list_archived().await.unwrap().len(), 1);
        assert_eq!(repo.list_all().await.unwrap().len(), 1);

        // delete
        repo.delete(note.id).await.unwrap();
        assert!(repo.find_by_id(note.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn opacity_and_color_persisted() {
        let repo = fresh_repo().await;
        let mut n = masterdesk_domain::Note::new("a", "b").unwrap();
        n.set_color("#00ff00").unwrap();
        n.set_opacity(0.42).unwrap();
        n.set_position(123.0, 456.0).unwrap();
        n.set_size(400.0, 300.0).unwrap();
        repo.save(&n).await.unwrap();
        let f = repo.find_by_id(n.id).await.unwrap().unwrap();
        assert_eq!(f.color, "#00ff00");
        assert!((f.opacity - 0.42).abs() < 0.001);
        assert_eq!(f.position, (123.0, 456.0));
        assert_eq!(f.size, (400.0, 300.0));
    }
}
