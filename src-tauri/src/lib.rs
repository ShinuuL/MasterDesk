//! Wiring do app Tauri — Fase 2 (Local Notes) + Fase 3 (Tasks/Notificações).

pub mod commands;
pub mod window_service;

use std::sync::Arc;

use masterdesk_infrastructure::{
    LocalAuthRepository, NotificationService, SqliteNoteRepository, SqliteTaskRepository,
};
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

            // Block_on para criar pool síncrono no setup (Tauri setup é síncrono)
            let pool = tauri::async_runtime::block_on(async {
                use sqlx::sqlite::SqliteConnectOptions;
                // Caminho Windows com backslashes não funciona como URL `sqlite://...`;
                // usar SqliteConnectOptions::filename lida corretamente com qualquer OS
                // e evita o panic "unknown query parameter `create_if_missing`" que fazia
                // o app abrir e fechar instantaneamente em release.
                let opts = SqliteConnectOptions::new()
                    .filename(&db_path)
                    .create_if_missing(true);
                let pool = sqlx::SqlitePool::connect_with(opts)
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

                    let _ = sqlx::query(
                        r#"
                        CREATE TABLE IF NOT EXISTS users (
                            id            TEXT PRIMARY KEY,
                            username      TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(username) >= 3 AND length(username) <= 32),
                            password_hash TEXT NOT NULL,
                            created_at    TEXT NOT NULL
                        )
                        "#,
                    )
                    .execute(&pool)
                    .await;
                }
                pool
            });

            let repo = Arc::new(SqliteNoteRepository::new(pool.clone()));
            let task_repo = Arc::new(SqliteTaskRepository::new(pool.clone()));
            let notification_service = Arc::new(NotificationService::new());
            let auth_repo = Arc::new(LocalAuthRepository::new(pool));
            app.manage(commands::AppState {
                repo,
                task_repo,
                notification_service,
                auth_repo,
            });

            // ---- System tray (Tauri 2 native) ----
            let show_item = tauri::menu::MenuItemBuilder::with_id("show", "Mostrar MasterDesk")
                .build(app)?;
            let quit_item =
                tauri::menu::MenuItemBuilder::with_id("quit", "Sair").build(app)?;
            let menu = tauri::menu::MenuBuilder::new(app)
                .item(&show_item)
                .item(&quit_item)
                .build()?;

            let _tray = tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
                    // Fallback: cria ícone 1x1 transparente se não achar o default
                    tauri::image::Image::new_owned(vec![0u8; 4], 1, 1)
                }))
                .menu(&menu)
                .tooltip("MasterDesk")
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .build(app)?;

            // ---- Close-to-tray: interceptar fechamento da janela principal ----
            // Se houver notas em janelas dedicadas (pop-out), esconde em vez de
            // sair — as notas continuam visíveis por cima de outros apps.
            // Sem janelas de nota, o X fecha normalmente.
            if let Some(main_win) = app.get_webview_window("main") {
                let main_handle = main_win.as_ref().clone();
                let app_for_check = app.handle().clone();
                main_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let has_note_windows = app_for_check
                            .webview_windows()
                            .keys()
                            .any(|label| label.starts_with("note-"));
                        if has_note_windows {
                            api.prevent_close();
                            let _ = main_handle.hide();
                        }
                    }
                });
            }

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
            commands::open_note_window,
            commands::close_note_window,
            commands::set_note_window_always_on_top,
            commands::set_note_window_position,
            commands::set_note_window_size,
            commands::is_note_window_open,
            commands::create_task,
            commands::get_task,
            commands::list_pending_tasks,
            commands::list_completed_tasks,
            commands::list_all_tasks,
            commands::update_task,
            commands::complete_task,
            commands::reopen_task,
            commands::delete_task,
            commands::snooze_task,
            commands::auth_register,
            commands::auth_login,
            commands::auth_logout,
            commands::auth_is_authenticated
        ])
        // .plugin(tauri_plugin_notification::init())                 // Fase 3 (ADR-004)
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o MasterDesk");
}
