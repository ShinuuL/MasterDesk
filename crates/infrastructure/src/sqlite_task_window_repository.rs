//! Geometria da janela destacada de uma tarefa.
//!
//! Mora fora da entidade `Task` de propósito — ver o cabeçalho de
//! `migrations/0009_task_window_state.sql` para o raciocínio.

use chrono::Utc;
use masterdesk_domain::{DomainError, DomainResult, TaskId};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Onde a janela da tarefa estava e se ficava por cima das outras.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TaskWindowState {
    pub position: (f64, f64),
    pub size: (f64, f64),
    pub always_on_top: bool,
}

impl Default for TaskWindowState {
    /// Precisa casar com os DEFAULT da migration, senão a primeira abertura
    /// (sem linha gravada) e a segunda (com linha) divergiriam.
    fn default() -> Self {
        Self {
            position: (120.0, 120.0),
            size: (380.0, 300.0),
            always_on_top: true,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct StateRow {
    position_x: f64,
    position_y: f64,
    size_w: f64,
    size_h: f64,
    always_on_top: i64,
}

impl From<StateRow> for TaskWindowState {
    fn from(r: StateRow) -> Self {
        Self {
            position: (r.position_x, r.position_y),
            size: (r.size_w, r.size_h),
            always_on_top: r.always_on_top != 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SqliteTaskWindowRepository {
    pool: SqlitePool,
}

fn map_sqlx_err(_e: sqlx::Error) -> DomainError {
    DomainError::Persistence
}

/// Faixas aceitas pelos `CHECK` da migration. Validar aqui evita que um
/// `onResized` com valor esquisito (minimizar reporta 0x0 em alguns WMs)
/// vire erro de constraint no meio de uma sincronização de janela.
fn clamp_size(w: f64, h: f64) -> (f64, f64) {
    let d = TaskWindowState::default();
    let w = if w.is_finite() { w } else { d.size.0 };
    let h = if h.is_finite() { h } else { d.size.1 };
    (w.clamp(180.0, 4096.0), h.clamp(140.0, 4096.0))
}

fn sane_position(x: f64, y: f64) -> (f64, f64) {
    let d = TaskWindowState::default();
    (
        if x.is_finite() { x } else { d.position.0 },
        if y.is_finite() { y } else { d.position.1 },
    )
}

impl SqliteTaskWindowRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Estado gravado, ou o default quando a tarefa nunca foi destacada.
    ///
    /// Devolver o default em vez de `Option` porque quem chama sempre precisa
    /// de algum valor para abrir a janela, e "nunca destacada" não é erro.
    pub async fn get(&self, task_id: TaskId) -> DomainResult<TaskWindowState> {
        let row: Option<StateRow> = sqlx::query_as(
            "SELECT position_x, position_y, size_w, size_h, always_on_top
             FROM task_window_state WHERE task_id = ?1",
        )
        .bind(task_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        Ok(row.map(Into::into).unwrap_or_default())
    }

    pub async fn set_position(&self, task_id: TaskId, x: f64, y: f64) -> DomainResult<()> {
        let (x, y) = sane_position(x, y);
        self.upsert(task_id, |s| s.position = (x, y)).await
    }

    pub async fn set_size(&self, task_id: TaskId, w: f64, h: f64) -> DomainResult<()> {
        let (w, h) = clamp_size(w, h);
        self.upsert(task_id, |s| s.size = (w, h)).await
    }

    pub async fn set_always_on_top(&self, task_id: TaskId, enabled: bool) -> DomainResult<()> {
        self.upsert(task_id, |s| s.always_on_top = enabled).await
    }

    /// Lê-modifica-grava numa transação.
    ///
    /// A transação importa porque `onMoved` e `onResized` disparam quase
    /// juntos ao arrastar a borda de uma janela: sem ela, os dois leriam o
    /// mesmo estado e o segundo write descartaria a mudança do primeiro.
    async fn upsert<F>(&self, task_id: TaskId, mutate: F) -> DomainResult<()>
    where
        F: FnOnce(&mut TaskWindowState),
    {
        let id = task_id.to_string();
        let mut tx = self.pool.begin().await.map_err(map_sqlx_err)?;

        let row: Option<StateRow> = sqlx::query_as(
            "SELECT position_x, position_y, size_w, size_h, always_on_top
             FROM task_window_state WHERE task_id = ?1",
        )
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_sqlx_err)?;

        let mut state: TaskWindowState = row.map(Into::into).unwrap_or_default();
        mutate(&mut state);

        sqlx::query(
            r#"
            INSERT INTO task_window_state
                (task_id, position_x, position_y, size_w, size_h, always_on_top, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(task_id) DO UPDATE SET
                position_x    = excluded.position_x,
                position_y    = excluded.position_y,
                size_w        = excluded.size_w,
                size_h        = excluded.size_h,
                always_on_top = excluded.always_on_top,
                updated_at    = excluded.updated_at
            "#,
        )
        .bind(&id)
        .bind(state.position.0)
        .bind(state.position.1)
        .bind(state.size.0)
        .bind(state.size.1)
        .bind(i64::from(state.always_on_top))
        .bind(Utc::now().to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_err)?;

        tx.commit().await.map_err(map_sqlx_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    /// Precisa de uma tarefa real: `task_id` é FK para `tasks`.
    async fn fresh() -> (SqliteTaskWindowRepository, TaskId) {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO tasks (id, title, description, priority, reminder_thresholds,
                                completed, created_at, updated_at)
             VALUES (?1, 'tarefa', '', 'Medium', '[]', 0, ?2, ?2)",
        )
        .bind(id.to_string())
        .bind(Utc::now().to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        (SqliteTaskWindowRepository::new(pool), id)
    }

    #[tokio::test]
    async fn never_popped_out_returns_the_default() {
        let (repo, id) = fresh().await;
        assert_eq!(repo.get(id).await.unwrap(), TaskWindowState::default());
    }

    #[tokio::test]
    async fn position_and_size_round_trip_independently() {
        let (repo, id) = fresh().await;
        repo.set_position(id, 640.0, 480.0).await.unwrap();
        repo.set_size(id, 500.0, 400.0).await.unwrap();

        let s = repo.get(id).await.unwrap();
        assert_eq!(s.position, (640.0, 480.0));
        assert_eq!(s.size, (500.0, 400.0));
        // Gravar tamanho não pode zerar a posição gravada antes — é o que um
        // upsert sem read-modify-write faria.
        assert!(s.always_on_top, "default preservado");
    }

    #[tokio::test]
    async fn always_on_top_toggles_without_touching_geometry() {
        let (repo, id) = fresh().await;
        repo.set_position(id, 10.0, 20.0).await.unwrap();
        repo.set_always_on_top(id, false).await.unwrap();

        let s = repo.get(id).await.unwrap();
        assert!(!s.always_on_top);
        assert_eq!(s.position, (10.0, 20.0));
    }

    #[tokio::test]
    async fn degenerate_size_is_clamped_instead_of_violating_the_check() {
        // Minimizar reporta 0x0 em alguns gerenciadores de janela; sem o clamp
        // isso viraria erro de constraint.
        let (repo, id) = fresh().await;
        repo.set_size(id, 0.0, 0.0).await.unwrap();
        let s = repo.get(id).await.unwrap();
        assert_eq!(s.size, (180.0, 140.0));

        repo.set_size(id, f64::NAN, f64::NAN).await.unwrap();
        assert_eq!(
            repo.get(id).await.unwrap().size,
            TaskWindowState::default().size
        );
    }

    #[tokio::test]
    async fn state_dies_with_the_task() {
        let (repo, id) = fresh().await;
        repo.set_position(id, 1.0, 2.0).await.unwrap();
        // `retire_mirror` apaga espelhos que saíram da fila; a geometria não
        // pode ficar órfã.
        sqlx::query("DELETE FROM tasks WHERE id = ?1")
            .bind(id.to_string())
            .execute(&repo.pool)
            .await
            .unwrap();
        assert_eq!(repo.get(id).await.unwrap(), TaskWindowState::default());
    }
}
