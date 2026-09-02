//! `WindowService` real via Tauri API (Fase 2, Dev 4).
//! Limitações documentadas no ADR-002 devem ser validadas manualmente em cada OS:
//! - Linux/Wayland: `set_always_on_top` pode falhar silenciosamente.
//! - macOS fullscreen: janela may not stay on top of fullscreen apps.
//! - Windows `visible_on_all_workspaces` não suportado.
//!
//! Nunca fingir o comportamento com workarounds não confiáveis.

use masterdesk_domain::{ports::WindowService, DomainError, DomainResult};
use tauri::Manager;

#[derive(Clone)]
pub struct TauriWindowService {
    app: tauri::AppHandle,
}

impl TauriWindowService {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }

    fn main_window(&self) -> DomainResult<tauri::WebviewWindow> {
        self.app
            .get_webview_window("main")
            .ok_or(DomainError::Persistence)
    }
}

impl WindowService for TauriWindowService {
    fn set_always_on_top(
        &self,
        _note_id: masterdesk_domain::NoteId,
        enabled: bool,
    ) -> DomainResult<()> {
        let win = self.main_window()?;
        win.set_always_on_top(enabled)
            .map_err(|_| DomainError::Persistence)
    }

    fn set_opacity(&self, _note_id: masterdesk_domain::NoteId, opacity: f32) -> DomainResult<()> {
        if !(0.1..=1.0).contains(&opacity) {
            return Err(DomainError::Validation(format!(
                "opacity out of range: {opacity}"
            )));
        }
        // Tauri não expõe `set_opacity` direto na janela no core 2.x sem plugin;
        // opacidade visual é aplicada via CSS no frontend (webview transparente).
        // Mantemos o domínio consistente persistindo opacity na Note; a janela usa CSS.
        // Se no futuro Tauri expuser API nativa, implementar aqui.
        Ok(())
    }

    fn set_position(
        &self,
        _note_id: masterdesk_domain::NoteId,
        x: f64,
        y: f64,
    ) -> DomainResult<()> {
        let win = self.main_window()?;
        // Tauri espera posição em pixels lógicos (PhysicalPosition vs LogicalPosition).
        // Usamos set_position com Logical.
        use tauri::{LogicalPosition, Position};
        win.set_position(Position::Logical(LogicalPosition { x, y }))
            .map_err(|_| DomainError::Persistence)
    }
}
