//! Wiring do app Tauri — Fase 2 (Local Notes) + Fase 3 (Tasks/Notificações).

pub mod commands;
pub mod realtime_supervisor;
pub mod sync_scheduler;

use std::sync::Arc;

use masterdesk_infrastructure::{
    LocalAuthRepository, MastersysProvider, NotificationService, SqliteNoteRepository,
    SqliteSettingsRepository, SqliteStatusCatalogRepository, SqliteTaskNoteRepository,
    SqliteTaskRepository, SqliteTaskWindowRepository,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    // Atualização automática — o toast de "atualização disponível" no frontend
    // fala com estes dois plugins: `updater` para checar/baixar/instalar e
    // `process` para o `relaunch()` que reabre o app depois de instalar.
    //
    // O `cfg` acompanha o do `Cargo.toml`: no mobile as crates nem entram no
    // grafo de dependências, então referenciá-las aqui sem guarda não compila.
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder
        .setup(|app| {
            // Inicializa SQLite em app_data_dir/masterdesk.db
            //
            // ATENÇÃO AO RENOMEAR O PRODUTO
            //
            // O nome de exibição virou "MasterNote" em 2026-09-03, mas nem o
            // `identifier` do `tauri.conf.json` (`com.masterdesk.app`) nem o
            // nome deste arquivo `.db` foram alterados, **de propósito**.
            //
            // `app_data_dir()` resolve para `%APPDATA%/<identifier>` no
            // Windows. Trocar o identifier ou o nome do arquivo faz o app abrir
            // apontando para um caminho vazio e **criar um banco novo em
            // silêncio** — sem erro, sem aviso, e com as notas, tarefas,
            // anotações e a sessão do Mastersys do usuário ficando órfãs no
            // caminho antigo.
            //
            // Mudar isso é tarefa própria, não efeito colateral de um rename:
            // exige mover a pasta antiga na primeira abertura, de forma
            // idempotente e sem sobrescrever nada. Nenhum dos dois nomes é
            // visível ao usuário — o que ele lê é o `productName`.
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
                //
                // `foreign_keys(true)`: SQLite desliga FK por conexão. Sem isso o
                // ON DELETE CASCADE de `task_notes` (migration 0005) seria ignorado.
                let opts = SqliteConnectOptions::new()
                    .filename(&db_path)
                    .create_if_missing(true)
                    .foreign_keys(true);
                let pool = sqlx::SqlitePool::connect_with(opts)
                    .await
                    .expect("failed to connect sqlite");

                // Migrations EMBUTIDAS no binário em tempo de compilação.
                //
                // Antes daqui o setup tentava `Migrator::new("./migrations")` em
                // caminhos relativos e, quando nenhum existia (o caso em release,
                // porque `migrations/` não é empacotado), caía num fallback que
                // recriava `notes`/`tasks`/`users` inline. Isso significava que
                // qualquer migration nova só se aplicava em dev, e o schema de
                // produção divergia silenciosamente do dos arquivos .sql.
                //
                // `migrate!` resolve os dois problemas: o SQL vai para dentro do
                // executável e o mesmo caminho de código roda em dev e release.
                // Bancos criados pelo fallback antigo têm `_sqlx_migrations`
                // vazio, então 0001..0004 reexecutam — são todos
                // CREATE ... IF NOT EXISTS, portanto idempotentes.
                sqlx::migrate!("../migrations")
                    .run(&pool)
                    .await
                    .expect("failed to run database migrations");

                pool
            });

            let repo = Arc::new(SqliteNoteRepository::new(pool.clone()));
            let task_repo = Arc::new(SqliteTaskRepository::new(pool.clone()));
            let task_note_repo = Arc::new(SqliteTaskNoteRepository::new(pool.clone()));
            let notification_service = Arc::new(NotificationService::new());
            let auth_repo = Arc::new(LocalAuthRepository::new(pool.clone()));
            let settings_repo = Arc::new(SqliteSettingsRepository::new(pool.clone()));
            let task_window_repo = Arc::new(SqliteTaskWindowRepository::new(pool.clone()));
            let status_catalog_repo = Arc::new(SqliteStatusCatalogRepository::new(pool));
            // O provider só faz rede quando um comando pede; construí-lo aqui
            // não abre conexão nem lê o cofre.
            let mastersys = Arc::new(MastersysProvider::new(
                settings_repo.clone(),
                status_catalog_repo,
            ));
            // Sincronização automática. O agendador precisa poder construir o
            // serviço a cada ciclo (ele é barato — só junta Arcs), então recebe
            // uma closure em vez de uma instância: guardar uma instância viva
            // por horas prenderia os Arcs sem ganho.
            let sync_handle = {
                let provider = mastersys.clone();
                let tr = task_repo.clone();
                let tnr = task_note_repo.clone();
                let ns = notification_service.clone();
                sync_scheduler::spawn(
                    app.handle().clone(),
                    move || {
                        masterdesk_application::MastersysSyncService::new(
                            provider.clone(),
                            tr.clone(),
                            tnr.clone(),
                            Some(ns.clone()),
                        )
                    },
                    settings_repo.clone(),
                )
            };

            // Canal de tempo real. Sobe já conectado se houver sessão gravada:
            // reabrir o app não deveria custar 5 minutos de latência até a
            // primeira atualização.
            let realtime = Arc::new(realtime_supervisor::RealtimeSupervisor::new(
                sync_handle.clone(),
            ));
            {
                let rt = realtime.clone();
                let provider = mastersys.clone();
                tauri::async_runtime::spawn(async move {
                    // `base_url()` toca o banco, então não pode rodar no setup
                    // síncrono sem bloquear a abertura da janela.
                    if let Ok(url) = provider.base_url().await {
                        rt.reevaluate(url.as_deref());
                    }
                });
            }

            app.manage(commands::AppState {
                repo,
                task_repo,
                task_note_repo,
                notification_service,
                auth_repo,
                settings_repo,
                task_window_repo,
                mastersys,
                sync_handle,
                realtime,
            });

            // ---- System tray (Tauri 2 native) ----
            let show_item =
                tauri::menu::MenuItemBuilder::with_id("show", "Mostrar MasterNote").build(app)?;
            let quit_item = tauri::menu::MenuItemBuilder::with_id("quit", "Sair").build(app)?;
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
                .tooltip("MasterNote")
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
            // Havendo pop-out aberto (nota OU tarefa), esconde em vez de sair —
            // os pop-outs continuam visíveis por cima de outros apps. Sem
            // nenhum, o X fecha normalmente.
            //
            // Incluir `task-` importa: sem isso, fechar a janela principal com
            // uma tarefa destacada encerrava o app e matava a janela dela.
            if let Some(main_win) = app.get_webview_window("main") {
                let main_handle = main_win.as_ref().clone();
                let app_for_check = app.handle().clone();
                main_win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        let has_popouts = app_for_check
                            .webview_windows()
                            .keys()
                            .any(|label| label.starts_with("note-") || label.starts_with("task-"));
                        if has_popouts {
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
            commands::open_note_window_ids,
            commands::open_task_window,
            commands::close_task_window,
            commands::open_task_window_ids,
            commands::save_task_window_position,
            commands::save_task_window_size,
            commands::set_task_window_always_on_top,
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
            commands::auth_is_authenticated,
            commands::add_task_note,
            commands::list_task_notes,
            commands::count_task_notes,
            commands::count_all_task_notes,
            commands::update_task_note,
            commands::set_task_note_done,
            commands::delete_task_note,
            commands::mastersys_status,
            commands::mastersys_set_endpoint,
            commands::mastersys_set_ticket_window,
            commands::mastersys_status_catalog,
            commands::mastersys_search_tickets,
            commands::mastersys_poll_interval,
            commands::mastersys_set_poll_interval,
            commands::mastersys_realtime_connected,
            commands::mastersys_last_sync,
            commands::mastersys_sync_now,
            commands::mastersys_connect,
            commands::mastersys_disconnect,
            commands::mastersys_sync
        ])
        // .plugin(tauri_plugin_notification::init())                 // Fase 3 (ADR-004)
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o MasterNote");
}
