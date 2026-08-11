//! Wiring do app Tauri. Comandos (`#[tauri::command]`) que expõem casos de
//! uso da `application` para o frontend entram a partir da Fase 2.

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // .plugin(tauri_plugin_sql::Builder::default().build())      // Fase 2
        // .plugin(tauri_plugin_notification::init())                 // Fase 3
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o MasterDesk");
}
