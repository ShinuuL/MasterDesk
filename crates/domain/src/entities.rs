//! Entidades do domínio. Nenhum tipo aqui conhece SQLite, Tauri ou HTTP.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type NoteId = Uuid;
pub type TaskId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

/// Nota independente de qualquer ticket do Mastersys (seção 5 do CLAUDE.md:
/// "Notes must not require a Mastersys ticket").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    pub title: String,
    pub content: String,
    pub tags: Vec<String>,
    pub priority: Priority,
    pub deadline: Option<DateTime<Utc>>,
    pub color: String,
    pub opacity: f32,
    pub pinned: bool,
    pub always_on_top: bool,
    pub archived: bool,
    pub position: (f64, f64),
    pub size: (f64, f64),
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Task local — também independente de Mastersys (seção 5 do CLAUDE.md).
/// A ligação com um ticket Mastersys, quando existir, será um campo opcional
/// de metadado de integração, adicionado apenas na Fase 5 (ADR-006).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub description: String,
    pub priority: Priority,
    pub deadline: Option<DateTime<Utc>>,
    pub reminder_thresholds: Vec<ReminderThreshold>,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Threshold de lembrete (seção 8 do CLAUDE.md: 5m/10m/15m/30m/1h/2h/custom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReminderThreshold {
    Minutes(u32),
    Hours(u32),
    Custom { minutes_before: u32 },
}
