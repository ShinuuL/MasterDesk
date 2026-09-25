//! "Iniciar com o sistema" — comandos sobre `tauri-plugin-autostart`.
//!
//! O estado vive no próprio SO (no Windows, o valor em
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`), não no banco: é o
//! que o Windows de fato vai executar, e guardar uma cópia no SQLite só criaria
//! a chance das duas divergirem.
//!
//! Desligar pelo Gerenciador de Tarefas / Configurações → Inicialização não
//! apaga o valor do `Run`, mas grava um override em `StartupApproved\Run`. O
//! `auto-launch` 0.5 lê esse override (`is_enabled` responde `false`) e o
//! `enable` o reativa — conferido no fonte de `auto-launch-0.5.0/src/windows.rs`.
//! Não validado em macOS/Linux.
//!
//! No mobile não há o plugin (ver `Cargo.toml`), e os comandos respondem erro
//! explícito em vez de fingir sucesso.

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
use tauri_plugin_autostart::ManagerExt;

/// Se o app está registrado para abrir no login do usuário.
#[tauri::command]
pub fn autostart_is_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        app.autolaunch()
            .is_enabled()
            .map_err(|e| format!("não foi possível ler a inicialização automática: {e}"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = app;
        Err(UNSUPPORTED.into())
    }
}

/// Liga ou desliga a abertura no login. Devolve o estado lido **depois** da
/// operação, para a UI mostrar o que o SO registrou e não o que foi pedido.
#[tauri::command]
pub fn autostart_set_enabled(app: tauri::AppHandle, enabled: bool) -> Result<bool, String> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
    {
        let manager = app.autolaunch();
        let read = |m: &tauri_plugin_autostart::AutoLaunchManager| {
            m.is_enabled()
                .map_err(|e| format!("não foi possível ler a inicialização automática: {e}"))
        };
        // Só age quando muda: o `disable` do `auto-launch` faz `delete_value`
        // e falha se o valor não existe — desligar o que já está desligado
        // viraria erro na tela.
        if read(&manager)? == enabled {
            return Ok(enabled);
        }
        let result = if enabled {
            manager.enable()
        } else {
            manager.disable()
        };
        result.map_err(|e| format!("não foi possível alterar a inicialização automática: {e}"))?;
        read(&manager)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = (app, enabled);
        Err(UNSUPPORTED.into())
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
const UNSUPPORTED: &str = "inicialização automática não está disponível nesta plataforma";
