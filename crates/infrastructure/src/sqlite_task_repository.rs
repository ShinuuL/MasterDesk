//! `SqliteTaskRepository` — implementação real do port `TaskRepository`
//! sobre SQLite via sqlx (ADR-003). Não vaza `sqlx::Error` para o domínio;
//! mapeia tudo para `DomainError` na borda.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use masterdesk_domain::{
    ports::TaskRepository, DomainError, DomainResult, ExternalKind, ExternalRef, ExternalSystem,
    Priority, ReminderThreshold, Task, TaskId,
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
    external_system: Option<String>,
    external_kind: Option<String>,
    external_id: Option<String>,
    external_client: Option<String>,
    external_ticket: Option<String>,
    external_status: Option<String>,
    external_status_parked: i64,
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

    let external = row_to_external(
        row.external_system.as_deref(),
        row.external_kind.as_deref(),
        row.external_id.as_deref(),
        row.external_client,
        row.external_ticket,
        row.external_status,
        row.external_status_parked != 0,
    )?;

    Ok(Task::reconstitute(
        id,
        row.title,
        row.description,
        priority,
        deadline,
        reminder_thresholds,
        row.completed != 0,
        created_at,
        updated_at,
    )?
    .attach_external(external))
}

/// Uma linha so e considerada externa quando sistema + tipo + id estao todos
/// presentes. Combinacao parcial e corrupcao de dados, nao um estado valido —
/// tratar como local esconderia o problema e duplicaria o item no proximo sync.
fn row_to_external(
    system: Option<&str>,
    kind: Option<&str>,
    external_id: Option<&str>,
    client: Option<String>,
    ticket: Option<String>,
    status: Option<String>,
    status_parked: bool,
) -> DomainResult<Option<ExternalRef>> {
    match (system, kind, external_id) {
        (None, None, None) => Ok(None),
        (Some(sys), Some(k), Some(eid)) => {
            let system = ExternalSystem::parse(sys).map_err(|_| DomainError::Persistence)?;
            let kind = ExternalKind::parse(k).map_err(|_| DomainError::Persistence)?;
            let reference = ExternalRef::new(system, kind, eid)
                .map_err(|_| DomainError::Persistence)?
                .with_client(client)
                .with_ticket(ticket)
                .with_status_label(status)
                .with_status_parked(status_parked);
            Ok(Some(reference))
        }
        _ => Err(DomainError::Persistence),
    }
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
                reminder_thresholds, completed,
                external_system, external_kind, external_id,
                external_client, external_ticket, external_status,
                external_status_parked,
                created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7,
                ?8, ?9, ?10,
                ?11, ?12, ?13,
                ?14,
                ?15, ?16
            )
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                description = excluded.description,
                priority = excluded.priority,
                deadline = excluded.deadline,
                reminder_thresholds = excluded.reminder_thresholds,
                completed = excluded.completed,
                external_system = excluded.external_system,
                external_kind = excluded.external_kind,
                external_id = excluded.external_id,
                external_client = excluded.external_client,
                external_ticket = excluded.external_ticket,
                external_status = excluded.external_status,
                external_status_parked = excluded.external_status_parked,
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
        .bind(task.external.as_ref().map(|e| e.system.as_str()))
        .bind(task.external.as_ref().map(|e| e.kind.as_str()))
        .bind(task.external.as_ref().map(|e| e.external_id.clone()))
        .bind(task.external.as_ref().and_then(|e| e.client.clone()))
        .bind(task.external.as_ref().and_then(|e| e.ticket.clone()))
        .bind(task.external.as_ref().and_then(|e| e.status_label.clone()))
        .bind(
            task.external
                .as_ref()
                .map_or(0i64, |e| if e.status_parked { 1 } else { 0 }),
        )
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

    async fn find_by_external(&self, reference: &ExternalRef) -> DomainResult<Option<Task>> {
        let row: Option<TaskRow> =
            sqlx::query_as("SELECT * FROM tasks WHERE external_system = ?1 AND external_id = ?2")
                .bind(reference.system.as_str())
                .bind(&reference.external_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
        match row {
            Some(r) => Ok(Some(row_to_task(r)?)),
            None => Ok(None),
        }
    }

    async fn list_by_external_system(&self, system: ExternalSystem) -> DomainResult<Vec<Task>> {
        let rows: Vec<TaskRow> = sqlx::query_as(
            "SELECT * FROM tasks WHERE external_system = ?1 ORDER BY updated_at DESC",
        )
        .bind(system.as_str())
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

    /// Roda as migrations REAIS em memoria em vez de um schema inline. Um
    /// schema de teste escrito a mao passa a divergir do de producao na
    /// primeira migration nova — e o teste continua verde mentindo.
    async fn fresh_repo() -> SqliteTaskRepository {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
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
    async fn external_ref_roundtrip_and_lookup() {
        let repo = fresh_repo().await;

        let reference = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Ticket, "4821")
            .unwrap()
            .with_client(Some("Padaria do Zé".into()))
            .with_ticket(Some("4821".into()))
            .with_status_label(Some("aguardando_retorno_cliente".into()));

        let task = masterdesk_domain::Task::new("Erro na NF-e de saída")
            .unwrap()
            .attach_external(Some(reference.clone()));
        repo.save(&task).await.unwrap();

        let found = repo.find_by_external(&reference).await.unwrap().unwrap();
        assert_eq!(found.id, task.id);
        let ext = found.external.as_ref().unwrap();
        assert_eq!(ext.system, ExternalSystem::Mastersys);
        assert_eq!(ext.kind, ExternalKind::Ticket);
        assert_eq!(ext.client.as_deref(), Some("Padaria do Zé"));
        assert_eq!(
            ext.status_label.as_deref(),
            Some("aguardando_retorno_cliente")
        );

        let by_system = repo
            .list_by_external_system(ExternalSystem::Mastersys)
            .await
            .unwrap();
        assert_eq!(by_system.len(), 1);
    }

    #[tokio::test]
    async fn local_tasks_have_no_external_and_dont_collide() {
        let repo = fresh_repo().await;
        // O índice único de external é parcial; várias tarefas locais (todas
        // com external_id NULL) precisam coexistir.
        for i in 0..3 {
            repo.save(&masterdesk_domain::Task::new(format!("local {i}")).unwrap())
                .await
                .unwrap();
        }
        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 3);
        assert!(all.iter().all(|t| !t.is_external()));
        assert!(repo
            .list_by_external_system(ExternalSystem::Mastersys)
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn unknown_external_id_returns_none() {
        let repo = fresh_repo().await;
        let missing =
            ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Task, "999").unwrap();
        assert!(repo.find_by_external(&missing).await.unwrap().is_none());
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

/// Testes de **upgrade de schema**, separados do CRUD porque simulam o estado
/// de um banco já instalado em vez de um banco novo.
#[cfg(test)]
mod migration_tests {
    use masterdesk_domain::ports::TaskRepository;
    use masterdesk_domain::{ExternalKind, ExternalRef, ExternalSystem, Task};
    use sqlx::SqlitePool;

    use super::SqliteTaskRepository;

    /// Recria exatamente o schema que o fallback inline de `src-tauri/src/lib.rs`
    /// criava quando `migrations/` não existia — ou seja, o banco real de quem
    /// instalou uma versão anterior em release.
    ///
    /// Esses bancos têm as tabelas mas `_sqlx_migrations` vazio, então TODAS as
    /// migrations reexecutam por cima. Este teste existe para provar que isso é
    /// seguro: se 0002/0003/0004 não fossem idempotentes ou se 0005 tentasse
    /// adicionar uma coluna já existente, o app abriria e morreria no upgrade.
    async fn legacy_fallback_db() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        for ddl in [
            r#"CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY, title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT '[]',
                priority TEXT NOT NULL DEFAULT 'Medium', deadline TEXT,
                color TEXT NOT NULL DEFAULT '#FFEB3B', opacity REAL NOT NULL DEFAULT 1.0,
                pinned INTEGER NOT NULL DEFAULT 0, always_on_top INTEGER NOT NULL DEFAULT 0,
                archived INTEGER NOT NULL DEFAULT 0,
                position_x REAL NOT NULL DEFAULT 100.0, position_y REAL NOT NULL DEFAULT 100.0,
                size_w REAL NOT NULL DEFAULT 300.0, size_h REAL NOT NULL DEFAULT 250.0,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL)"#,
            r#"CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY, title TEXT NOT NULL,
                description TEXT NOT NULL DEFAULT '', priority TEXT NOT NULL DEFAULT 'Medium',
                deadline TEXT, reminder_thresholds TEXT NOT NULL DEFAULT '[]',
                completed INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL)"#,
            r#"CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL COLLATE NOCASE UNIQUE
                    CHECK (length(username) >= 3 AND length(username) <= 32),
                password_hash TEXT NOT NULL, created_at TEXT NOT NULL)"#,
        ] {
            sqlx::query(ddl).execute(&pool).await.unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn migrations_apply_over_a_legacy_fallback_database() {
        let pool = legacy_fallback_db().await;

        // Semeia com SQL cru, do jeito que a versão ANTIGA escrevia (9 colunas).
        // Usar o repositório atual aqui falharia, porque ele já grava as
        // colunas external_* — e é justamente isso que a migration adiciona.
        // Em produção não existe essa janela: `lib.rs` roda as migrations no
        // setup, antes de qualquer repositório tocar o banco.
        let existing = Task::new("tarefa de antes do upgrade").unwrap();
        sqlx::query(
            "INSERT INTO tasks (id, title, description, priority, deadline,
                                reminder_thresholds, completed, created_at, updated_at)
             VALUES (?1, ?2, '', 'Medium', NULL, '[]', 0, ?3, ?3)",
        )
        .bind(existing.id.to_string())
        .bind(&existing.title)
        .bind(existing.created_at.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();

        let repo = SqliteTaskRepository::new(pool.clone());

        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations devem aplicar sobre um banco do fallback antigo");

        let survivor = repo.find_by_id(existing.id).await.unwrap().unwrap();
        assert_eq!(survivor.title, "tarefa de antes do upgrade");
        assert!(!survivor.is_external(), "tarefa antiga continua local");

        // E as capacidades novas funcionam no banco migrado.
        let reference =
            ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Task, "1").unwrap();
        let imported = Task::new("veio do mastersys")
            .unwrap()
            .attach_external(Some(reference.clone()));
        repo.save(&imported).await.unwrap();
        assert_eq!(
            repo.find_by_external(&reference).await.unwrap().unwrap().id,
            imported.id
        );
    }

    #[tokio::test]
    async fn migrations_are_safe_to_run_twice() {
        // `migrate!` marca o que já rodou, então a segunda chamada é no-op.
        // Se algum dia alguém editar um .sql aplicado, o checksum falha aqui.
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
    }

    #[tokio::test]
    async fn external_unique_index_rejects_a_duplicate_mirror() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        let repo = SqliteTaskRepository::new(pool);

        let reference =
            ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Ticket, "77").unwrap();
        repo.save(
            &Task::new("primeiro espelho")
                .unwrap()
                .attach_external(Some(reference.clone())),
        )
        .await
        .unwrap();

        // Uma segunda tarefa (id local diferente) para o MESMO item externo
        // precisa ser recusada pelo índice — é a rede de segurança contra a
        // sincronização duplicar um chamado se a busca por external falhar.
        let duplicate = Task::new("espelho duplicado")
            .unwrap()
            .attach_external(Some(reference));
        assert!(
            repo.save(&duplicate).await.is_err(),
            "o índice único parcial deve impedir dois espelhos do mesmo item"
        );
    }
}
