//! Wiring do app Tauri — Fase 2 (Local Notes) + Fase 3 (Tasks/Notificações).

pub mod commands;
pub mod window_service;

use std::sync::Arc;

use masterdesk_infrastructure::{NotificationService, SqliteNoteRepository, SqliteTaskRepository};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Inicializa SQLite em app_data_dir/masterdesk.db
            let handle = app.handle().clone();
            let resource_dir = handle
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("."));
            std::fs::create_dir_all(&resource_dir).ok();
            let db_path = resource_dir.join("masterdesk.db");
            let db_url = format!("sqlite://{}?create_if_missing=true", db_path.display());

            // Block_on para criar pool síncrono no setup (Tauri setup é síncrono)
            let pool = tauri::async_runtime::block_on(async {
                let pool = sqlx::SqlitePool::connect(&db_url)
                    .await
                    .expect("failed to connect sqlite");
                // Garante schema — tenta migrations se o diretório existir, senão cria inline.
                // Em produção o `.db` é criado via tauri-plugin-sql; aqui usamos sqlx direto.
                let try_migrate = async {
                    for candidate in ["./migrations", "../migrations", "../../migrations"] {
                        if let Ok(m) =
                            sqlx::migrate::Migrator::new(std::path::Path::new(candidate)).await
                        {
                            if m.run(&pool).await.is_ok() {
                                return true;
                            }
                        }
                    }
                    false
                }
                .await;
                if !try_migrate {
                    // Fallback: cria tabelas notes + tasks diretamente
                    // (útil em dev onde cwd é src-tauri)
                    let _ = sqlx::query(
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
                    .execute(&pool)
                    .await;

                    let _ = sqlx::query(
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
                    .execute(&pool)
                    .await;
                }
                pool
            });

            let repo = Arc::new(SqliteNoteRepository::new(pool.clone()));
            let task_repo = Arc::new(SqliteTaskRepository::new(pool));
            let notification_service = Arc::new(NotificationService::new());
            app.manage(commands::AppState {
                repo,
                task_repo,
                notification_service,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_note,
            commands::get_note,
            commands::list_active_notes,
            commands::list_archived_notes,
            commands::list_all_notes,
            commands::update_note,
            commands::archive_note,
            commands::unarchive_note,
            commands::delete_note,
            commands::toggle_pin,
            commands::set_always_on_top,
            commands::set_window_always_on_top,
            commands::create_task,
            commands::get_task,
            commands::list_pending_tasks,
            commands::list_completed_tasks,
            commands::list_all_tasks,
            commands::update_task,
            commands::complete_task,
            commands::reopen_task,
            commands::delete_task,
            commands::snooze_task
        ])
        // .plugin(tauri_plugin_notification::init())                 // Fase 3 (ADR-004)
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o MasterDesk");
}
