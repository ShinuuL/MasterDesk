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
use masterdesk_domain::{
    ExternalWorkItem, Note, Priority, ReminderThreshold, SupportIdentity, Task, TaskNote,
};
use masterdesk_infrastructure::{
    LocalAuthRepository, MastersysProvider, MastersysTicketStatus, NotificationService,
    SqliteNoteRepository, SqliteSettingsRepository, SqliteTaskNoteRepository, SqliteTaskRepository,
    SqliteTaskWindowRepository,
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
    pub task_window_repo: Arc<SqliteTaskWindowRepository>,
    pub mastersys: Arc<MastersysProvider>,
    /// Ponta para pedir sincronização ao agendador em segundo plano.
    pub sync_handle: crate::sync_scheduler::SyncHandle,
    /// Dono do canal de tempo real. Ligado/desligado junto com a sessão.
    pub realtime: Arc<crate::realtime_supervisor::RealtimeSupervisor>,
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
/// Tamanho mínimo utilizável de uma janela de nota, e um teto de sanidade.
const MIN_WIN_W: f64 = 180.0;
const MIN_WIN_H: f64 = 140.0;
const MAX_WIN_DIM: f64 = 4096.0;

/// Quanto da janela precisa estar dentro de um monitor para ela contar como
/// alcançável. 64 px de barra de título já bastam para o usuário pegá-la.
const MIN_VISIBLE_PX: f64 = 64.0;

fn clamp_window_size(w: f64, h: f64) -> (f64, f64) {
    let w = if w.is_finite() { w } else { 300.0 };
    let h = if h.is_finite() { h } else { 250.0 };
    (
        w.clamp(MIN_WIN_W, MAX_WIN_DIM),
        h.clamp(MIN_WIN_H, MAX_WIN_DIM),
    )
}

/// Posição salva, se ela deixar a janela alcançável em algum monitor ligado.
/// `None` significa "não use posição salva" — quem chama centraliza.
///
/// ## O bug que isto corrige
///
/// Antes a posição era só `x.clamp(0.0, 4096.0)`, uma constante. O comentário
/// dizia proteger contra "monitor removido", mas era exatamente o caso que
/// escapava: uma nota salva em `x = 2500`, num ambiente que depois passa a ter
/// um único monitor de 1920, tem `2500 < 4096` e passava intacta. A janela
/// abria **fora da tela**.
///
/// E aí não havia volta: a janela existia, então `is_note_window_open` dizia
/// `true` e o quadro continuava escondendo a nota; `skip_taskbar(true)` tirava
/// o ícone da barra; e arrastar estava quebrado por falta de permissão na ACL.
/// A nota só voltava editando o banco.
///
/// Trabalhamos em coordenadas físicas convertidas para lógicas porque
/// `Monitor::position/size` vêm em físico, enquanto o builder posiciona em
/// lógico — misturar os dois erra em qualquer tela com escala != 100%, que é o
/// padrão em notebook Windows.
fn visible_position(app: &tauri::AppHandle, x: f64, y: f64, w: f64, h: f64) -> Option<(f64, f64)> {
    let monitors = app.available_monitors().ok()?;
    // Físico -> lógico: `Monitor::position/size` vêm em pixels físicos, mas o
    // builder posiciona em lógicos. Misturar os dois erra em qualquer tela com
    // escala != 100%, que é o padrão em notebook Windows.
    let rects: Vec<LogicalRect> = monitors
        .iter()
        .filter_map(|m| {
            let scale = m.scale_factor();
            if !scale.is_finite() || scale <= 0.0 {
                return None;
            }
            let pos = m.position();
            let size = m.size();
            Some(LogicalRect {
                left: f64::from(pos.x) / scale,
                top: f64::from(pos.y) / scale,
                right: f64::from(pos.x) / scale + f64::from(size.width) / scale,
                bottom: f64::from(pos.y) / scale + f64::from(size.height) / scale,
            })
        })
        .collect();

    position_is_reachable(x, y, w, h, &rects).then_some((x, y))
}

/// Retângulo de um monitor em coordenadas lógicas.
#[derive(Debug, Clone, Copy)]
struct LogicalRect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

/// A janela em `(x, y, w, h)` tem pedaço suficiente dentro de algum monitor
/// para o usuário conseguir pegá-la?
///
/// Separado de [`visible_position`] só para poder ser testado — a outra metade
/// depende de um `AppHandle`, que exige app rodando.
///
/// Lista de monitores vazia devolve `false`: sem informação não há como julgar,
/// e centralizar é o palpite seguro. Insistir na posição salva era justamente o
/// bug.
fn position_is_reachable(x: f64, y: f64, w: f64, h: f64, monitors: &[LogicalRect]) -> bool {
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    monitors.iter().any(|m| {
        let overlap_w = (x + w).min(m.right) - x.max(m.left);
        let overlap_h = (y + h).min(m.bottom) - y.max(m.top);
        // `min(w)`/`min(h)` para uma janela menor que o limiar não ser
        // considerada inalcançável por ser pequena.
        overlap_w >= MIN_VISIBLE_PX.min(w) && overlap_h >= MIN_VISIBLE_PX.min(h)
    })
}

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

    let (w, h) = clamp_window_size(w, h);
    // Posição contra os monitores REAIS. Ver `visible_position` para o bug que
    // isso corrige — janela abrindo invisível e sem como recuperar.
    let position = visible_position(&app, x, y, w, h);

    let mut builder = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(w, h)
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
        ));

    // Só posiciona quando a posição salva é utilizável; senão deixa o SO
    // decidir e centraliza depois do build. Chamar `.position()` com valor
    // inválido é justamente o que punha a janela fora da tela.
    builder = match position {
        Some((px, py)) => builder.position(px, py),
        None => builder.center(),
    };

    let win = builder.build().map_err(|e| e.to_string())?;

    // Mostra apenas depois do build concluído (init script já registrado),
    // reduzindo o flash branco de "pop-out".
    win.show().map_err(|e| e.to_string())?;

    // `_color` é mantido no contrato para uso futuro (ex.: definir cor de fundo
    // nativa da janela em plataformas que permitam). Hoje o frontend aplica a
    // cor da nota via `?note=` + CSS.
    Ok(())
}

// ---------------------------------------------------------------------------
// Pop-out de tarefa
//
// Mesmo desenho das janelas de nota, e de propósito: o usuário já aprendeu que
// um pop-out é frameless, arrastável pelo cabeçalho e fica por cima. O que NÃO
// foi copiado são os quatro defeitos que as notas tinham — posição validada
// contra os monitores reais, e a lista de janelas abertas vem do gerenciador de
// janelas em vez de estado de componente.
// ---------------------------------------------------------------------------

/// Destaca uma tarefa em janela própria.
///
/// `async` por obrigação: `WebviewWindowBuilder::build()` trava o thread
/// principal quando chamado de comando síncrono no Windows — ver o comentário
/// em `open_note_window`.
#[tauri::command]
pub async fn open_task_window(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let label = format!("task-{}", id);

    // Idempotente: segundo clique traz a janela existente para a frente em vez
    // de criar outra.
    if let Some(existing) = app.get_webview_window(&label) {
        existing.show().map_err(|e| e.to_string())?;
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    let task_id = uuid::Uuid::parse_str(&id).map_err(|_| "id de tarefa inválido".to_string())?;
    // Confere que a tarefa existe ANTES de abrir: uma janela apontando para
    // tarefa inexistente mostraria erro e não teria como se recuperar.
    // Via `TaskService` e não pelo repositório, como o resto deste arquivo —
    // `get_task` já devolve `NotFound` em vez de `Option`.
    let task = task_service(&state)
        .get_task(task_id)
        .await
        .map_err(|e| e.to_string())?;

    let win_state = state
        .task_window_repo
        .get(task_id)
        .await
        .map_err(|e| e.to_string())?;

    let (w, h) = clamp_window_size(win_state.size.0, win_state.size.1);
    let position = visible_position(&app, win_state.position.0, win_state.position.1, w, h);

    let id_literal = serde_json::to_string(&id).unwrap_or_else(|_| "\"\"".to_string());
    let url = WebviewUrl::App(format!("/index.html#task={}", id).into());

    let mut builder = WebviewWindowBuilder::new(&app, &label, url)
        // Título ajuda quem usa Alt+Tab ou leitor de tela, mesmo sem moldura.
        .title(&task.title)
        .inner_size(w, h)
        .resizable(true)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(win_state.always_on_top)
        .skip_taskbar(true)
        .visible(false)
        .initialization_script(format!("window.__TASK_ID__={id_literal};"));

    builder = match position {
        Some((px, py)) => builder.position(px, py),
        None => builder.center(),
    };

    let win = builder.build().map_err(|e| e.to_string())?;
    win.show().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn close_task_window(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let label = format!("task-{}", id);
    if let Some(win) = app.get_webview_window(&label) {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Ids das tarefas com janela aberta agora.
///
/// Mesma razão de `open_note_window_ids`: o gerenciador de janelas é a fonte da
/// verdade, e a resposta sobrevive à remontagem do componente na troca de aba.
/// Serve também para o quadro fechar janelas de espelhos que o `retire_mirror`
/// apagou — um id daqui sem tarefa correspondente é uma janela órfã.
#[tauri::command]
pub fn open_task_window_ids(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    Ok(app
        .webview_windows()
        .keys()
        .filter_map(|label| label.strip_prefix("task-").map(str::to_string))
        .collect())
}

// GRAVAR geometria e APLICAR geometria são operações separadas, e misturá-las
// é um laço de realimentação.
//
// O bug, capturado em vídeo pelo DEV em 2026-09-03: a janela destacada
// "agitava freneticamente". A primeira versão destes comandos gravava no banco
// **e** chamava `win.set_position` / `win.set_size`. Como quem os chama é o
// listener de `onMoved`/`onResized`, o ciclo era:
//
//     usuário arrasta -> onMoved -> comando -> win.set_position
//         -> o SO move a janela -> onMoved -> comando -> ...
//
// E não convergia: a posição vai para o frontend em pixels FÍSICOS, é dividida
// pela escala, volta em LÓGICOS e é reconvertida — cada volta arredonda para
// uma coordenada um pouco diferente, então a janela oscilava em vez de
// estabilizar.
//
// O pop-out de nota nunca teve isso porque o `onMoved` dele chama `update_note`,
// que só persiste. Estes comandos agora fazem o mesmo, e o nome diz isso.

/// Grava a posição da janela destacada. **Não move a janela.**
#[tauri::command]
pub async fn save_task_window_position(
    state: State<'_, AppState>,
    id: String,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let task_id = uuid::Uuid::parse_str(&id).map_err(|_| "id de tarefa inválido".to_string())?;
    state
        .task_window_repo
        .set_position(task_id, x, y)
        .await
        .map_err(|e| e.to_string())
}

/// Grava o tamanho da janela destacada. **Não redimensiona a janela.**
#[tauri::command]
pub async fn save_task_window_size(
    state: State<'_, AppState>,
    id: String,
    w: f64,
    h: f64,
) -> Result<(), String> {
    let task_id = uuid::Uuid::parse_str(&id).map_err(|_| "id de tarefa inválido".to_string())?;
    state
        .task_window_repo
        .set_size(task_id, w, h)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_task_window_always_on_top(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let task_id = uuid::Uuid::parse_str(&id).map_err(|_| "id de tarefa inválido".to_string())?;
    // Banco primeiro: se a janela for fechada logo depois, a preferência
    // sobrevive para a próxima abertura.
    state
        .task_window_repo
        .set_always_on_top(task_id, enabled)
        .await
        .map_err(|e| e.to_string())?;
    if let Some(win) = app.get_webview_window(&format!("task-{}", id)) {
        win.set_always_on_top(enabled).map_err(|e| e.to_string())?;
    }
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

/// Ids das notas que têm janela aberta agora.
///
/// ## Por que existe
///
/// O `NotesBoard` mantinha esse conjunto em `useState` e escondia do quadro
/// toda nota que estivesse nele. Dois problemas vinham disso:
///
/// 1. **Trocar de aba zerava o conjunto.** `App.tsx` renderiza o `NotesBoard`
///    só quando a aba é "notes", então ir para Tarefas o desmonta. Ao voltar, a
///    nota reaparecia no quadro *com a janela dela ainda aberta* — duas
///    superfícies editando a mesma nota, sobrescrevendo uma à outra.
/// 2. **Consultar uma a uma** com `is_note_window_open` custava um `invoke` por
///    nota e, se uma falhasse, o código antigo mantinha a nota escondida.
///
/// Perguntar ao gerenciador de janelas de uma vez resolve os dois: ele é quem
/// realmente sabe, e a resposta sobrevive a qualquer remontagem de componente.
#[tauri::command]
pub fn open_note_window_ids(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    Ok(app
        .webview_windows()
        .keys()
        .filter_map(|label| label.strip_prefix("note-").map(str::to_string))
        .collect())
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
    /// Janela, em dias, de chamados buscados na sincronização. Exposta porque
    /// explica uma ausência que confundiria: chamado aberto mais antigo que a
    /// janela não aparece no quadro.
    pub ticket_window_days: i64,
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
        ticket_window_days: state
            .mastersys
            .ticket_window_days()
            .await
            .map_err(|e| e.to_string())?,
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
        .map_err(|e| e.to_string())?;
    // Endereço novo: o canal precisa apontar para o servidor novo.
    state.realtime.reevaluate(Some(&endpoint));
    Ok(())
}

/// Catálogo de status espelhado do Mastersys, para a UI montar o filtro e o
/// selo de status. Lê só o banco local — não toca a rede.
///
/// Lista vazia é resposta legítima (nunca sincronizou, ou o endpoint falhou),
/// e a UI cai no comportamento sem catálogo.
#[tauri::command]
pub async fn mastersys_status_catalog(
    state: State<'_, AppState>,
) -> Result<Vec<MastersysTicketStatus>, String> {
    state
        .mastersys
        .status_catalog()
        .await
        .map_err(|e| e.to_string())
}

/// Busca ao vivo no acervo de chamados do Mastersys.
///
/// Complementa o filtro local, que só alcança o que já está na sua fila. O
/// resultado é **consulta**: ver `MastersysProvider::search_tickets` para por
/// que um chamado achado aqui não pode ser gravado como espelho.
#[tauri::command]
pub async fn mastersys_search_tickets(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<ExternalWorkItem>, String> {
    state
        .mastersys
        .search_tickets(&query)
        .await
        .map_err(|e| e.to_string())
}

/// Ajusta a janela de chamados. Ver `MastersysProvider::fetch_tickets` para
/// por que o recorte é por data e não por status.
#[tauri::command]
pub async fn mastersys_set_ticket_window(
    state: State<'_, AppState>,
    days: i64,
) -> Result<(), String> {
    state
        .mastersys
        .set_ticket_window_days(days)
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
    let identity = mastersys_service(&state)
        .connect(&identifier, &password)
        .await
        .map_err(|e| e.to_string())?;

    // Sessão nova: liga o tempo real e pede a primeira sincronização agora, em
    // vez de deixar o usuário esperando o timer.
    if let Ok(url) = state.mastersys.base_url().await {
        state.realtime.reevaluate(url.as_deref());
    }
    state
        .sync_handle
        .request(crate::sync_scheduler::SyncTrigger::Requested);

    Ok(identity)
}

#[tauri::command]
pub async fn mastersys_disconnect(state: State<'_, AppState>) -> Result<SyncReportPayload, String> {
    // Canal primeiro: manter o socket aberto depois de desconectar traria
    // eventos que disparariam sincronização sem sessão para sincronizar.
    state.realtime.stop();
    mastersys_service(&state)
        .disconnect()
        .await
        .map(SyncReportPayload::from)
        .map_err(|e| e.to_string())
}

/// O que aconteceu na última sincronização automática.
///
/// `None` = nenhuma rodou ainda nesta execução do app. Serve para responder
/// "está demorando ou nem acontecendo?" sem precisar de log.
#[tauri::command]
pub fn mastersys_last_sync(state: State<'_, AppState>) -> Option<crate::sync_scheduler::LastSync> {
    state.sync_handle.last_sync()
}

/// Pede uma sincronização agora, sem esperar o timer.
#[tauri::command]
pub fn mastersys_sync_now(state: State<'_, AppState>) {
    state
        .sync_handle
        .request(crate::sync_scheduler::SyncTrigger::Requested);
}

/// O canal de tempo real está conectado?
///
/// A UI usa para dizer se o quadro acompanha em segundos (tempo real) ou em
/// minutos (polling) — sem isso o usuário não tem como saber por que uma
/// mudança levou 5 minutos para aparecer.
#[tauri::command]
pub fn mastersys_realtime_connected(state: State<'_, AppState>) -> bool {
    state.realtime.is_connected()
}

#[tauri::command]
pub async fn mastersys_sync(
    state: State<'_, AppState>,
    default_reminders: Option<Vec<i64>>,
) -> Result<SyncReportPayload, String> {
    let minutes = default_reminders.unwrap_or_default();

    // Guarda a escolha para a sincronização AUTOMÁTICA aplicar os mesmos
    // lembretes. Sem isto, um item importado pelo timer nasceria sem lembrete
    // enquanto o mesmo item importado por este botão nasceria com — diferença
    // invisível que só apareceria como "o alarme não tocou".
    let _ = crate::sync_scheduler::store_default_reminders(&state.settings_repo, &minutes).await;

    let default_reminders = minutes
        .into_iter()
        .filter_map(threshold_minutes_to_enum)
        .collect();
    mastersys_service(&state)
        .sync(SyncOptions { default_reminders })
        .await
        .map(SyncReportPayload::from)
        .map_err(|e| e.to_string())
}

/// Intervalo atual do polling, em segundos. A UI mostra para o usuário saber
/// de quanto em quanto tempo o quadro se atualiza sozinho.
#[tauri::command]
pub async fn mastersys_poll_interval(state: State<'_, AppState>) -> Result<u64, String> {
    Ok(crate::sync_scheduler::poll_interval(&state.settings_repo)
        .await
        .as_secs())
}

#[tauri::command]
pub async fn mastersys_set_poll_interval(
    state: State<'_, AppState>,
    seconds: u64,
) -> Result<(), String> {
    crate::sync_scheduler::set_poll_interval(&state.settings_repo, seconds).await?;
    // Pede um ciclo agora para o novo intervalo valer sem esperar o antigo
    // terminar — o laço relê a configuração no começo de cada volta.
    state
        .sync_handle
        .request(crate::sync_scheduler::SyncTrigger::Requested);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Monitor único 1920x1080 na origem, escala 100% — o caso comum.
    fn primary() -> LogicalRect {
        LogicalRect {
            left: 0.0,
            top: 0.0,
            right: 1920.0,
            bottom: 1080.0,
        }
    }

    /// Segundo monitor à direita.
    fn right_of_primary() -> LogicalRect {
        LogicalRect {
            left: 1920.0,
            top: 0.0,
            right: 3840.0,
            bottom: 1080.0,
        }
    }

    /// Segundo monitor à ESQUERDA — coordenadas negativas, arranjo comum e o
    /// que o clamp antigo (`0.0..4096.0`) destruía ao empurrar para x = 0.
    fn left_of_primary() -> LogicalRect {
        LogicalRect {
            left: -1920.0,
            top: 0.0,
            right: 0.0,
            bottom: 1080.0,
        }
    }

    #[test]
    fn position_inside_the_primary_monitor_is_reachable() {
        assert!(position_is_reachable(
            100.0,
            100.0,
            300.0,
            250.0,
            &[primary()]
        ));
    }

    /// O bug relatado: nota "some e não volta".
    ///
    /// Salva em x = 2500 num ambiente que passou a ter só 1920 de largura. O
    /// clamp antigo era `x.clamp(0.0, 4096.0)`, e 2500 < 4096, então passava
    /// intacto e a janela abria fora da tela — sem barra de tarefas
    /// (`skip_taskbar`) e sem poder arrastar (ACL), era irrecuperável.
    #[test]
    fn position_on_a_monitor_that_no_longer_exists_is_not_reachable() {
        assert!(
            !position_is_reachable(2500.0, 300.0, 300.0, 250.0, &[primary()]),
            "posicao fora de todo monitor tem de ser recusada, ainda que < 4096"
        );
    }

    #[test]
    fn the_same_position_is_reachable_when_the_second_monitor_is_present() {
        assert!(position_is_reachable(
            2500.0,
            300.0,
            300.0,
            250.0,
            &[primary(), right_of_primary()]
        ));
    }

    /// Monitor à esquerda tem x negativo. Precisa ser aceito, senão a nota
    /// salta para o monitor primário a cada abertura.
    #[test]
    fn negative_coordinates_are_valid_on_a_left_hand_monitor() {
        assert!(position_is_reachable(
            -1500.0,
            200.0,
            300.0,
            250.0,
            &[primary(), left_of_primary()]
        ));
    }

    #[test]
    fn a_sliver_on_screen_still_counts_as_reachable() {
        // 100 px dentro da borda direita: dá para pegar a barra de título.
        assert!(position_is_reachable(
            1820.0,
            500.0,
            300.0,
            250.0,
            &[primary()]
        ));
    }

    #[test]
    fn almost_entirely_off_screen_is_not_reachable() {
        // Só 10 px dentro — abaixo do limiar de 64 px.
        assert!(!position_is_reachable(
            1910.0,
            500.0,
            300.0,
            250.0,
            &[primary()]
        ));
    }

    #[test]
    fn window_smaller_than_the_threshold_is_judged_by_its_own_size() {
        // Janela de 40 px não pode ser reprovada por ser menor que 64.
        assert!(position_is_reachable(
            100.0,
            100.0,
            40.0,
            40.0,
            &[primary()]
        ));
    }

    #[test]
    fn no_monitor_information_means_do_not_trust_the_saved_position() {
        assert!(!position_is_reachable(100.0, 100.0, 300.0, 250.0, &[]));
    }

    #[test]
    fn non_finite_coordinates_are_rejected() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(!position_is_reachable(
                bad,
                100.0,
                300.0,
                250.0,
                &[primary()]
            ));
            assert!(!position_is_reachable(
                100.0,
                bad,
                300.0,
                250.0,
                &[primary()]
            ));
        }
    }

    #[test]
    fn size_is_clamped_into_something_usable() {
        assert_eq!(clamp_window_size(10.0, 10.0), (MIN_WIN_W, MIN_WIN_H));
        assert_eq!(
            clamp_window_size(99_999.0, 99_999.0),
            (MAX_WIN_DIM, MAX_WIN_DIM)
        );
        assert_eq!(clamp_window_size(400.0, 320.0), (400.0, 320.0));
    }

    #[test]
    fn non_finite_size_falls_back_to_the_default_instead_of_panicking() {
        // `clamp` entra em panic se o valor for NaN, então o guard vem antes.
        assert_eq!(clamp_window_size(f64::NAN, f64::NAN), (300.0, 250.0));
    }
}
