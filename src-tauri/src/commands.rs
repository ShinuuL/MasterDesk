//! Comandos Tauri para Notes (Fase 2), Tasks (Fase 3), Auth (Fase 4),
//! anotações de tarefa e integração Mastersys (ADR-006).
//! Cada comando delega para um serviço de `masterdesk-application` que
//! orquestra validação de domínio + persistência.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use masterdesk_application::{
    AuthService, CreateNoteInput, CreateTaskInput, CreateUserInput, LoginInput,
    MastersysSyncService, SyncOptions, SyncReport, TaskNoteService, UpdateNoteInput,
    UpdateTaskInput, UserView,
};
use masterdesk_domain::{Note, Priority, ReminderThreshold, SupportIdentity, Task, TaskNote};
use masterdesk_infrastructure::{
    LocalAuthRepository, MastersysProvider, NotificationService, SqliteNoteRepository,
    SqliteSettingsRepository, SqliteTaskNoteRepository, SqliteTaskRepository,
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State, WebviewUrl, WebviewWindowBuilder};

// ---------------------------------------------------------------------------
// Estado
// ---------------------------------------------------------------------------

pub struct AppState {
    pub repo: Arc<SqliteNoteRepository>,
    pub task_repo: Arc<SqliteTaskRepository>,
    pub task_note_repo: Arc<SqliteTaskNoteRepository>,
    pub notification_service: Arc<NotificationService>,
    pub auth_repo: Arc<LocalAuthRepository>,
    pub settings_repo: Arc<SqliteSettingsRepository>,
    pub mastersys: Arc<MastersysProvider>,
}

fn note_service(state: &State<'_, AppState>) -> masterdesk_application::NoteService {
    // Cheap clone of Arc<dyn NoteRepository>
    let repo: Arc<dyn masterdesk_domain::ports::NoteRepository> =
        state.repo.clone() as Arc<dyn masterdesk_domain::ports::NoteRepository>;
    masterdesk_application::NoteService::new(repo)
}

fn task_service(state: &State<'_, AppState>) -> masterdesk_application::TaskService {
    let task_repo: Arc<dyn masterdesk_domain::ports::TaskRepository> =
        state.task_repo.clone() as Arc<dyn masterdesk_domain::ports::TaskRepository>;
    let ns: Arc<dyn masterdesk_domain::ports::NotificationService> =
        state.notification_service.clone();
    masterdesk_application::TaskService::new(task_repo, Some(ns))
}

fn task_note_service(state: &State<'_, AppState>) -> TaskNoteService {
    let task_repo: Arc<dyn masterdesk_domain::ports::TaskRepository> = state.task_repo.clone();
    let note_repo: Arc<dyn masterdesk_domain::ports::TaskNoteRepository> =
        state.task_note_repo.clone();
    TaskNoteService::new(task_repo, note_repo)
}

fn mastersys_service(state: &State<'_, AppState>) -> MastersysSyncService {
    let provider: Arc<dyn masterdesk_domain::ports::SupportSystemProvider> =
        state.mastersys.clone();
    let task_repo: Arc<dyn masterdesk_domain::ports::TaskRepository> = state.task_repo.clone();
    let note_repo: Arc<dyn masterdesk_domain::ports::TaskNoteRepository> =
        state.task_note_repo.clone();
    let ns: Arc<dyn masterdesk_domain::ports::NotificationService> =
        state.notification_service.clone();
    MastersysSyncService::new(provider, task_repo, note_repo, Some(ns))
}

fn auth_service(state: &State<'_, AppState>) -> AuthService {
    let provider: Arc<dyn masterdesk_domain::ports::AuthenticationProvider> =
        state.auth_repo.clone();
    AuthService::new(provider)
}

// ---------------------------------------------------------------------------
// DTOs (expostos ao frontend)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateNotePayload {
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub priority: Option<String>,
    pub color: Option<String>,
    pub opacity: Option<f32>,
    pub position: Option<(f64, f64)>,
    pub size: Option<(f64, f64)>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateNotePayload {
    pub title: Option<String>,
    pub content: Option<String>,
    pub tags: Option<Vec<String>>,
    pub priority: Option<String>,
    pub color: Option<String>,
    pub opacity: Option<f32>,
    pub deadline: Option<Option<String>>, // Some(None)=clear, None=no change
    pub position: Option<(f64, f64)>,
    pub size: Option<(f64, f64)>,
    pub pinned: Option<bool>,
    pub always_on_top: Option<bool>,
}

fn parse_priority(s: Option<String>) -> Option<Priority> {
    s.and_then(|v| match v.as_str() {
        "Low" => Some(Priority::Low),
        "Medium" => Some(Priority::Medium),
        "High" => Some(Priority::High),
        "Urgent" => Some(Priority::Urgent),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Task DTOs (expostos ao frontend)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTaskPayload {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub priority: Option<String>,
    pub deadline: Option<String>, // ISO8601 UTC
    #[serde(default)]
    pub reminder_thresholds: Option<Vec<i64>>, // minutos antes do deadline
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateTaskPayload {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<String>,
    pub deadline: Option<Option<String>>, // Some(None)=clear, None=no change
    pub reminder_thresholds: Option<Vec<i64>>, // minutos antes do deadline
}

/// Converte minutos (u32) em `ReminderThreshold`. Usa `Minutes` para minutos
/// divisíveis não-custom ou `Custom` conforme entrada; a regra mais simples:
/// qualquer valor inteiro de minutos vira `ReminderThreshold::Minutes(min)`.
fn threshold_minutes_to_enum(mins: i64) -> Option<ReminderThreshold> {
    if mins <= 0 || mins > 10080 {
        return None;
    }
    Some(ReminderThreshold::Minutes(mins as u32))
}

fn parse_deadline(s: Option<String>) -> Result<Option<DateTime<Utc>>, String> {
    s.map(|v| {
        v.parse::<DateTime<Utc>>()
            .map_err(|e| format!("invalid deadline: {e}"))
    })
    .transpose()
}

fn parse_thresholds(v: Option<Vec<i64>>) -> Result<Option<Vec<ReminderThreshold>>, String> {
    match v {
        None => Ok(None),
        Some(vec) => {
            let mut out = Vec::with_capacity(vec.len());
            for m in vec {
                let t = threshold_minutes_to_enum(m)
                    .ok_or_else(|| format!("invalid reminder threshold minutes: {m}"))?;
                out.push(t);
            }
            Ok(Some(out))
        }
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn create_note(
    state: State<'_, AppState>,
    payload: CreateNotePayload,
) -> Result<Note, String> {
    let svc = note_service(&state);
    let input = CreateNoteInput {
        title: payload.title,
        content: payload.content,
        tags: payload.tags,
        priority: parse_priority(payload.priority),
        color: payload.color,
        opacity: payload.opacity,
        position: payload.position,
        size: payload.size,
    };
    svc.create_note(input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_note(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    let svc = note_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.get_note(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_active_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let svc = note_service(&state);
    svc.list_active_notes().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_archived_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let svc = note_service(&state);
    svc.list_archived_notes().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_all_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let svc = note_service(&state);
    svc.list_all_notes().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_note(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateNotePayload,
) -> Result<Note, String> {
    let svc = note_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    // deadline parsing — payload.deadline is Option<Option<String>>
    let deadline_raw = payload.deadline.clone();
    // Se payload.deadline is None => no change; if Some(None) => clear; if Some(Some(str)) => parse
    let deadline: Option<Option<DateTime<Utc>>> = match deadline_raw {
        None => None,
        Some(None) => Some(None),
        Some(Some(s)) => {
            let dt = s
                .parse::<DateTime<Utc>>()
                .map_err(|e| format!("invalid deadline: {e}"))?;
            Some(Some(dt))
        }
    };

    let input = UpdateNoteInput {
        title: payload.title,
        content: payload.content,
        tags: payload.tags,
        priority: parse_priority(payload.priority),
        color: payload.color,
        opacity: payload.opacity,
        deadline,
        position: payload.position,
        size: payload.size,
        pinned: payload.pinned,
        always_on_top: payload.always_on_top,
    };
    svc.update_note(uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn archive_note(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    let svc = note_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.archive_note(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn unarchive_note(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    let svc = note_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.unarchive_note(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_note(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let svc = note_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.delete_note(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_pin(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    let svc = note_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.toggle_pin(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_always_on_top(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<Note, String> {
    let svc = note_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.set_always_on_top(uid, enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_window_always_on_top(window: tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    window.set_always_on_top(enabled).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Note window commands — "pinned notes" that survive main minimize
// ---------------------------------------------------------------------------

/// Abre uma janela dedicada para uma nota. Se já existir, foca ela.
/// Assinatura definida no plano de pinned notes (id/title/color/x/y/w/h).
///
/// P1 — `async` é OBRIGATÓRIO aqui: em Windows, `WebviewWindowBuilder::new`/
/// `build()` deadlocka quando usado em comando síncrono (docs.rs Known issues +
/// tauri#13963 / wry#583). Comando async roda no runtime tokio, e o build é
/// despachado para o thread principal sem bloquear o event loop.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn open_note_window(
    app: tauri::AppHandle,
    id: String,
    title: String,
    _color: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> Result<(), String> {
    let label = format!("note-{}", id);

    // Se já existe, apenas foca
    if let Some(existing) = app.get_webview_window(&label) {
        existing.show().map_err(|e| e.to_string())?;
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    println!(
        "open_note_window: id={} title={} x={} y={} w={} h={}",
        id, title, x, y, w, h
    );

    // P2 — URL UNIFICADA via WebviewUrl::App em dev E prod (removido o
    // cfg!(debug_assertions) com External). Em dev o App resolve para
    // `devUrl` (http://localhost:1420/index.html#note=<id>); em prod para
    // `frontendDist` (tauri://index.html#note=<id>). Unificar garante que a
    // `initialization_script` (window.__NOTE_ID__) rode: no Windows a
    // initialization_script NÃO roda em WebviewUrl::External (wry/tauri).
    // App.tsx já lê search + hash + window.__NOTE_ID__ (fallback).
    let url = WebviewUrl::App(format!("/index.html#note={}", id).into());
    println!("open_note_window url: {:?}", url);

    // Sanitiza o id para o initialization_script: serde_json::to_string produz
    // um literal de string JS válido e escapado — evita quebra de script / XSS
    // via aspas, backslash ou template literals no id.
    let id_literal = serde_json::to_string(&id).unwrap_or_else(|_| "\"\"".to_string());

    // Clampar posição/tamanho: impede abrir a janela fora da área de trabalho
    // se uma posição inválida ficou persistida (ex.: monitor removido).
    const MAX_COORD: f64 = 4096.0;
    const MIN_W: f64 = 180.0;
    const MIN_H: f64 = 140.0;
    let x = x.clamp(0.0, MAX_COORD);
    let y = y.clamp(0.0, MAX_COORD);
    let w = w.clamp(MIN_W, MAX_COORD);
    let h = h.clamp(MIN_H, MAX_COORD);

    let win = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(w, h)
        .position(x, y)
        .resizable(true)
        // P3 — janela frameless sticky: transparente, sem moldura, sempre visível.
        // Esconder moldura novamente (pedido do DEV) após debug com decorations.
        // `transparent(true)` + `decorations(false)` + `shadow(false)` + `visible(false)` + `show()`
        // evita flash branco (Issues #14831/14515/8308). Skip_taskbar true para não poluir barra.
        .decorations(false)
        .transparent(true)
        .shadow(false)
        // always_on_top: janela de nota é on-top por padrão (pin). O estado
        // persistido `note.always_on_top` é aplicado pelo frontend via
        // `set_note_window_always_on_top` quando o usuário alterna o toggle.
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .initialization_script(format!(
            "window.__NOTE_ID__={id_literal};console.log('note-window init', window.__NOTE_ID__, location.href);"
        ))
        .build()
        .map_err(|e| e.to_string())?;

    // Mostra apenas depois do build concluído (init script já registrado),
    // reduzindo o flash branco de "pop-out".
    win.show().map_err(|e| e.to_string())?;

    // `_color` é mantido no contrato para uso futuro (ex.: definir cor de fundo
    // nativa da janela em plataformas que permitam). Hoje o frontend aplica a
    // cor da nota via `?note=` + CSS.
    Ok(())
}

/// Fecha a janela de uma nota.
/// `async` (P1): operações de janela não devem bloquear o thread principal em
/// Windows (mesmo risco de deadlock de `WebviewWindowBuilder::new`).
#[tauri::command]
pub async fn close_note_window(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let label = format!("note-{}", id);
    if let Some(win) = app.get_webview_window(&label) {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Alterna always-on-top de uma janela de nota.
/// `async` (P1): ver `close_note_window`.
#[tauri::command]
pub async fn set_note_window_always_on_top(
    app: tauri::AppHandle,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let label = format!("note-{}", id);
    if let Some(win) = app.get_webview_window(&label) {
        win.set_always_on_top(enabled).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Move a janela de uma nota.
/// `async` (P1): ver `close_note_window`.
#[tauri::command]
pub async fn set_note_window_position(
    app: tauri::AppHandle,
    id: String,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let label = format!("note-{}", id);
    if let Some(win) = app.get_webview_window(&label) {
        use tauri::{LogicalPosition, Position};
        win.set_position(Position::Logical(LogicalPosition { x, y }))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Redimensiona a janela de uma nota.
/// `async` (P1): ver `close_note_window`.
#[tauri::command]
pub async fn set_note_window_size(
    app: tauri::AppHandle,
    id: String,
    w: f64,
    h: f64,
) -> Result<(), String> {
    let label = format!("note-{}", id);
    if let Some(win) = app.get_webview_window(&label) {
        use tauri::{LogicalSize, Size};
        win.set_size(Size::Logical(LogicalSize {
            width: w,
            height: h,
        }))
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Verifica se uma nota tem janela aberta.
#[tauri::command]
pub fn is_note_window_open(app: tauri::AppHandle, id: String) -> Result<bool, String> {
    let label = format!("note-{}", id);
    Ok(app.get_webview_window(&label).is_some())
}

// ---------------------------------------------------------------------------
// Task commands (Fase 3)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn create_task(
    state: State<'_, AppState>,
    payload: CreateTaskPayload,
) -> Result<Task, String> {
    let svc = task_service(&state);
    let input = CreateTaskInput {
        title: payload.title,
        description: payload.description,
        priority: parse_priority(payload.priority),
        deadline: parse_deadline(payload.deadline)?,
        reminder_thresholds: parse_thresholds(payload.reminder_thresholds)?,
    };
    svc.create_task(input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_task(state: State<'_, AppState>, id: String) -> Result<Task, String> {
    let svc = task_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.get_task(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_pending_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    let svc = task_service(&state);
    svc.list_pending_tasks().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_completed_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    let svc = task_service(&state);
    svc.list_completed_tasks().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_all_tasks(state: State<'_, AppState>) -> Result<Vec<Task>, String> {
    let svc = task_service(&state);
    svc.list_all_tasks().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_task(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateTaskPayload,
) -> Result<Task, String> {
    let svc = task_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;

    let deadline: Option<Option<DateTime<Utc>>> = match payload.deadline {
        None => None,
        Some(None) => Some(None),
        Some(Some(s)) => {
            let dt = s
                .parse::<DateTime<Utc>>()
                .map_err(|e| format!("invalid deadline: {e}"))?;
            Some(Some(dt))
        }
    };

    let input = UpdateTaskInput {
        title: payload.title,
        description: payload.description,
        priority: parse_priority(payload.priority),
        deadline,
        reminder_thresholds: parse_thresholds(payload.reminder_thresholds)?,
    };
    svc.update_task(uid, input).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn complete_task(state: State<'_, AppState>, id: String) -> Result<Task, String> {
    let svc = task_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.complete_task(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reopen_task(state: State<'_, AppState>, id: String) -> Result<Task, String> {
    let svc = task_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.reopen_task(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let svc = task_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.delete_task(uid).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn snooze_task(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let svc = task_service(&state);
    let uid = id.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
    svc.snooze_task(uid).await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Auth commands (Fase 4) — autenticação local isolada
// ---------------------------------------------------------------------------

/// Resposta de auth enviada ao frontend. **Nunca** inclui `password_hash`.
#[derive(Debug, Serialize)]
pub struct AuthPayload {
    pub id: String,
    pub username: String,
    pub created_at: String,
    pub authenticated: bool,
}

impl From<UserView> for AuthPayload {
    fn from(v: UserView) -> Self {
        Self {
            id: v.id.to_string(),
            username: v.username,
            created_at: v.created_at.to_rfc3339(),
            authenticated: true,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RegisterPayload {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[tauri::command]
pub async fn auth_register(
    state: State<'_, AppState>,
    payload: RegisterPayload,
) -> Result<AuthPayload, String> {
    let svc = auth_service(&state);
    let res = svc
        .register(CreateUserInput {
            username: payload.username,
            password: payload.password,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(AuthPayload::from(res.user))
}

#[tauri::command]
pub async fn auth_login(
    state: State<'_, AppState>,
    payload: LoginPayload,
) -> Result<AuthPayload, String> {
    let svc = auth_service(&state);
    let res = svc
        .login(LoginInput {
            username: payload.username,
            password: payload.password,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(AuthPayload::from(res.user))
}

#[tauri::command]
pub async fn auth_logout(state: State<'_, AppState>) -> Result<(), String> {
    let svc = auth_service(&state);
    svc.logout().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn auth_is_authenticated(state: State<'_, AppState>) -> Result<bool, String> {
    let svc = auth_service(&state);
    svc.is_authenticated().await.map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Anotações dentro de tarefas
// ---------------------------------------------------------------------------

fn parse_uuid(id: &str) -> Result<uuid::Uuid, String> {
    id.parse::<uuid::Uuid>()
        .map_err(|_| "identificador inválido".to_string())
}

#[tauri::command]
pub async fn add_task_note(
    state: State<'_, AppState>,
    task_id: String,
    content: String,
) -> Result<TaskNote, String> {
    task_note_service(&state)
        .add_note(parse_uuid(&task_id)?, content)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_task_notes(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Vec<TaskNote>, String> {
    task_note_service(&state)
        .list_notes(parse_uuid(&task_id)?)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn count_task_notes(state: State<'_, AppState>, task_id: String) -> Result<u32, String> {
    task_note_service(&state)
        .count_notes(parse_uuid(&task_id)?)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_task_note(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> Result<TaskNote, String> {
    task_note_service(&state)
        .update_note(parse_uuid(&id)?, content)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_task_note_done(
    state: State<'_, AppState>,
    id: String,
    done: bool,
) -> Result<TaskNote, String> {
    task_note_service(&state)
        .set_note_done(parse_uuid(&id)?, done)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_task_note(state: State<'_, AppState>, id: String) -> Result<(), String> {
    task_note_service(&state)
        .delete_note(parse_uuid(&id)?)
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Integração Mastersys (ADR-006) — somente leitura
// ---------------------------------------------------------------------------

/// Estado da integração para a UI. **Nunca** inclui token ou senha
/// (CLAUDE §13: "Never log secrets" — e nunca devolvê-los ao frontend).
#[derive(Debug, Serialize)]
pub struct MastersysStatus {
    /// Endpoint salvo, se houver. É configuração, não segredo.
    pub endpoint: Option<String>,
    /// True quando há endpoint + sessão + usuário — ou seja, dá para sincronizar.
    pub connected: bool,
    /// Identidade do usuário na origem, para a UI mostrar quem está conectado.
    pub identity: Option<SupportIdentity>,
}

#[derive(Debug, Serialize)]
pub struct SyncReportPayload {
    pub imported: u32,
    pub updated: u32,
    pub removed: u32,
    pub kept_with_notes: u32,
}

impl From<SyncReport> for SyncReportPayload {
    fn from(r: SyncReport) -> Self {
        Self {
            imported: r.imported,
            updated: r.updated,
            removed: r.removed,
            kept_with_notes: r.kept_with_notes,
        }
    }
}

#[tauri::command]
pub async fn mastersys_status(state: State<'_, AppState>) -> Result<MastersysStatus, String> {
    let endpoint = state
        .mastersys
        .base_url()
        .await
        .map_err(|e| e.to_string())?;
    let svc = mastersys_service(&state);
    Ok(MastersysStatus {
        endpoint,
        connected: svc.is_configured().await,
        identity: svc.current_identity().await.map_err(|e| e.to_string())?,
    })
}

#[tauri::command]
pub async fn mastersys_set_endpoint(
    state: State<'_, AppState>,
    endpoint: String,
) -> Result<(), String> {
    state
        .mastersys
        .set_base_url(&endpoint)
        .await
        .map_err(|e| e.to_string())
}

/// Autentica no Mastersys. A senha é usada e descartada: só o refresh token
/// vai para o cofre do SO (`SecretStore`), nunca a senha.
#[tauri::command]
pub async fn mastersys_connect(
    state: State<'_, AppState>,
    identifier: String,
    password: String,
) -> Result<SupportIdentity, String> {
    mastersys_service(&state)
        .connect(&identifier, &password)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mastersys_disconnect(state: State<'_, AppState>) -> Result<SyncReportPayload, String> {
    mastersys_service(&state)
        .disconnect()
        .await
        .map(SyncReportPayload::from)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mastersys_sync(
    state: State<'_, AppState>,
    default_reminders: Option<Vec<i64>>,
) -> Result<SyncReportPayload, String> {
    let default_reminders = default_reminders
        .unwrap_or_default()
        .into_iter()
        .filter_map(threshold_minutes_to_enum)
        .collect();
    mastersys_service(&state)
        .sync(SyncOptions { default_reminders })
        .await
        .map(SyncReportPayload::from)
        .map_err(|e| e.to_string())
}
