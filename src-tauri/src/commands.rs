//! Comandos Tauri para Notes (Fase 2) e Tasks (Fase 3).
//! Cada comando delega para `masterdesk-application::NoteService` / `TaskService`
//! que orquestra validação de domínio + persistência via repositórios SQLite.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use masterdesk_application::{CreateNoteInput, CreateTaskInput, UpdateNoteInput, UpdateTaskInput};
use masterdesk_domain::{Note, Priority, ReminderThreshold, Task};
use masterdesk_infrastructure::{NotificationService, SqliteNoteRepository, SqliteTaskRepository};
use serde::{Deserialize, Serialize};
use tauri::State;

// ---------------------------------------------------------------------------
// Estado
// ---------------------------------------------------------------------------

pub struct AppState {
    pub repo: Arc<SqliteNoteRepository>,
    pub task_repo: Arc<SqliteTaskRepository>,
    pub notification_service: Arc<NotificationService>,
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
