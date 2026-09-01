//! `NotificationService` — lógica própria de agendamento de lembretes de tasks.
//!
//! ADR-004: usa o plugin oficial `tauri-plugin-notification` para *exibir*
//! notificações, mas o **agendamento, repetição e snooze são lógica própria**
//! deste serviço (não dependem de push/FCM). Nesta Fase 3, os agendamentos são
//! guardados em memória e logados — o motor de disparo real (background worker
//! acionando um notification via tauri) será conectado quando o app estiver
//! rodando; o contrato/arquitetura já fica pronto e testável.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use masterdesk_domain::{DomainError, DomainResult, TaskId};
use std::collections::HashMap;
use std::sync::Mutex;

/// Um agendamento de lembrete em memória.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledReminder {
    pub task_id: TaskId,
    pub fire_at: DateTime<Utc>,
}

/// Implementação concreta de `NotificationService` com agendamento em memória.
/// Nenhuma dependência de push/FCM — apenas validação e armazenamento.
pub struct NotificationService {
    /// task_id -> agendamento atual (um por task nesta fase).
    schedules: Mutex<HashMap<TaskId, ScheduledReminder>>,
    /// Log simples de disparos/eventos para depuração e teste.
    log: Mutex<Vec<String>>,
}

impl Default for NotificationService {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationService {
    pub fn new() -> Self {
        Self {
            schedules: Mutex::new(HashMap::new()),
            log: Mutex::new(Vec::new()),
        }
    }

    /// Lista os agendamentos atuais (para inspeção/testes).
    pub fn scheduled(&self) -> Vec<ScheduledReminder> {
        self.schedules.lock().unwrap().values().cloned().collect()
    }

    /// Log de eventos (para depuração e testes).
    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    /// Dispara (logado) um lembrete cujo fire_at já chegou.
    /// Chamado por um eventual background worker.
    pub fn fire_due(&self, now: DateTime<Utc>) {
        let due: Vec<_> = self
            .schedules
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.fire_at <= now)
            .cloned()
            .collect();
        for r in due {
            self.log
                .lock()
                .unwrap()
                .push(format!("fire task {} at {}", r.task_id, r.fire_at));
        }
    }
}

#[async_trait]
impl masterdesk_domain::ports::NotificationService for NotificationService {
    async fn schedule_reminder(&self, task_id: TaskId, fire_at: DateTime<Utc>) -> DomainResult<()> {
        let now = Utc::now();
        if fire_at <= now {
            return Err(DomainError::Validation(
                "reminder fire_at must be in the future".into(),
            ));
        }
        let mut schedules = self.schedules.lock().unwrap();
        // Uma task tem um único "next" reminder agendado por vez.
        schedules.insert(task_id, ScheduledReminder { task_id, fire_at });
        self.log
            .lock()
            .unwrap()
            .push(format!("scheduled task {} at {}", task_id, fire_at));
        Ok(())
    }

    async fn cancel_reminder(&self, task_id: TaskId) -> DomainResult<()> {
        self.schedules.lock().unwrap().remove(&task_id);
        self.log
            .lock()
            .unwrap()
            .push(format!("cancelled task {task_id}"));
        Ok(())
    }

    async fn snooze(&self, task_id: TaskId, minutes: u32) -> DomainResult<()> {
        if minutes == 0 {
            return Err(DomainError::Validation("snooze minutes must be > 0".into()));
        }
        let now = Utc::now();
        let new_fire_at = now + chrono::Duration::try_minutes(minutes as i64).unwrap();
        let mut schedules = self.schedules.lock().unwrap();
        schedules.insert(
            task_id,
            ScheduledReminder {
                task_id,
                fire_at: new_fire_at,
            },
        );
        self.log
            .lock()
            .unwrap()
            .push(format!("snoozed task {task_id} by {minutes} min"));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use masterdesk_domain::ports::NotificationService as _;

    #[tokio::test]
    async fn schedule_validates_future() {
        let ns = NotificationService::new();
        let past = Utc::now() - chrono::Duration::try_minutes(5).unwrap();
        let res = ns.schedule_reminder(uuid::Uuid::new_v4(), past).await;
        assert!(matches!(res, Err(DomainError::Validation(_))));
    }

    #[tokio::test]
    async fn schedule_cancel_flow() {
        let ns = NotificationService::new();
        let task_id = uuid::Uuid::new_v4();
        let future = Utc::now() + chrono::Duration::try_minutes(10).unwrap();
        ns.schedule_reminder(task_id, future).await.unwrap();
        assert_eq!(ns.scheduled().len(), 1);

        ns.cancel_reminder(task_id).await.unwrap();
        assert_eq!(ns.scheduled().len(), 0);
    }

    #[tokio::test]
    async fn snooze_reschedules() {
        let ns = NotificationService::new();
        let task_id = uuid::Uuid::new_v4();
        let future = Utc::now() + chrono::Duration::try_minutes(10).unwrap();
        ns.schedule_reminder(task_id, future).await.unwrap();

        ns.snooze(task_id, 15).await.unwrap();
        let sched = ns.scheduled();
        assert_eq!(sched.len(), 1);
        // fire_at should be ~15 min from now (within a small window)
        let expected = Utc::now() + chrono::Duration::try_minutes(15).unwrap();
        assert!((sched[0].fire_at - expected).num_seconds().abs() < 5);
    }

    #[tokio::test]
    async fn snooze_zero_invalid() {
        let ns = NotificationService::new();
        let res = ns.snooze(uuid::Uuid::new_v4(), 0).await;
        assert!(matches!(res, Err(DomainError::Validation(_))));
    }

    #[test]
    fn fire_due_logs() {
        let ns = NotificationService::new();
        let task_id = uuid::Uuid::new_v4();
        let future = Utc::now() + chrono::Duration::try_seconds(2).unwrap();
        // We cannot await here; call schedule synchronously via block_on
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { ns.schedule_reminder(task_id, future).await.unwrap() });
        // Advance time past fire_at
        let later = Utc::now() + chrono::Duration::try_minutes(1).unwrap();
        ns.fire_due(later);
        assert!(ns.log().iter().any(|l| l.starts_with("fire task")));
    }
}
