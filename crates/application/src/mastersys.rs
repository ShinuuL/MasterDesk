//! Sincronização com o sistema de suporte (ADR-006).
//!
//! O MasterNote **espelha** os itens atribuídos ao usuário: puxa e reconcilia,
//! nunca escreve de volta. O provider concreto entra por injeção
//! (`SupportSystemProvider`), então esta camada não sabe se a origem é o
//! Mastersys, um mock de teste ou uma integração futura.
//!
//! ## Quem é dono de qual campo
//!
//! | Campo                        | Dono     |
//! |------------------------------|----------|
//! | título, descrição, prioridade, prazo, concluída | origem externa |
//! | anotações da tarefa          | usuário  |
//! | thresholds de lembrete       | usuário  |
//!
//! A cada sincronização os campos da origem são reescritos e os do usuário
//! ficam intactos — ver `Task::apply_external_update`.

use std::collections::HashSet;
use std::sync::Arc;

use masterdesk_domain::{
    ports::{NotificationService, SupportSystemProvider, TaskNoteRepository, TaskRepository},
    DomainResult, ExternalSystem, ExternalWorkItem, ReminderThreshold, SupportIdentity, Task,
    TaskId,
};

/// Ajustes de uma execução de sincronização.
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Lembretes aplicados a itens **recém-importados**. Itens que já existem
    /// localmente não são tocados: o usuário pode ter ajustado os dele à mão.
    ///
    /// Vem de fora (da UI) em vez de ser um default embutido porque "avisar 30
    /// minutos antes" é preferência de quem atende, não regra de negócio.
    pub default_reminders: Vec<ReminderThreshold>,
}

/// O que a sincronização fez. Serve para a UI dar retorno concreto em vez de
/// um "sincronizado" opaco.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Itens que não existiam localmente e foram criados.
    pub imported: u32,
    /// Espelhos existentes que foram atualizados.
    pub updated: u32,
    /// Espelhos removidos porque saíram da fila do usuário na origem.
    pub removed: u32,
    /// Espelhos que saíram da origem mas foram **preservados** porque tinham
    /// anotações do usuário. Aparecem como concluídos, não desaparecem.
    pub kept_with_notes: u32,
    /// Espelhos cujo id de origem mudou e foram **reapontados** para o novo
    /// item do mesmo chamado, em vez de retirados e importados de novo.
    /// Ver [`MastersysSyncService::sync`].
    pub reanchored: u32,
}

impl SyncReport {
    pub fn total_changes(&self) -> u32 {
        self.imported + self.updated + self.removed + self.kept_with_notes + self.reanchored
    }
}

pub struct MastersysSyncService {
    provider: Arc<dyn SupportSystemProvider>,
    task_repo: Arc<dyn TaskRepository>,
    note_repo: Arc<dyn TaskNoteRepository>,
    notification_service: Option<Arc<dyn NotificationService>>,
}

impl MastersysSyncService {
    pub fn new(
        provider: Arc<dyn SupportSystemProvider>,
        task_repo: Arc<dyn TaskRepository>,
        note_repo: Arc<dyn TaskNoteRepository>,
        notification_service: Option<Arc<dyn NotificationService>>,
    ) -> Self {
        Self {
            provider,
            task_repo,
            note_repo,
            notification_service,
        }
    }

    pub async fn is_configured(&self) -> bool {
        self.provider.is_configured().await
    }

    pub async fn current_identity(&self) -> DomainResult<Option<SupportIdentity>> {
        self.provider.current_identity().await
    }

    pub async fn connect(&self, identifier: &str, password: &str) -> DomainResult<SupportIdentity> {
        self.provider.authenticate(identifier, password).await
    }

    /// Desconecta e apaga os espelhos locais que não têm anotações.
    ///
    /// Deixar os espelhos para trás seria pior: eles ficariam congelados no
    /// quadro sem nunca mais atualizar, indistinguíveis de tarefas locais.
    /// Os que têm anotações ficam, porque contêm trabalho do usuário.
    pub async fn disconnect(&self) -> DomainResult<SyncReport> {
        let mut report = SyncReport::default();
        let mirrors = self
            .task_repo
            .list_by_external_system(ExternalSystem::Mastersys)
            .await?;
        for task in mirrors {
            self.retire_mirror(task, &mut report).await?;
        }
        self.provider.sign_out().await?;
        Ok(report)
    }

    /// Puxa a fila do usuário e reconcilia com o estado local.
    ///
    /// ## Reancoragem: quando o id do item muda mas o chamado é o mesmo
    ///
    /// A chave local de um espelho é o id da origem (`task-123`, `ticket-991`).
    /// Existe um caminho no Mastersys que troca essa chave sem que nada tenha
    /// saído da fila da pessoa: ao mudar o status de um chamado para um status
    /// ativo, `TicketService` reatribui a tarefa vinculada a **quem mudou o
    /// status** (`taskAssigneeId = userId`), ignorando o analista responsável.
    /// A tarefa some de `GET /api/tasks/users/<eu>` e o chamado volta pelo ramo
    /// de chamados, agora como `ticket-991`.
    ///
    /// Sem tratamento, o espelho `task-123` era retirado (levando lembretes, e
    /// as anotações junto quando não havia nenhuma) e um item novo entrava no
    /// lugar. Foi o que o DEV viu em 2026-09-03: "o Note acompanhou a mudança
    /// de status mas não manteve atribuído a meu usuário".
    ///
    /// Aqui o espelho é **reapontado** para o novo id quando os dois falam do
    /// mesmo chamado, preservando id local, anotações e lembretes. É contorno
    /// declarado, não conserto: a causa está no Mastersys e está registrada em
    /// `docs/PENDENCIAS.md`. O contorno é seguro por si — só age quando o
    /// número do chamado é o mesmo, e chamado é a identidade que o usuário
    /// reconhece.
    pub async fn sync(&self, options: SyncOptions) -> DomainResult<SyncReport> {
        let items = self.provider.fetch_assigned_work().await?;
        let mut report = SyncReport::default();

        let mirrors = self
            .task_repo
            .list_by_external_system(ExternalSystem::Mastersys)
            .await?;

        // Chaves que a origem devolveu nesta rodada. Calculado antes de
        // reconciliar porque a reancoragem precisa saber se o espelho candidato
        // está órfão *nesta* fila, não na anterior.
        let seen: HashSet<String> = items
            .iter()
            .map(|item| item.reference.dedup_key())
            .collect();

        // Espelhos órfãos (a chave deles não veio) indexados por chamado — os
        // únicos candidatos a reancoragem.
        let mut orphans_by_ticket: Vec<(String, Task)> = mirrors
            .iter()
            .filter(|task| {
                task.external
                    .as_ref()
                    .is_some_and(|e| !seen.contains(&e.dedup_key()))
            })
            .filter_map(|task| {
                let ticket = task.external.as_ref()?.ticket.clone()?;
                Some((ticket, task.clone()))
            })
            .collect();

        // Ids locais que a reancoragem consumiu, para o passo de retirada não
        // apagar o que acabou de ser reaproveitado.
        let mut reanchored_ids: HashSet<TaskId> = HashSet::new();

        for item in &items {
            let reanchor = self
                .take_orphan_for(&mut orphans_by_ticket, item, &reanchored_ids)
                .await?;
            if let Some(mut task) = reanchor {
                task.apply_external_update(item)?;
                self.task_repo.save(&task).await?;
                self.reschedule(&task).await;
                reanchored_ids.insert(task.id);
                report.reanchored += 1;
                continue;
            }
            self.reconcile_item(item, &options, &mut report).await?;
        }

        // Itens que existiam localmente e não vieram mais na fila: foram
        // reatribuídos a outra pessoa ou saíram do escopo do usuário.
        for task in mirrors {
            if reanchored_ids.contains(&task.id) {
                continue;
            }
            let still_present = task
                .external
                .as_ref()
                .is_some_and(|e| seen.contains(&e.dedup_key()));
            if !still_present {
                self.retire_mirror(task, &mut report).await?;
            }
        }

        Ok(report)
    }

    /// Escolhe um espelho órfão do mesmo chamado para receber o item novo.
    ///
    /// Devolve `None` — e o item segue o caminho normal — quando:
    ///
    /// - o item não traz número de chamado (tarefa solta: nada para casar);
    /// - já existe espelho com a chave exata do item (não é troca de id);
    /// - nenhum órfão aponta para aquele chamado.
    ///
    /// Cada órfão é consumido no máximo uma vez: dois itens do mesmo chamado
    /// (a tarefa e o chamado, por exemplo) não podem reivindicar o mesmo
    /// espelho local.
    async fn take_orphan_for(
        &self,
        orphans: &mut Vec<(String, Task)>,
        item: &ExternalWorkItem,
        already_used: &HashSet<TaskId>,
    ) -> DomainResult<Option<Task>> {
        // Item cancelado/removido não é reancorado: ele vai ser retirado de
        // qualquer forma, e reapontar o espelho antes disso só embaralharia.
        if item.removed {
            return Ok(None);
        }
        let Some(ticket) = item.reference.ticket.as_deref() else {
            return Ok(None);
        };
        if self
            .task_repo
            .find_by_external(&item.reference)
            .await?
            .is_some()
        {
            return Ok(None);
        }
        let position = orphans
            .iter()
            .position(|(t, task)| t == ticket && !already_used.contains(&task.id));
        Ok(position.map(|i| orphans.remove(i).1))
    }

    async fn reconcile_item(
        &self,
        item: &ExternalWorkItem,
        options: &SyncOptions,
        report: &mut SyncReport,
    ) -> DomainResult<()> {
        let existing = self.task_repo.find_by_external(&item.reference).await?;

        // Cancelado na origem: trata como se tivesse saído da fila.
        if item.removed {
            if let Some(task) = existing {
                self.retire_mirror(task, report).await?;
            }
            return Ok(());
        }

        match existing {
            Some(mut task) => {
                // Espelho já idêntico ao item: nada a gravar, nada a reagendar
                // e — o que mais importa — nada a contar como mudança.
                //
                // `report.total_changes() > 0` é o que faz o `sync_scheduler`
                // avisar a UI. Contar aqui sem comparar fazia toda rodada
                // parecer uma rodada com mudança, e o quadro recarregava
                // inteiro a cada ciclo. Ver `Task::matches_external`.
                if task.matches_external(item) {
                    return Ok(());
                }
                task.apply_external_update(item)?;
                self.task_repo.save(&task).await?;
                self.reschedule(&task).await;
                report.updated += 1;
            }
            None => {
                // Item já concluído na origem que nunca foi importado não
                // entra: encheria o quadro de histórico no primeiro sync.
                if item.completed {
                    return Ok(());
                }
                let mut task = Task::new(&item.title)?;
                task.apply_external_update(item)?;
                if !options.default_reminders.is_empty() {
                    task.set_reminder_thresholds(options.default_reminders.clone())?;
                }
                self.task_repo.save(&task).await?;
                self.reschedule(&task).await;
                report.imported += 1;
            }
        }
        Ok(())
    }

    /// Remove o espelho — ou o preserva como concluído, se o usuário escreveu
    /// anotações nele. Anotação é trabalho manual e nunca é descartada por uma
    /// sincronização.
    async fn retire_mirror(&self, mut task: Task, report: &mut SyncReport) -> DomainResult<()> {
        if self.note_repo.count_by_task(task.id).await? > 0 {
            if !task.completed {
                task.set_completed(true);
                self.task_repo.save(&task).await?;
            }
            self.cancel_reminders(&task).await;
            report.kept_with_notes += 1;
            return Ok(());
        }

        self.cancel_reminders(&task).await;
        // Explícito além do ON DELETE CASCADE: o cascade depende de
        // `PRAGMA foreign_keys` estar ligado na conexão.
        self.note_repo.delete_by_task(task.id).await?;
        self.task_repo.delete(task.id).await?;
        report.removed += 1;
        Ok(())
    }

    async fn reschedule(&self, task: &Task) {
        let Some(ref ns) = self.notification_service else {
            return;
        };
        let _ = ns.cancel_reminder(task.id).await;
        if task.completed {
            return;
        }
        if let Some(fire_at) = task.next_reminder_fire_at() {
            // Falha de agendamento não pode abortar a sincronização inteira —
            // o item já está salvo e visível no quadro.
            let _ = ns.schedule_reminder(task.id, fire_at).await;
        }
    }

    async fn cancel_reminders(&self, task: &Task) {
        if let Some(ref ns) = self.notification_service {
            let _ = ns.cancel_reminder(task.id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::{Duration, Utc};
    use masterdesk_domain::{
        DomainError, ExternalKind, ExternalRef, Priority, TaskId, TaskNote, TaskNoteId,
    };
    use std::collections::HashMap;
    use std::sync::Mutex;

    // ---- Fakes -----------------------------------------------------------

    #[derive(Default)]
    struct FakeTasks {
        items: Mutex<HashMap<TaskId, Task>>,
    }

    #[async_trait]
    impl TaskRepository for FakeTasks {
        async fn save(&self, task: &Task) -> DomainResult<()> {
            self.items.lock().unwrap().insert(task.id, task.clone());
            Ok(())
        }
        async fn find_by_id(&self, id: TaskId) -> DomainResult<Option<Task>> {
            Ok(self.items.lock().unwrap().get(&id).cloned())
        }
        async fn list_pending(&self) -> DomainResult<Vec<Task>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .values()
                .filter(|t| !t.completed)
                .cloned()
                .collect())
        }
        async fn list_completed(&self) -> DomainResult<Vec<Task>> {
            Ok(Vec::new())
        }
        async fn list_all(&self) -> DomainResult<Vec<Task>> {
            Ok(self.items.lock().unwrap().values().cloned().collect())
        }
        async fn list_overdue(&self) -> DomainResult<Vec<Task>> {
            Ok(Vec::new())
        }
        async fn delete(&self, id: TaskId) -> DomainResult<()> {
            self.items.lock().unwrap().remove(&id);
            Ok(())
        }
        async fn find_by_external(&self, r: &ExternalRef) -> DomainResult<Option<Task>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .values()
                .find(|t| {
                    t.external
                        .as_ref()
                        .is_some_and(|e| e.dedup_key() == r.dedup_key())
                })
                .cloned())
        }
        async fn list_by_external_system(&self, s: ExternalSystem) -> DomainResult<Vec<Task>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .values()
                .filter(|t| t.external.as_ref().is_some_and(|e| e.system == s))
                .cloned()
                .collect())
        }
    }

    #[derive(Default)]
    struct FakeNotes {
        items: Mutex<Vec<TaskNote>>,
    }

    #[async_trait]
    impl TaskNoteRepository for FakeNotes {
        async fn save(&self, note: &TaskNote) -> DomainResult<()> {
            self.items.lock().unwrap().push(note.clone());
            Ok(())
        }
        async fn find_by_id(&self, id: TaskNoteId) -> DomainResult<Option<TaskNote>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .iter()
                .find(|n| n.id == id)
                .cloned())
        }
        async fn list_by_task(&self, task_id: TaskId) -> DomainResult<Vec<TaskNote>> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .iter()
                .filter(|n| n.task_id == task_id)
                .cloned()
                .collect())
        }
        async fn count_by_task(&self, task_id: TaskId) -> DomainResult<u32> {
            Ok(self
                .items
                .lock()
                .unwrap()
                .iter()
                .filter(|n| n.task_id == task_id)
                .count() as u32)
        }
        async fn delete(&self, id: TaskNoteId) -> DomainResult<()> {
            self.items.lock().unwrap().retain(|n| n.id != id);
            Ok(())
        }
        async fn counts_by_task(&self) -> DomainResult<Vec<(TaskId, u32)>> {
            let mut acc: HashMap<TaskId, u32> = HashMap::new();
            for n in self.items.lock().unwrap().iter() {
                *acc.entry(n.task_id).or_insert(0) += 1;
            }
            Ok(acc.into_iter().collect())
        }
        async fn delete_by_task(&self, task_id: TaskId) -> DomainResult<()> {
            self.items.lock().unwrap().retain(|n| n.task_id != task_id);
            Ok(())
        }
    }

    struct FakeProvider {
        items: Mutex<Vec<ExternalWorkItem>>,
        signed_out: Mutex<bool>,
    }

    impl FakeProvider {
        fn with(items: Vec<ExternalWorkItem>) -> Arc<Self> {
            Arc::new(Self {
                items: Mutex::new(items),
                signed_out: Mutex::new(false),
            })
        }
    }

    #[async_trait]
    impl SupportSystemProvider for FakeProvider {
        async fn is_configured(&self) -> bool {
            !*self.signed_out.lock().unwrap()
        }
        async fn authenticate(&self, _i: &str, _p: &str) -> DomainResult<SupportIdentity> {
            Err(DomainError::unauthorized("sessão inválida"))
        }
        async fn current_identity(&self) -> DomainResult<Option<SupportIdentity>> {
            Ok(None)
        }
        async fn sign_out(&self) -> DomainResult<()> {
            *self.signed_out.lock().unwrap() = true;
            Ok(())
        }
        async fn fetch_assigned_work(&self) -> DomainResult<Vec<ExternalWorkItem>> {
            Ok(self.items.lock().unwrap().clone())
        }
    }

    // ---- Helpers ---------------------------------------------------------

    fn item(id: &str, title: &str) -> ExternalWorkItem {
        let reference =
            ExternalRef::new(ExternalSystem::Mastersys, ExternalKind::Task, id).unwrap();
        ExternalWorkItem::new(reference, title).unwrap()
    }

    /// Item que carrega número de chamado — condição para reancoragem.
    fn item_of_ticket(id: &str, title: &str, ticket: &str) -> ExternalWorkItem {
        let kind = if id.starts_with("ticket-") {
            ExternalKind::Ticket
        } else {
            ExternalKind::Task
        };
        let reference = ExternalRef::new(ExternalSystem::Mastersys, kind, id)
            .unwrap()
            .with_ticket(Some(ticket.into()));
        ExternalWorkItem::new(reference, title).unwrap()
    }

    struct Fixture {
        service: MastersysSyncService,
        tasks: Arc<FakeTasks>,
        notes: Arc<FakeNotes>,
        provider: Arc<FakeProvider>,
    }

    fn fixture(items: Vec<ExternalWorkItem>) -> Fixture {
        let tasks = Arc::new(FakeTasks::default());
        let notes = Arc::new(FakeNotes::default());
        let provider = FakeProvider::with(items);
        let service =
            MastersysSyncService::new(provider.clone(), tasks.clone(), notes.clone(), None);
        Fixture {
            service,
            tasks,
            notes,
            provider,
        }
    }

    // ---- Tests -----------------------------------------------------------

    /// A sincronização em vazio é o caso mais comum de todos: as salas globais
    /// do Mastersys pedem sincronização a cada evento de qualquer usuário da
    /// empresa, e na esmagadora maioria delas nada da fila deste usuário mudou.
    /// Se essa rodada contar como mudança, a UI recarrega o quadro inteiro à
    /// toa — foi o que fazia a tela piscar de 15 em 15 segundos.
    #[tokio::test]
    async fn resync_with_identical_items_reports_no_change() {
        let f = fixture(vec![item("task-1", "Primeiro"), item("task-2", "Segundo")]);
        let first = f.service.sync(SyncOptions::default()).await.unwrap();
        assert_eq!(first.imported, 2);

        let second = f.service.sync(SyncOptions::default()).await.unwrap();
        assert_eq!(second.updated, 0, "nada mudou na origem");
        assert_eq!(
            second.total_changes(),
            0,
            "é `total_changes() > 0` que avisa a UI — em rodada sem mudança tem de ser zero"
        );
    }

    #[tokio::test]
    async fn resync_still_updates_when_the_item_really_changed() {
        let f = fixture(vec![item("task-1", "Título antigo")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        *f.provider.items.lock().unwrap() = vec![item("task-1", "Título novo")];
        let report = f.service.sync(SyncOptions::default()).await.unwrap();

        assert_eq!(report.updated, 1);
        assert_eq!(f.tasks.list_all().await.unwrap()[0].title, "Título novo");
    }

    #[tokio::test]
    async fn first_sync_imports_open_items() {
        let f = fixture(vec![item("task-1", "Primeiro"), item("task-2", "Segundo")]);
        let report = f.service.sync(SyncOptions::default()).await.unwrap();

        assert_eq!(report.imported, 2);
        assert_eq!(report.updated, 0);
        assert_eq!(f.tasks.list_all().await.unwrap().len(), 2);
        assert!(f
            .tasks
            .list_all()
            .await
            .unwrap()
            .iter()
            .all(|t| t.is_external()));
    }

    #[tokio::test]
    async fn second_sync_updates_instead_of_duplicating() {
        let f = fixture(vec![item("task-1", "Título antigo")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        let mut changed = item("task-1", "Título novo");
        changed.priority = Priority::Urgent;
        *f.provider.items.lock().unwrap() = vec![changed];

        let report = f.service.sync(SyncOptions::default()).await.unwrap();
        assert_eq!(report.imported, 0);
        assert_eq!(report.updated, 1);

        let all = f.tasks.list_all().await.unwrap();
        assert_eq!(all.len(), 1, "não pode duplicar o mesmo item externo");
        assert_eq!(all[0].title, "Título novo");
        assert_eq!(all[0].priority, Priority::Urgent);
    }

    #[tokio::test]
    async fn mirror_is_reanchored_when_the_source_id_changes_but_the_ticket_is_the_same() {
        // Cenário exato do bug: a tarefa do chamado 991 é reatribuída no
        // Mastersys por causa de uma mudança de status, sai de
        // `/api/tasks/users/<eu>`, e o chamado volta pelo ramo de chamados.
        let f = fixture(vec![item_of_ticket("task-123", "Corrigir NF-e", "991")]);
        f.service.sync(SyncOptions::default()).await.unwrap();
        let before = f.tasks.list_all().await.unwrap();
        assert_eq!(before.len(), 1);
        let local_id = before[0].id;

        *f.provider.items.lock().unwrap() =
            vec![item_of_ticket("ticket-991", "Corrigir NF-e", "991")];
        let report = f.service.sync(SyncOptions::default()).await.unwrap();

        assert_eq!(report.reanchored, 1);
        assert_eq!(report.removed, 0, "o espelho não pode ser retirado");
        assert_eq!(report.imported, 0, "nem reimportado como item novo");

        let after = f.tasks.list_all().await.unwrap();
        assert_eq!(after.len(), 1, "continua sendo um item só no quadro");
        assert_eq!(after[0].id, local_id, "o id local sobrevive à troca");
        assert_eq!(
            after[0].external.as_ref().unwrap().external_id,
            "ticket-991",
            "e passa a apontar para o id novo da origem"
        );
    }

    #[tokio::test]
    async fn reanchoring_preserves_the_users_notes() {
        let f = fixture(vec![item_of_ticket("task-123", "Corrigir NF-e", "991")]);
        f.service.sync(SyncOptions::default()).await.unwrap();
        let task_id = f.tasks.list_all().await.unwrap()[0].id;
        f.notes
            .save(&TaskNote::new(task_id, "cliente ligou 14h").unwrap())
            .await
            .unwrap();

        *f.provider.items.lock().unwrap() =
            vec![item_of_ticket("ticket-991", "Corrigir NF-e", "991")];
        f.service.sync(SyncOptions::default()).await.unwrap();

        let notes = f.notes.list_by_task(task_id).await.unwrap();
        assert_eq!(
            notes.len(),
            1,
            "anotação segue colada na mesma tarefa local"
        );
    }

    #[tokio::test]
    async fn a_different_ticket_is_not_reanchored() {
        let f = fixture(vec![item_of_ticket("task-123", "Chamado 991", "991")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        // Outro chamado: é item novo, e o antigo saiu mesmo da fila.
        *f.provider.items.lock().unwrap() =
            vec![item_of_ticket("ticket-555", "Chamado 555", "555")];
        let report = f.service.sync(SyncOptions::default()).await.unwrap();

        assert_eq!(report.reanchored, 0);
        assert_eq!(report.imported, 1);
        assert_eq!(report.removed, 1);
    }

    #[tokio::test]
    async fn an_item_still_in_the_queue_is_updated_not_reanchored() {
        let f = fixture(vec![item_of_ticket("task-123", "antigo", "991")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        // Mesma chave de origem: caminho normal de atualização.
        *f.provider.items.lock().unwrap() = vec![item_of_ticket("task-123", "novo", "991")];
        let report = f.service.sync(SyncOptions::default()).await.unwrap();

        assert_eq!(report.reanchored, 0);
        assert_eq!(report.updated, 1);
        assert_eq!(f.tasks.list_all().await.unwrap()[0].title, "novo");
    }

    #[tokio::test]
    async fn two_items_of_the_same_ticket_do_not_claim_the_same_mirror() {
        let f = fixture(vec![item_of_ticket("task-123", "Chamado 991", "991")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        // A tarefa mudou de id E o chamado veio solto: um reancora, o outro
        // entra como item novo — nunca os dois no mesmo espelho local.
        *f.provider.items.lock().unwrap() = vec![
            item_of_ticket("task-456", "Chamado 991", "991"),
            item_of_ticket("ticket-991", "Chamado 991", "991"),
        ];
        let report = f.service.sync(SyncOptions::default()).await.unwrap();

        assert_eq!(report.reanchored, 1);
        assert_eq!(report.imported, 1);
        assert_eq!(f.tasks.list_all().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_canceled_item_is_retired_instead_of_reanchored() {
        let f = fixture(vec![item_of_ticket("task-123", "Chamado 991", "991")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        let mut canceled = item_of_ticket("ticket-991", "Chamado 991", "991");
        canceled.removed = true;
        *f.provider.items.lock().unwrap() = vec![canceled];
        let report = f.service.sync(SyncOptions::default()).await.unwrap();

        assert_eq!(report.reanchored, 0);
        assert_eq!(report.removed, 1);
        assert!(f.tasks.list_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn already_completed_item_is_not_imported_on_first_sync() {
        let mut done = item("task-9", "Já resolvido");
        done.completed = true;
        let f = fixture(vec![done]);

        let report = f.service.sync(SyncOptions::default()).await.unwrap();
        assert_eq!(report.imported, 0);
        assert!(f.tasks.list_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn completion_in_origin_is_mirrored_for_known_items() {
        let f = fixture(vec![item("task-1", "Em aberto")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        let mut done = item("task-1", "Em aberto");
        done.completed = true;
        *f.provider.items.lock().unwrap() = vec![done];

        f.service.sync(SyncOptions::default()).await.unwrap();
        let all = f.tasks.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].completed);
    }

    #[tokio::test]
    async fn removed_item_deletes_the_mirror() {
        let f = fixture(vec![item("task-1", "Vai cancelar")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        let mut canceled = item("task-1", "Vai cancelar");
        canceled.removed = true;
        *f.provider.items.lock().unwrap() = vec![canceled];

        let report = f.service.sync(SyncOptions::default()).await.unwrap();
        assert_eq!(report.removed, 1);
        assert!(f.tasks.list_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn item_that_left_the_queue_is_removed() {
        let f = fixture(vec![item("task-1", "a"), item("task-2", "b")]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        // task-2 foi reatribuída a outra pessoa: some da fila.
        *f.provider.items.lock().unwrap() = vec![item("task-1", "a")];

        let report = f.service.sync(SyncOptions::default()).await.unwrap();
        assert_eq!(report.removed, 1);
        // task-1 veio igual: não conta como atualização. Este assert era `1`
        // quando toda rodada contava cada espelho como atualizado.
        assert_eq!(report.updated, 0);
        let all = f.tasks.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "a");
    }

    #[tokio::test]
    async fn mirror_with_user_notes_is_preserved_not_deleted() {
        let f = fixture(vec![item("task-1", "Com anotações")]);
        f.service.sync(SyncOptions::default()).await.unwrap();
        let task_id = f.tasks.list_all().await.unwrap()[0].id;
        f.notes
            .save(&TaskNote::new(task_id, "liguei para o cliente").unwrap())
            .await
            .unwrap();

        // Sai da fila.
        *f.provider.items.lock().unwrap() = vec![];
        let report = f.service.sync(SyncOptions::default()).await.unwrap();

        assert_eq!(report.removed, 0);
        assert_eq!(report.kept_with_notes, 1);
        let all = f.tasks.list_all().await.unwrap();
        assert_eq!(all.len(), 1, "trabalho do usuário não é descartado");
        assert!(all[0].completed);
        assert_eq!(f.notes.count_by_task(task_id).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn local_tasks_are_never_touched_by_sync() {
        let f = fixture(vec![item("task-1", "externa")]);
        let local = Task::new("minha tarefa local").unwrap();
        f.tasks.save(&local).await.unwrap();

        f.service.sync(SyncOptions::default()).await.unwrap();
        // E um segundo sync com a fila vazia, que é o caso que mais poderia
        // varrer tarefas locais por engano.
        *f.provider.items.lock().unwrap() = vec![];
        f.service.sync(SyncOptions::default()).await.unwrap();

        let survivors = f.tasks.list_all().await.unwrap();
        assert_eq!(survivors.len(), 1);
        assert_eq!(survivors[0].id, local.id);
        assert!(!survivors[0].is_external());
    }

    #[tokio::test]
    async fn default_reminders_apply_only_to_newly_imported_items() {
        let options = SyncOptions {
            default_reminders: vec![ReminderThreshold::Minutes(30)],
        };
        let mut with_deadline = item("task-1", "com prazo");
        with_deadline.deadline = Some(Utc::now() + Duration::try_hours(4).unwrap());
        let f = fixture(vec![with_deadline.clone()]);

        f.service.sync(options.clone()).await.unwrap();
        let task_id = f.tasks.list_all().await.unwrap()[0].id;
        assert_eq!(
            f.tasks
                .find_by_id(task_id)
                .await
                .unwrap()
                .unwrap()
                .reminder_thresholds,
            vec![ReminderThreshold::Minutes(30)]
        );

        // Usuário troca o lembrete à mão...
        let mut edited = f.tasks.find_by_id(task_id).await.unwrap().unwrap();
        edited
            .set_reminder_thresholds(vec![ReminderThreshold::Hours(2)])
            .unwrap();
        f.tasks.save(&edited).await.unwrap();

        // ...e um novo sync não desfaz a escolha dele.
        f.service.sync(options).await.unwrap();
        assert_eq!(
            f.tasks
                .find_by_id(task_id)
                .await
                .unwrap()
                .unwrap()
                .reminder_thresholds,
            vec![ReminderThreshold::Hours(2)]
        );
    }

    #[tokio::test]
    async fn empty_queue_on_a_clean_install_is_a_no_op() {
        let f = fixture(vec![]);
        let report = f.service.sync(SyncOptions::default()).await.unwrap();
        assert_eq!(report, SyncReport::default());
        assert_eq!(report.total_changes(), 0);
    }

    #[tokio::test]
    async fn disconnect_signs_out_and_clears_mirrors_without_notes() {
        let f = fixture(vec![
            item("task-1", "sem anotação"),
            item("task-2", "com anotação"),
        ]);
        f.service.sync(SyncOptions::default()).await.unwrap();

        let with_notes = f
            .tasks
            .list_all()
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.title == "com anotação")
            .unwrap();
        f.notes
            .save(&TaskNote::new(with_notes.id, "importante").unwrap())
            .await
            .unwrap();

        let report = f.service.disconnect().await.unwrap();
        assert_eq!(report.removed, 1);
        assert_eq!(report.kept_with_notes, 1);
        assert!(!f.service.is_configured().await);

        let survivors = f.tasks.list_all().await.unwrap();
        assert_eq!(survivors.len(), 1);
        assert_eq!(survivors[0].id, with_notes.id);
    }
}
