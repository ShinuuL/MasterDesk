//! Entidades do domínio. Nenhum tipo aqui conhece SQLite, Tauri ou HTTP.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{DomainError, DomainResult};

pub type NoteId = Uuid;
pub type TaskId = Uuid;
pub type UserId = Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Priority {
    Low,
    #[default]
    Medium,
    High,
    Urgent,
}

/// Nota independente de qualquer ticket do Mastersys (seção 5 do CLAUDE.md:
/// "Notes must not require a Mastersys ticket").
///
/// Seção 6 do CLAUDE.md: title, content, tags, priority, deadline, reminder,
/// completion, color, theme, size, position, opacity, pinning, always-on-top,
/// archive, delete. `theme` é modelado via `color` + `opacity` nesta fase;
/// extensões futuras podem adicionar campo dedicado sem quebrar schema.
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

impl Note {
    /// Cria uma nova nota com validações de domínio.
    pub fn new(title: impl Into<String>, content: impl Into<String>) -> DomainResult<Self> {
        let title = title.into();
        let content = content.into();
        validate_title(&title)?;
        validate_content(&content)?;
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            title: title.trim().to_string(),
            content,
            tags: Vec::new(),
            priority: Priority::default(),
            deadline: None,
            color: default_color(),
            opacity: 1.0,
            pinned: false,
            always_on_top: false,
            archived: false,
            position: (100.0, 100.0),
            size: (300.0, 250.0),
            created_at: now,
            updated_at: now,
        })
    }

    /// Reconstrói uma nota a partir de dados persistidos (sem revalidar geração de id/timestamps).
    /// Usado por adapters de infraestrutura.
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: NoteId,
        title: String,
        content: String,
        tags: Vec<String>,
        priority: Priority,
        deadline: Option<DateTime<Utc>>,
        color: String,
        opacity: f32,
        pinned: bool,
        always_on_top: bool,
        archived: bool,
        position: (f64, f64),
        size: (f64, f64),
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        validate_title(&title)?;
        validate_content(&content)?;
        validate_color(&color)?;
        validate_opacity(opacity)?;
        validate_tags(&tags)?;
        validate_position(position)?;
        validate_size(size)?;
        Ok(Self {
            id,
            title: title.trim().to_string(),
            content,
            tags: normalize_tags(tags),
            priority,
            deadline,
            color,
            opacity,
            pinned,
            always_on_top,
            archived,
            position,
            size,
            created_at,
            updated_at,
        })
    }

    pub fn set_title(&mut self, title: impl Into<String>) -> DomainResult<()> {
        let t = title.into();
        validate_title(&t)?;
        self.title = t.trim().to_string();
        self.touch();
        Ok(())
    }

    pub fn set_content(&mut self, content: impl Into<String>) -> DomainResult<()> {
        let c = content.into();
        validate_content(&c)?;
        self.content = c;
        self.touch();
        Ok(())
    }

    pub fn set_color(&mut self, color: impl Into<String>) -> DomainResult<()> {
        let c = color.into();
        validate_color(&c)?;
        self.color = c;
        self.touch();
        Ok(())
    }

    pub fn set_opacity(&mut self, opacity: f32) -> DomainResult<()> {
        validate_opacity(opacity)?;
        self.opacity = opacity;
        self.touch();
        Ok(())
    }

    pub fn set_position(&mut self, x: f64, y: f64) -> DomainResult<()> {
        validate_position((x, y))?;
        self.position = (x, y);
        self.touch();
        Ok(())
    }

    pub fn set_size(&mut self, w: f64, h: f64) -> DomainResult<()> {
        validate_size((w, h))?;
        self.size = (w, h);
        self.touch();
        Ok(())
    }

    pub fn set_tags(&mut self, tags: Vec<String>) -> DomainResult<()> {
        validate_tags(&tags)?;
        self.tags = normalize_tags(tags);
        self.touch();
        Ok(())
    }

    pub fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
        self.touch();
    }

    pub fn set_deadline(&mut self, deadline: Option<DateTime<Utc>>) {
        self.deadline = deadline;
        self.touch();
    }

    pub fn set_pinned(&mut self, pinned: bool) {
        self.pinned = pinned;
        self.touch();
    }

    pub fn set_always_on_top(&mut self, enabled: bool) {
        self.always_on_top = enabled;
        self.touch();
    }

    pub fn archive(&mut self) {
        self.archived = true;
        self.touch();
    }

    pub fn unarchive(&mut self) {
        self.archived = false;
        self.touch();
    }

    pub fn is_active(&self) -> bool {
        !self.archived
    }

    fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

// ---------------------------------------------------------------------------
// Validações puras (sem I/O)
// ---------------------------------------------------------------------------

fn default_color() -> String {
    "#FFEB3B".to_string()
}

fn validate_title(title: &str) -> DomainResult<()> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(DomainError::Validation("title must not be empty".into()));
    }
    if trimmed.chars().count() > 200 {
        return Err(DomainError::Validation("title must be <= 200 chars".into()));
    }
    Ok(())
}

fn validate_content(content: &str) -> DomainResult<()> {
    if content.chars().count() > 20000 {
        return Err(DomainError::Validation(
            "content must be <= 20000 chars".into(),
        ));
    }
    Ok(())
}

fn validate_color(color: &str) -> DomainResult<()> {
    if !is_valid_hex_color(color) {
        return Err(DomainError::Validation(format!(
            "invalid color hex: {color}"
        )));
    }
    Ok(())
}

fn validate_opacity(opacity: f32) -> DomainResult<()> {
    if !opacity.is_finite() || !(0.1..=1.0).contains(&opacity) {
        return Err(DomainError::Validation(format!(
            "opacity must be finite between 0.1 and 1.0, got {opacity}"
        )));
    }
    Ok(())
}

fn validate_position(pos: (f64, f64)) -> DomainResult<()> {
    if !pos.0.is_finite() || !pos.1.is_finite() {
        return Err(DomainError::Validation("position must be finite".into()));
    }
    Ok(())
}

fn validate_size(size: (f64, f64)) -> DomainResult<()> {
    if !size.0.is_finite() || !size.1.is_finite() {
        return Err(DomainError::Validation("size must be finite".into()));
    }
    if !(80.0..=4000.0).contains(&size.0) || !(80.0..=4000.0).contains(&size.1) {
        return Err(DomainError::Validation(format!(
            "size must be between 80 and 4000, got {:?}",
            size
        )));
    }
    Ok(())
}

fn validate_tags(tags: &[String]) -> DomainResult<()> {
    if tags.len() > 20 {
        return Err(DomainError::Validation("too many tags (max 20)".into()));
    }
    for t in tags {
        let trimmed = t.trim();
        if trimmed.is_empty() {
            return Err(DomainError::Validation("tag must not be empty".into()));
        }
        if trimmed.chars().count() > 30 {
            return Err(DomainError::Validation(format!("tag too long: {t}")));
        }
    }
    Ok(())
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .collect()
}

fn is_valid_hex_color(s: &str) -> bool {
    let s = s.trim();
    if !s.starts_with('#') {
        return false;
    }
    let hex = &s[1..];
    if hex.len() != 6 && hex.len() != 3 {
        return false;
    }
    hex.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_note_defaults() {
        let n = Note::new("Hello", "world").unwrap();
        assert_eq!(n.title, "Hello");
        assert_eq!(n.opacity, 1.0);
        assert_eq!(n.color, "#FFEB3B");
        assert!(!n.archived);
        assert!(n.is_active());
    }

    #[test]
    fn title_validation() {
        assert!(Note::new("", "x").is_err());
        assert!(Note::new("   ", "x").is_err());
        let long = "a".repeat(201);
        assert!(Note::new(long, "x").is_err());
    }

    #[test]
    fn opacity_validation() {
        let mut n = Note::new("t", "c").unwrap();
        assert!(n.set_opacity(0.05).is_err());
        assert!(n.set_opacity(f32::NAN).is_err());
        assert!(n.set_opacity(0.5).is_ok());
        assert_eq!(n.opacity, 0.5);
    }

    #[test]
    fn color_validation() {
        let mut n = Note::new("t", "c").unwrap();
        assert!(n.set_color("#GGGGGG").is_err());
        assert!(n.set_color("red").is_err());
        assert!(n.set_color("#fff").is_ok());
        assert!(n.set_color("#FF00AA").is_ok());
    }

    #[test]
    fn size_validation() {
        let mut n = Note::new("t", "c").unwrap();
        assert!(n.set_size(10.0, 10.0).is_err());
        assert!(n.set_size(300.0, 250.0).is_ok());
    }

    #[test]
    fn tags_validation_and_normalization() {
        let mut n = Note::new("t", "c").unwrap();
        n.set_tags(vec![" Rust ".into(), "Tauri".into()]).unwrap();
        assert_eq!(n.tags, vec!["rust", "tauri"]);
        let many: Vec<String> = (0..21).map(|i| format!("t{i}")).collect();
        assert!(n.set_tags(many).is_err());
    }

    #[test]
    fn archive_flow() {
        let mut n = Note::new("t", "c").unwrap();
        assert!(n.is_active());
        n.archive();
        assert!(!n.is_active());
        n.unarchive();
        assert!(n.is_active());
    }

    // -----------------------------------------------------------------------
    // Task tests
    // -----------------------------------------------------------------------

    #[test]
    fn task_new_defaults() {
        let t = Task::new("Buy milk").unwrap();
        assert_eq!(t.title, "Buy milk");
        assert!(!t.completed);
        assert!(t.deadline.is_none());
        assert!(t.reminder_thresholds.is_empty());
        assert_eq!(t.description, "");
    }

    #[test]
    fn task_title_validation() {
        assert!(Task::new("").is_err());
        assert!(Task::new("   ").is_err());
        let long = "a".repeat(201);
        assert!(Task::new(long).is_err());
    }

    #[test]
    fn task_description_validation() {
        let mut t = Task::new("task").unwrap();
        let long_desc = "x".repeat(20001);
        assert!(t.set_description(long_desc).is_err());
        assert!(t.set_description("ok").is_ok());
        assert_eq!(t.description, "ok");
    }

    #[test]
    fn task_set_title_touches_updated() {
        let mut t = Task::new("old").unwrap();
        let before = t.updated_at;
        // small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(15));
        t.set_title("new").unwrap();
        assert!(t.updated_at >= before);
        assert_eq!(t.title, "new");
    }

    #[test]
    fn task_completion_flow() {
        let mut t = Task::new("do it").unwrap();
        assert!(!t.completed);
        t.set_completed(true);
        assert!(t.completed);
        t.set_completed(false);
        assert!(!t.completed);
    }

    #[test]
    fn task_overdue() {
        let mut t = Task::new("urgent").unwrap();
        // No deadline → not overdue
        assert!(!t.is_overdue());

        // Past deadline
        let past = Utc::now() - chrono::Duration::try_minutes(10).unwrap();
        t.set_deadline(Some(past));
        assert!(t.is_overdue());

        // Completed tasks are never overdue
        t.set_completed(true);
        assert!(!t.is_overdue());
    }

    #[test]
    fn task_due_soon() {
        let mut t = Task::new("soon").unwrap();
        assert!(!t.is_due_soon(15)); // no deadline

        // Deadline in 10 minutes → due_soon(15) should be true
        let in_10 = Utc::now() + chrono::Duration::try_minutes(10).unwrap();
        t.set_deadline(Some(in_10));
        assert!(t.is_due_soon(15));
        assert!(!t.is_due_soon(5)); // 5 min threshold, deadline is 10 min away

        // Completed → not due soon
        t.set_completed(true);
        assert!(!t.is_due_soon(15));
    }

    #[test]
    fn task_next_reminder_fire_at() {
        let mut t = Task::new("reminder").unwrap();
        assert!(t.next_reminder_fire_at().is_none()); // no deadline

        // Deadline in 2 hours, thresholds 5m and 1h
        let in_2h = Utc::now() + chrono::Duration::try_hours(2).unwrap();
        t.set_deadline(Some(in_2h));
        t.set_reminder_thresholds(vec![
            ReminderThreshold::Minutes(5),
            ReminderThreshold::Hours(1),
        ])
        .unwrap();

        let next = t.next_reminder_fire_at().unwrap();
        // next should be deadline - 1h (1h from now), not 5m (1h55m from now)
        let expected = in_2h - chrono::Duration::try_hours(1).unwrap();
        assert!((next - expected).num_seconds().abs() < 2);
    }

    #[test]
    fn task_reminder_thresholds_validation() {
        let mut t = Task::new("task").unwrap();
        // Too many thresholds
        let many: Vec<ReminderThreshold> = (0..21)
            .map(|i| ReminderThreshold::Custom {
                minutes_before: i + 1,
            })
            .collect();
        assert!(t.set_reminder_thresholds(many).is_err());

        // Valid thresholds
        let valid = vec![
            ReminderThreshold::Minutes(5),
            ReminderThreshold::Minutes(15),
            ReminderThreshold::Hours(1),
        ];
        assert!(t.set_reminder_thresholds(valid).is_ok());

        // Duplicate thresholds
        let dupes = vec![ReminderThreshold::Minutes(5), ReminderThreshold::Minutes(5)];
        assert!(t.set_reminder_thresholds(dupes).is_err());

        // Out of range (0 minutes)
        let invalid = vec![ReminderThreshold::Custom { minutes_before: 0 }];
        assert!(t.set_reminder_thresholds(invalid).is_err());
    }

    #[test]
    fn reminder_threshold_as_minutes() {
        assert_eq!(ReminderThreshold::Minutes(5).as_minutes(), 5);
        assert_eq!(ReminderThreshold::Hours(2).as_minutes(), 120);
        assert_eq!(
            ReminderThreshold::Custom { minutes_before: 45 }.as_minutes(),
            45
        );
    }

    #[test]
    fn task_reconstitute() {
        let t = Task::reconstitute(
            uuid::Uuid::new_v4(),
            "title".into(),
            "desc".into(),
            Priority::High,
            None,
            vec![],
            false,
            Utc::now(),
            Utc::now(),
        )
        .unwrap();
        assert_eq!(t.title, "title");
        assert_eq!(t.priority, Priority::High);
    }
}

/// Task local — também independente de Mastersys (seção 5 do CLAUDE.md).
/// A ligação com um ticket Mastersys, quando existir, será um campo opcional
/// de metadado de integração, adicionado apenas na Fase 5 (ADR-006).
///
/// Seção 6 do CLAUDE.md: title, description, priority, deadline, completed,
/// reminder_thresholds (5m/10m/15m/30m/1h/2h/custom). Seção 8: notificações
/// com thresholds configuráveis.
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
/// Cada variante representa tempo antes do deadline para disparar lembrete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReminderThreshold {
    /// Lembra N minutos antes do deadline.
    Minutes(u32),
    /// Lembra N horas antes do deadline.
    Hours(u32),
    /// Lembra N minutos antes (customização do usuário).
    Custom { minutes_before: u32 },
}

impl ReminderThreshold {
    /// Converte o threshold para minutos totais.
    pub fn as_minutes(&self) -> u32 {
        match self {
            ReminderThreshold::Minutes(m) => *m,
            ReminderThreshold::Hours(h) => h * 60,
            ReminderThreshold::Custom { minutes_before } => *minutes_before,
        }
    }

    /// Valida que o threshold é razoável (1 minuto a 7 dias).
    pub fn is_valid(&self) -> bool {
        let m = self.as_minutes();
        (1..=10080).contains(&m) // 7 dias = 10080 minutos
    }

    /// Presets padrão: 5m, 10m, 15m, 30m, 1h, 2h.
    pub fn default_presets() -> Vec<ReminderThreshold> {
        vec![
            ReminderThreshold::Minutes(5),
            ReminderThreshold::Minutes(10),
            ReminderThreshold::Minutes(15),
            ReminderThreshold::Minutes(30),
            ReminderThreshold::Hours(1),
            ReminderThreshold::Hours(2),
        ]
    }
}

impl Task {
    /// Cria uma nova task com validações de domínio.
    pub fn new(title: impl Into<String>) -> DomainResult<Self> {
        let title = title.into();
        validate_task_title(&title)?;
        let now = Utc::now();
        Ok(Self {
            id: Uuid::new_v4(),
            title: title.trim().to_string(),
            description: String::new(),
            priority: Priority::default(),
            deadline: None,
            reminder_thresholds: Vec::new(),
            completed: false,
            created_at: now,
            updated_at: now,
        })
    }

    /// Reconstrói uma task a partir de dados persistidos.
    /// Usado por adapters de infraestrutura.
    #[allow(clippy::too_many_arguments)]
    pub fn reconstitute(
        id: TaskId,
        title: String,
        description: String,
        priority: Priority,
        deadline: Option<DateTime<Utc>>,
        reminder_thresholds: Vec<ReminderThreshold>,
        completed: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        validate_task_title(&title)?;
        validate_task_description(&description)?;
        validate_reminder_thresholds(&reminder_thresholds)?;
        Ok(Self {
            id,
            title: title.trim().to_string(),
            description,
            priority,
            deadline,
            reminder_thresholds,
            completed,
            created_at,
            updated_at,
        })
    }

    pub fn set_title(&mut self, title: impl Into<String>) -> DomainResult<()> {
        let t = title.into();
        validate_task_title(&t)?;
        self.title = t.trim().to_string();
        self.touch();
        Ok(())
    }

    pub fn set_description(&mut self, description: impl Into<String>) -> DomainResult<()> {
        let d = description.into();
        validate_task_description(&d)?;
        self.description = d;
        self.touch();
        Ok(())
    }

    pub fn set_deadline(&mut self, deadline: Option<DateTime<Utc>>) {
        self.deadline = deadline;
        self.touch();
    }

    pub fn set_priority(&mut self, priority: Priority) {
        self.priority = priority;
        self.touch();
    }

    pub fn set_completed(&mut self, completed: bool) {
        self.completed = completed;
        self.touch();
    }

    pub fn set_reminder_thresholds(
        &mut self,
        thresholds: Vec<ReminderThreshold>,
    ) -> DomainResult<()> {
        validate_reminder_thresholds(&thresholds)?;
        self.reminder_thresholds = thresholds;
        self.touch();
        Ok(())
    }

    /// True se a task tem deadline no passado e não está completa.
    pub fn is_overdue(&self) -> bool {
        if self.completed {
            return false;
        }
        match self.deadline {
            Some(dl) => Utc::now() >= dl,
            None => false,
        }
    }

    /// True se a task tem deadline dentro de `threshold` minutos a partir de agora.
    pub fn is_due_soon(&self, threshold_minutes: u32) -> bool {
        if self.completed {
            return false;
        }
        match self.deadline {
            Some(dl) => {
                let now = Utc::now();
                let threshold_duration = chrono::Duration::try_minutes(threshold_minutes as i64)
                    .unwrap_or(chrono::Duration::try_minutes(0).unwrap());
                now < dl && dl - now <= threshold_duration
            }
            None => false,
        }
    }

    /// Retorna o próximo horário de disparo de lembrete baseado nos thresholds.
    /// Para cada threshold, calcula `deadline - threshold` e retorna a primeira
    /// vez que ainda está no futuro. None se não há deadline ou todos já passaram.
    pub fn next_reminder_fire_at(&self) -> Option<DateTime<Utc>> {
        let dl = self.deadline?;
        let now = Utc::now();
        let mut candidates: Vec<DateTime<Utc>> = self
            .reminder_thresholds
            .iter()
            .filter_map(|t| {
                let minutes = t.as_minutes();
                let duration = chrono::Duration::try_minutes(minutes as i64)?;
                let fire_at = dl - duration;
                (fire_at > now).then_some(fire_at)
            })
            .collect();
        candidates.sort();
        candidates.into_iter().next()
    }

    /// Lista todos os horários de disparo (passados e futuros).
    pub fn all_reminder_fire_ats(&self) -> Vec<DateTime<Utc>> {
        let dl = match self.deadline {
            Some(d) => d,
            None => return Vec::new(),
        };
        let mut times: Vec<DateTime<Utc>> = self
            .reminder_thresholds
            .iter()
            .filter_map(|t| {
                let minutes = t.as_minutes();
                let duration = chrono::Duration::try_minutes(minutes as i64)?;
                Some(dl - duration)
            })
            .collect();
        times.sort();
        times
    }

    fn touch(&mut self) {
        self.updated_at = Utc::now();
    }
}

// ---------------------------------------------------------------------------
// User (Fase 4 — autenticação local)
// ---------------------------------------------------------------------------

/// Usuário local (sem Mastersys). O `password_hash` é um hash Argon2 opaco
/// produzido pela infraestrutura — nunca plaintext. A inspeção/comparação do
/// hash é responsabilidade da infraestrutura, não do domínio; o domínio apenas
/// valida o formato do username e transporta o hash como string opaca.
///
/// Nunca serializar `password_hash` para a UI (seção 11/18 do CLAUDE.md).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
}

impl User {
    /// Cria um usuário local com validação de domínio do username e do tamanho
    /// mínimo de senha. O `password_hash` deve ser produzido pela camada de
    /// infraestrutura via Argon2 e passado aqui como string opaca.
    pub fn new(
        username: impl Into<String>,
        password_hash: impl Into<String>,
    ) -> DomainResult<Self> {
        let username = username.into();
        let password_hash = password_hash.into();
        validate_username(&username)?;
        Ok(Self {
            id: Uuid::new_v4(),
            username: username.trim().to_string(),
            password_hash,
            created_at: Utc::now(),
        })
    }

    /// Reconstrói um usuário a partir de dados persistidos (sem revalidar id/created_at).
    /// Usado por adapters de infraestrutura.
    pub fn reconstitute(
        id: UserId,
        username: String,
        password_hash: String,
        created_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        validate_username(&username)?;
        Ok(Self {
            id,
            username: username.trim().to_string(),
            password_hash,
            created_at,
        })
    }
}

/// Valida username: 3-32 caracteres alfanuméricos (a-z, A-Z, 0-9, e _).
pub fn validate_username(username: &str) -> DomainResult<()> {
    let trimmed = username.trim();
    if trimmed.len() < 3 {
        return Err(DomainError::Validation(
            "username must be at least 3 characters".into(),
        ));
    }
    if trimmed.len() > 32 {
        return Err(DomainError::Validation(
            "username must be at most 32 characters".into(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return Err(DomainError::Validation(
            "username may only contain letters, digits and underscore".into(),
        ));
    }
    Ok(())
}

/// Valida o tamanho mínimo da senha antes do hashing (a verificação de força
/// real ocorre na infraestrutura durante o Argon2; aqui garantimos um mínimo).
pub fn validate_password(password: &str) -> DomainResult<()> {
    if password.len() < 8 {
        return Err(DomainError::Validation(
            "password must be at least 8 characters".into(),
        ));
    }
    if password.len() > 1024 {
        return Err(DomainError::Validation(
            "password must be at most 1024 characters".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod user_tests {
    use super::*;

    #[test]
    fn user_new_valid() {
        let u = User::new("alice", "hashedvalue").unwrap();
        assert_eq!(u.username, "alice");
        assert_eq!(u.password_hash, "hashedvalue");
        assert!(!u.username.is_empty());
    }

    #[test]
    fn username_validation() {
        assert!(User::new("ab", "x").is_err()); // too short
        assert!(User::new("", "x").is_err()); // empty
        assert!(User::new("alice!", "x").is_err()); // invalid char
        let long = "a".repeat(33);
        assert!(User::new(long, "x").is_err()); // too long
        assert!(User::new("alice_1", "x").is_ok()); // underscore allowed
        assert!(User::new("Álvaro", "x").is_err()); // non-ascii not allowed
    }

    #[test]
    fn password_validation() {
        assert!(validate_password("short").is_err()); // < 8
        assert!(validate_password("password123").is_ok());
        let long = "x".repeat(1025);
        assert!(validate_password(&long).is_err());
    }

    #[test]
    fn user_reconstitute() {
        let u =
            User::reconstitute(UserId::new_v4(), "bob".into(), "hash".into(), Utc::now()).unwrap();
        assert_eq!(u.username, "bob");
    }
}

// ---------------------------------------------------------------------------
// Validações de Task (pur, sem I/O)
// ---------------------------------------------------------------------------

fn validate_task_title(title: &str) -> DomainResult<()> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(DomainError::Validation(
            "task title must not be empty".into(),
        ));
    }
    if trimmed.chars().count() > 200 {
        return Err(DomainError::Validation(
            "task title must be <= 200 chars".into(),
        ));
    }
    Ok(())
}

fn validate_task_description(description: &str) -> DomainResult<()> {
    if description.chars().count() > 20000 {
        return Err(DomainError::Validation(
            "task description must be <= 20000 chars".into(),
        ));
    }
    Ok(())
}

fn validate_reminder_thresholds(thresholds: &[ReminderThreshold]) -> DomainResult<()> {
    if thresholds.len() > 20 {
        return Err(DomainError::Validation(
            "too many reminder thresholds (max 20)".into(),
        ));
    }
    for t in thresholds {
        if !t.is_valid() {
            return Err(DomainError::Validation(format!(
                "reminder threshold must be between 1 and 10080 minutes, got {}",
                t.as_minutes()
            )));
        }
    }
    // Check for duplicates
    let mut seen = std::collections::HashSet::new();
    for t in thresholds {
        let mins = t.as_minutes();
        if !seen.insert(mins) {
            return Err(DomainError::Validation(format!(
                "duplicate reminder threshold: {mins} minutes"
            )));
        }
    }
    Ok(())
}
