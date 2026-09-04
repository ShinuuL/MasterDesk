//! Entidades do domínio. Nenhum tipo aqui conhece SQLite, Tauri ou HTTP.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::errors::{DomainError, DomainResult};
use crate::external::ExternalRef;

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
    use crate::external::{ExternalKind, ExternalSystem};

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

    // -----------------------------------------------------------------------
    // Vínculo externo (ADR-006)
    // -----------------------------------------------------------------------

    fn mastersys_item(id: &str, title: &str) -> crate::external::ExternalWorkItem {
        use crate::external::{ExternalKind, ExternalRef, ExternalSystem, ExternalWorkItem};
        let r = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Ticket, id).unwrap();
        ExternalWorkItem::new(r, title).unwrap()
    }

    #[test]
    fn task_is_local_by_default() {
        let t = Task::new("local").unwrap();
        assert!(!t.is_external());
        assert!(t.external.is_none());
    }

    #[test]
    fn attach_external_marks_origin_without_touching_updated_at() {
        use crate::external::{ExternalKind, ExternalRef, ExternalSystem};
        let t = Task::new("do banco").unwrap();
        let before = t.updated_at;
        let r = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Task, "77").unwrap();
        let t = t.attach_external(Some(r));
        assert!(t.is_external());
        assert_eq!(t.external.as_ref().unwrap().external_id, "77");
        assert_eq!(t.updated_at, before, "reidratar não é edição do usuário");
    }

    #[test]
    fn ticket_link_validates_and_normalizes() {
        assert!(TicketLink::new("   ").is_err());
        assert!(TicketLink::new("9".repeat(65)).is_err());

        let link = TicketLink::new(" 991 ")
            .unwrap()
            .with_client(Some("  ".into()))
            .unwrap()
            .with_custom_status(Some("  aguardando peça  ".into()))
            .unwrap();
        assert_eq!(link.ticket, "991");
        assert_eq!(link.client, None, "campo em branco não é dado");
        assert_eq!(link.custom_status.as_deref(), Some("aguardando peça"));
    }

    #[test]
    fn ticket_link_rejects_oversized_custom_status_instead_of_truncating() {
        let long = "x".repeat(65);
        assert!(
            TicketLink::new("1")
                .unwrap()
                .with_custom_status(Some(long))
                .is_err(),
            "cortar em silêncio o que o usuário digitou é pior que recusar"
        );
    }

    #[test]
    fn manual_link_is_not_an_external_mirror() {
        let mut t = Task::new("acompanhar retorno").unwrap();
        t.set_ticket_link(Some(TicketLink::new("991").unwrap()));

        assert!(
            !t.is_external(),
            "vínculo manual não pode fazer a tarefa passar por espelho — senão o sync a apaga"
        );
        assert_eq!(t.related_ticket(), Some("991"));

        t.set_ticket_link(None);
        assert_eq!(t.related_ticket(), None);
    }

    #[test]
    fn related_ticket_prefers_the_mirror_over_the_manual_link() {
        use crate::external::{ExternalKind, ExternalRef, ExternalSystem};
        let mut t = Task::new("espelho").unwrap();
        t.set_ticket_link(Some(TicketLink::new("111").unwrap()));
        let r = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Ticket, "task-1")
            .unwrap()
            .with_ticket(Some("222".into()));
        let t = t.attach_external(Some(r));
        assert_eq!(t.related_ticket(), Some("222"));
    }

    #[test]
    fn apply_external_update_overwrites_owned_fields() {
        let mut t = Task::new("título antigo").unwrap();
        t.set_description("desc antiga").unwrap();
        t.set_priority(Priority::Low);

        let mut item = mastersys_item("991", "título novo");
        item.description = "desc nova".into();
        item.priority = Priority::Urgent;
        item.deadline = Some(Utc::now());
        item.completed = true;

        t.apply_external_update(&item).unwrap();

        assert_eq!(t.title, "título novo");
        assert_eq!(t.description, "desc nova");
        assert_eq!(t.priority, Priority::Urgent);
        assert!(t.deadline.is_some());
        assert!(t.completed);
        assert_eq!(t.external.as_ref().unwrap().external_id, "991");
    }

    #[test]
    fn apply_external_update_preserves_local_reminders() {
        let mut t = Task::new("t").unwrap();
        t.set_reminder_thresholds(vec![ReminderThreshold::Minutes(15)])
            .unwrap();
        t.apply_external_update(&mastersys_item("1", "vindo do mastersys"))
            .unwrap();
        assert_eq!(
            t.reminder_thresholds,
            vec![ReminderThreshold::Minutes(15)],
            "lembretes são configuração local e não podem ser apagados por um sync"
        );
    }

    #[test]
    fn apply_external_update_rejects_invalid_payload() {
        let mut t = Task::new("original").unwrap();
        let mut item = mastersys_item("1", "ok");
        item.title = "   ".into(); // adapter com bug / API devolvendo lixo
        assert!(t.apply_external_update(&item).is_err());
        assert_eq!(t.title, "original", "payload inválido não altera o estado");
    }

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

    /// O caso do `melhoria.png`: um chamado em pós-atendimento tinha prazo
    /// vencido, era pintado como atrasado e ainda disparava lembrete. Item
    /// parado na origem não tem urgência, então não agenda nada.
    #[test]
    fn parked_task_schedules_no_reminder_even_with_a_deadline() {
        let mut t = Task::new("chamado em pós-atendimento").unwrap();
        let in_2h = Utc::now() + chrono::Duration::try_hours(2).unwrap();
        t.set_deadline(Some(in_2h));
        t.set_reminder_thresholds(vec![ReminderThreshold::Minutes(15)])
            .unwrap();

        // Com o mesmo prazo e threshold, um item ATIVO agenda.
        let active = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Ticket, "ticket-1")
            .unwrap()
            .with_status_parked(false);
        let t_active = t.clone().attach_external(Some(active));
        assert!(
            t_active.next_reminder_fire_at().is_some(),
            "item ativo com prazo futuro tem de agendar"
        );
        assert!(!t_active.is_parked());

        // Mudando SÓ a flag de parado, o lembrete desaparece.
        let parked = ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Ticket, "ticket-1")
            .unwrap()
            .with_status_parked(true);
        let t_parked = t.attach_external(Some(parked));
        assert!(t_parked.is_parked());
        assert!(
            t_parked.next_reminder_fire_at().is_none(),
            "pós-atendimento não pode gerar lembrete de atraso"
        );
    }

    #[test]
    fn local_task_is_never_parked() {
        // `is_parked` só existe para origem externa; tarefa local não pode
        // perder lembrete por causa dessa regra.
        let mut t = Task::new("tarefa local").unwrap();
        t.set_deadline(Some(Utc::now() + chrono::Duration::try_hours(1).unwrap()));
        t.set_reminder_thresholds(vec![ReminderThreshold::Minutes(5)])
            .unwrap();
        assert!(!t.is_parked());
        assert!(t.next_reminder_fire_at().is_some());
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
    /// Origem externa, quando a tarefa espelha um item de um sistema de
    /// suporte (ADR-006). `None` = tarefa puramente local — o caso padrão,
    /// que continua funcionando sem nenhuma integração configurada.
    ///
    /// `serde(default)` mantém compatibilidade com payloads antigos do
    /// frontend que não conhecem o campo.
    #[serde(default)]
    pub external: Option<ExternalRef>,
    /// Vínculo **manual** com um chamado, criado pelo usuário aqui dentro.
    ///
    /// Distinto de [`Task::external`] de propósito: `external` é espelho — a
    /// origem é dona dos campos e a sincronização os sobrescreve. Este é o
    /// contrário: tarefa 100% local, que apenas aponta para um chamado, e que
    /// nenhuma sincronização toca. É o que permite anotar trabalho ligado a um
    /// chamado sem escrever nada no Mastersys.
    #[serde(default)]
    pub link: Option<TicketLink>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Vínculo manual de uma tarefa local a um chamado de suporte.
///
/// ## Por que não reusar `ExternalRef`
///
/// `ExternalRef` significa "este item **veio** da origem e ela é dona dele".
/// Gravar um vínculo manual ali faria a tarefa entrar no índice único de
/// espelhos, aparecer em `list_by_external_system` e ser **apagada** pela
/// próxima sincronização, que retira todo espelho que não veio na fila. O
/// pedido é o oposto: um item que o usuário controla e que sobrevive a
/// qualquer sync.
///
/// ## Status personalizado
///
/// Texto livre, escolhido pelo usuário, e sem qualquer relação com o catálogo
/// de status do Mastersys. Não existe cadastro para validar contra — inventar
/// um vocabulário aqui seria criar significado que a origem não tem (Regra 1).
/// Vazio significa "sem status próprio", e o card não mostra selo nenhum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TicketLink {
    /// Número do chamado como o usuário o conhece. String porque é o número
    /// que ele digita ou escolhe na busca, não uma chave nossa.
    pub ticket: String,
    /// Cliente, quando o vínculo veio da busca no Mastersys e trouxe o nome.
    /// Só exibição.
    pub client: Option<String>,
    /// Status criado pelo usuário para esta tarefa. Texto livre.
    pub custom_status: Option<String>,
}

impl TicketLink {
    pub fn new(ticket: impl Into<String>) -> DomainResult<Self> {
        let ticket = ticket.into().trim().to_string();
        if ticket.is_empty() {
            return Err(DomainError::Validation(
                "número do chamado não pode ser vazio".into(),
            ));
        }
        if ticket.chars().count() > 64 {
            return Err(DomainError::Validation(
                "número do chamado deve ter até 64 caracteres".into(),
            ));
        }
        Ok(Self {
            ticket,
            client: None,
            custom_status: None,
        })
    }

    pub fn with_client(mut self, client: Option<String>) -> DomainResult<Self> {
        self.client = clean_link_field(client, 200, "cliente")?;
        Ok(self)
    }

    pub fn with_custom_status(mut self, status: Option<String>) -> DomainResult<Self> {
        self.custom_status = clean_link_field(status, 64, "status personalizado")?;
        Ok(self)
    }
}

/// Normaliza um campo opcional do vínculo: vazio vira `None`, e comprimento
/// acima do limite é **erro**, não truncamento.
///
/// Truncar seria o caminho de `ExternalRef`, e lá faz sentido: o dado vem da
/// API e cortar é melhor que derrubar o sync inteiro. Aqui o dado vem de quem
/// está digitando, e cortar em silêncio o status que a pessoa escreveu é pior
/// que dizer que passou do limite.
fn clean_link_field(
    value: Option<String>,
    max_chars: usize,
    field: &str,
) -> DomainResult<Option<String>> {
    let Some(trimmed) = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    else {
        return Ok(None);
    };
    if trimmed.chars().count() > max_chars {
        return Err(DomainError::Validation(format!(
            "{field} deve ter até {max_chars} caracteres"
        )));
    }
    Ok(Some(trimmed))
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
            external: None,
            link: None,
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
            external: None,
            link: None,
            created_at,
            updated_at,
        })
    }

    /// Anexa (ou remove) a referência externa. Aditivo de propósito: manter a
    /// assinatura de `reconstitute` evita quebrar os consumidores existentes
    /// em `application` e `infrastructure` (Collaborative Rule 7).
    ///
    /// Não chama `touch()` — reidratar do banco não é uma edição do usuário.
    pub fn attach_external(mut self, external: Option<ExternalRef>) -> Self {
        self.external = external;
        self
    }

    /// Anexa o vínculo manual ao reidratar do banco. Como `attach_external`,
    /// não chama `touch()`.
    pub fn attach_link(mut self, link: Option<TicketLink>) -> Self {
        self.link = link;
        self
    }

    /// Cria, altera ou remove (`None`) o vínculo manual com um chamado.
    pub fn set_ticket_link(&mut self, link: Option<TicketLink>) {
        self.link = link;
        self.touch();
    }

    /// True quando a tarefa espelha um item de um sistema de suporte.
    pub fn is_external(&self) -> bool {
        self.external.is_some()
    }

    /// Número do chamado relacionado, venha ele do espelho ou do vínculo
    /// manual. É o que a UI usa para mostrar `#chamado` sem se importar com a
    /// procedência.
    pub fn related_ticket(&self) -> Option<&str> {
        self.external
            .as_ref()
            .and_then(|e| e.ticket.as_deref())
            .or(self.link.as_ref().map(|l| l.ticket.as_str()))
    }

    /// Aplica no estado local os campos que a origem externa é dona.
    ///
    /// Título, descrição, prioridade, prazo e conclusão pertencem à origem —
    /// são sobrescritos a cada sincronização. Tudo o que o usuário criou no
    /// MasterNote (anotações da tarefa, lembretes configurados localmente)
    /// não é tocado aqui, senão um sync apagaria trabalho do usuário.
    /// O espelho já reflete exatamente este item da origem?
    ///
    /// Compara os campos que [`Task::apply_external_update`] escreve — e só
    /// esses: lembretes e anotações são locais e não entram na conta.
    ///
    /// ## Por que isto existe
    ///
    /// `apply_external_update` sempre chama `touch()`, então a reconciliação
    /// contava **toda** rodada de sincronização como "atualizou" para cada
    /// espelho. Com algumas centenas de chamados na fila, `total_changes()`
    /// nunca era zero e o `sync_scheduler` avisava a UI a cada ciclo — o que
    /// recarregava o quadro inteiro sem que nada tivesse mudado. Somando as
    /// salas globais do Mastersys (evento de qualquer usuário da empresa pede
    /// sincronização), isso virava uma gravação de centenas de linhas e um
    /// recarregamento de tela a cada 15 segundos.
    ///
    /// Comparar é barato; gravar, reagendar lembrete e recarregar a UI não são.
    pub fn matches_external(&self, item: &crate::external::ExternalWorkItem) -> bool {
        self.title == item.title.trim()
            && self.description == item.description
            && self.priority == item.priority
            && self.deadline == item.deadline
            && self.completed == item.completed
            && self.external.as_ref() == Some(&item.reference)
    }

    pub fn apply_external_update(
        &mut self,
        item: &crate::external::ExternalWorkItem,
    ) -> DomainResult<()> {
        validate_task_title(&item.title)?;
        validate_task_description(&item.description)?;
        self.title = item.title.trim().to_string();
        self.description = item.description.clone();
        self.priority = item.priority;
        self.deadline = item.deadline;
        self.completed = item.completed;
        self.external = Some(item.reference.clone());
        self.touch();
        Ok(())
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

    /// True quando o sistema de origem considera o item parado (em espera,
    /// concluído ou cancelado). Sempre `false` para tarefa local.
    ///
    /// Ver [`ExternalRef::status_parked`] para o porquê e para quem preenche.
    pub fn is_parked(&self) -> bool {
        self.external.as_ref().is_some_and(|e| e.status_parked)
    }

    /// Retorna o próximo horário de disparo de lembrete baseado nos thresholds.
    /// Para cada threshold, calcula `deadline - threshold` e retorna a primeira
    /// vez que ainda está no futuro. None se não há deadline ou todos já passaram.
    ///
    /// Item parado na origem também devolve `None`: avisar "faltam 15 minutos"
    /// para um chamado que está em pós-atendimento é ruído, não lembrete. A
    /// regra vive aqui, e não em quem agenda, para valer nos dois caminhos —
    /// a sincronização e o agendamento avulso de `TaskService`.
    pub fn next_reminder_fire_at(&self) -> Option<DateTime<Utc>> {
        if self.is_parked() {
            return None;
        }
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
/// Valida um nome de usuário local.
///
/// ## O que passou a ser aceito em 2026-09-03
///
/// A regra anterior era `is_ascii_alphanumeric() || '_'`, que recusava
/// **espaço, acento, hífen e ponto**. Na prática isso rejeitava o nome de
/// quase todo mundo — "Gabriel Ferreira", "João", "ana.paula", "maria-clara" —
/// e a mensagem de erro dizia apenas "may only contain letters, digits and
/// underscore", em inglês, no meio de um app em português.
///
/// Agora o critério é o que de fato importa para um nome que só identifica uma
/// conta **local**: ter conteúdo visível e não conter caractere de controle.
/// Letra acentuada é `char::is_alphabetic`, então entra naturalmente.
///
/// ## O que continua recusado, e por quê
///
/// - **Pontuação fora do conjunto de nomes.** Aceitos: letra (com acento),
///   dígito, espaço, `_`, `-`, `.` e apóstrofo — o suficiente para
///   "ana.paula", "maria-clara" e "D'Ávila". `alice!` continua recusado: não
///   foi pedido, e recusar mantém a política que o teste existente já
///   registrava.
/// - **Espaço no início ou fim**: aceito na entrada e removido, porque é erro
///   de digitação comum e invisível na tela — mas guardar " ana" e "ana " como
///   contas diferentes seria uma armadilha.
/// - **Espaços internos múltiplos ou não convencionais**: colapsados, para
///   "ana  paula" e "ana paula" não virarem duas contas.
///
/// O comprimento passou a ser medido em **caracteres**, não bytes: com `len()`
/// um nome de 3 letras acentuadas ocupava 6 bytes e passava, enquanto o limite
/// de 32 cortava um nome de 20 letras acentuadas.
pub fn validate_username(username: &str) -> DomainResult<()> {
    let normalized = normalize_username(username);
    let count = normalized.chars().count();
    if count < 3 {
        return Err(DomainError::Validation(
            "o nome de usuário precisa de ao menos 3 caracteres".into(),
        ));
    }
    if count > 32 {
        return Err(DomainError::Validation(
            "o nome de usuário pode ter no máximo 32 caracteres".into(),
        ));
    }
    if let Some(bad) = normalized.chars().find(|c| !is_username_char(*c)) {
        // Mostra QUAL caractere ofendeu. A mensagem antiga listava o que era
        // permitido e deixava o usuário procurando na própria digitação.
        return Err(DomainError::Validation(format!(
            "o caractere '{bad}' não pode ser usado no nome de usuário"
        )));
    }
    Ok(())
}

/// Caracteres aceitos num nome de usuário local.
///
/// Letra cobre acentuada via `is_alphabetic`. A pontuação é a que aparece em
/// nome de pessoa — nada além, para não virar campo livre.
fn is_username_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, ' ' | '_' | '-' | '.' | '\'')
}

/// Forma canônica de um nome de usuário: sem espaço nas pontas e com espaços
/// internos colapsados em um só.
///
/// Precisa ser aplicada tanto ao cadastrar quanto ao logar, senão quem se
/// cadastrou como "ana  paula" (dois espaços) não conseguiria entrar digitando
/// "ana paula".
pub fn normalize_username(username: &str) -> String {
    username.split_whitespace().collect::<Vec<_>>().join(" ")
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
        assert!(User::new("ab", "x").is_err()); // curto
        assert!(User::new("", "x").is_err()); // vazio
        assert!(User::new("alice!", "x").is_err()); // pontuação fora do conjunto
        let long = "a".repeat(33);
        assert!(User::new(long, "x").is_err()); // longo
        assert!(User::new("alice_1", "x").is_ok());
    }

    /// A regra antiga era `is_ascii_alphanumeric() || '_'`, que recusava o nome
    /// de quase todo mundo. Estes casos são os que o DEV relatou em 2026-09-03.
    #[test]
    fn username_accepts_real_people_names() {
        for name in [
            "Gabriel Ferreira", // espaço — o caso relatado
            "João",             // acento
            "Álvaro",           // acento inicial (antes recusado)
            "ana.paula",
            "maria-clara",
            "D'Ávila",
            "José Antônio da Silva",
        ] {
            assert!(
                User::new(name, "x").is_ok(),
                "{name} é nome de gente e tem de ser aceito"
            );
        }
    }

    #[test]
    fn username_is_normalized_on_the_way_in() {
        // Espaço nas pontas e espaço interno duplicado não podem virar contas
        // diferentes de "ana paula".
        assert_eq!(normalize_username("  ana  paula  "), "ana paula");
        assert_eq!(normalize_username("ana\tpaula"), "ana paula");
        assert_eq!(normalize_username("ana paula"), "ana paula");
        // Só espaços não é nome.
        assert!(User::new("   ", "x").is_err());
    }

    #[test]
    fn username_length_is_counted_in_characters_not_bytes() {
        // "ção" tem 3 caracteres e 5 bytes. Com `len()` em bytes, um nome de 3
        // letras acentuadas passava e um de 20 era cortado.
        assert!(User::new("ção", "x").is_ok());
        let twenty_accented = "á".repeat(20);
        assert!(User::new(twenty_accented, "x").is_ok());
        let thirty_three_accented = "á".repeat(33);
        assert!(User::new(thirty_three_accented, "x").is_err());
    }

    #[test]
    fn username_error_says_which_character_offended() {
        let err = User::new("alice!", "x").unwrap_err();
        assert!(
            err.to_string().contains('!'),
            "a mensagem tem de apontar o caractere, não listar os permitidos: {err}"
        );
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
