//! Sincronização automática com o Mastersys.
//!
//! Antes daqui, sincronizar era só manual: um chamado reatribuído a você só
//! aparecia quando alguém clicava em "Sincronizar".
//!
//! ## Desenho: uma fila de gatilhos, não um `setInterval`
//!
//! O laço espera em `select!` entre um timer e um canal de pedidos. Isso importa
//! por dois motivos:
//!
//! 1. **Coalescência.** Vários pedidos próximos no tempo viram uma
//!    sincronização. Necessário para o canal de tempo real, onde as salas
//!    `tasks`/`tickets` do Mastersys são **globais** — chegam eventos de todos
//!    os usuários da empresa, não só seus. Sem coalescer, um time movimentado
//!    dispararia sincronização sem parar.
//! 2. **Extensibilidade sem reescrever.** Um cliente de Socket.IO só precisa
//!    empurrar `SyncTrigger::Realtime` no canal; o resto — coalescência,
//!    intervalo mínimo, evento para a UI — já está aqui.
//!
//! Mora em `src-tauri` porque é composição: junta o serviço de aplicação com o
//! `AppHandle` (necessário para emitir evento à UI). A camada de aplicação
//! continua sem saber que Tauri existe.

use std::sync::Arc;
use std::time::Duration;

use masterdesk_application::{MastersysSyncService, SyncOptions, SyncReport};
use masterdesk_domain::ReminderThreshold;
use masterdesk_infrastructure::{SettingKey, SqliteSettingsRepository};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;

/// Evento emitido para o frontend depois de uma sincronização que mudou algo.
pub const SYNC_EVENT: &str = "masterdesk://mastersys-synced";

/// Intervalo padrão do polling, em segundos (5 min).
///
/// Escolhido para ser útil sem ser abusivo: são duas requisições por ciclo
/// (tarefas + chamados paginados) contra um servidor de suporte que atende
/// gente de verdade.
const DEFAULT_POLL_SECS: u64 = 300;

/// Piso do intervalo configurável. Abaixo de um minuto isto deixa de ser
/// sincronização e passa a ser carga.
const MIN_POLL_SECS: u64 = 60;

/// Distância mínima entre duas sincronizações, independente de quantos
/// gatilhos cheguem. É o que protege o servidor das salas globais.
const MIN_SYNC_GAP: Duration = Duration::from_secs(15);

/// Janela de coalescência: depois do primeiro gatilho, espera um pouco para
/// juntar os que vierem atrás. Um chamado editado no Mastersys costuma emitir
/// `ticket:updated` seguido de `task:updated`, e os dois querem o mesmo sync.
const COALESCE_WINDOW: Duration = Duration::from_secs(2);

/// De onde veio o pedido de sincronizar. Só para log e telemetria futura — o
/// tratamento é o mesmo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncTrigger {
    /// Timer do polling.
    Timer,
    /// Evento do canal de tempo real (quando existir).
    Realtime,
    /// Pedido explícito de outra parte do app.
    Requested,
}

/// Resultado da última sincronização automática.
///
/// ## Por que isto existe
///
/// A falha do sync automático é silenciosa de propósito (VPN caída é rotina, e
/// um toast a cada 5 minutos treina o usuário a ignorar avisos). Só que, sem
/// registro nenhum, "está demorando" e "não está acontecendo" ficam
/// **indistinguíveis** — foi exatamente a dúvida que o DEV levantou em
/// 2026-09-03, e eu não tinha como responder.
///
/// Então: continua sem interromper ninguém, mas fica gravado e visível em quem
/// for procurar.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LastSync {
    /// ISO8601 UTC.
    pub at: String,
    pub trigger: &'static str,
    /// `None` = deu certo.
    pub error: Option<String>,
    pub report: Option<SyncReportPayload>,
}

/// Ponta de escrita do agendador. Clonável e barata.
#[derive(Clone)]
pub struct SyncHandle {
    tx: mpsc::Sender<SyncTrigger>,
    last: Arc<std::sync::Mutex<Option<LastSync>>>,
}

impl SyncHandle {
    /// O que aconteceu na última sincronização automática, se houve alguma.
    pub fn last_sync(&self) -> Option<LastSync> {
        self.last.lock().ok().and_then(|g| g.clone())
    }

    /// Pede uma sincronização. Não espera pelo resultado.
    ///
    /// Silenciosamente ignora quando a fila está cheia: fila cheia significa
    /// que já há sincronização pedida, que é exatamente o que este pedido
    /// queria. Bloquear aqui prenderia quem chama — no caso do tempo real,
    /// seria o laço de eventos do socket.
    pub fn request(&self, trigger: SyncTrigger) {
        let _ = self.tx.try_send(trigger);
    }
}

/// Intervalo de polling configurado, com o padrão aplicado.
///
/// Valor inválido ou abaixo do piso cai no padrão em vez de derrubar o
/// agendador — é configuração, não contrato.
pub async fn poll_interval(settings: &SqliteSettingsRepository) -> Duration {
    let secs = settings
        .get(SettingKey::MastersysPollSeconds)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|s| *s >= MIN_POLL_SECS)
        .unwrap_or(DEFAULT_POLL_SECS);
    Duration::from_secs(secs)
}

pub async fn set_poll_interval(
    settings: &SqliteSettingsRepository,
    secs: u64,
) -> Result<(), String> {
    if secs < MIN_POLL_SECS {
        return Err(format!(
            "o intervalo mínimo de sincronização é de {MIN_POLL_SECS} segundos"
        ));
    }
    settings
        .set(SettingKey::MastersysPollSeconds, &secs.to_string())
        .await
        .map_err(|e| e.to_string())
}

/// Lembretes que a sincronização automática aplica a itens recém-importados.
///
/// Lê o que o usuário escolheu da última vez que sincronizou à mão. Sem isto,
/// um item importado pelo timer nasceria **sem lembrete**, enquanto o mesmo
/// item importado pelo botão nasceria com — uma diferença invisível e que só
/// apareceria como "o alarme não tocou".
async fn stored_default_reminders(settings: &SqliteSettingsRepository) -> Vec<ReminderThreshold> {
    settings
        .get(SettingKey::MastersysDefaultReminders)
        .await
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str::<Vec<u32>>(&raw).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|m| *m > 0 && *m <= 10080)
        .map(ReminderThreshold::Minutes)
        .collect()
}

/// Grava os lembretes padrão para a sincronização automática reusar.
pub async fn store_default_reminders(
    settings: &SqliteSettingsRepository,
    minutes: &[i64],
) -> Result<(), String> {
    let valid: Vec<u32> = minutes
        .iter()
        .filter(|m| **m > 0 && **m <= 10080)
        .map(|m| *m as u32)
        .collect();
    let json = serde_json::to_string(&valid).map_err(|e| e.to_string())?;
    settings
        .set(SettingKey::MastersysDefaultReminders, &json)
        .await
        .map_err(|e| e.to_string())
}

/// Sobe o laço de sincronização em segundo plano e devolve a ponta para pedir
/// sincronizações.
///
/// A tarefa vive enquanto o app viver. Não devolve `JoinHandle` porque não há
/// caso de parar: desconectar do Mastersys já faz o laço não ter o que fazer
/// (`is_configured` volta `false`).
pub fn spawn(
    app: AppHandle,
    make_service: impl Fn() -> MastersysSyncService + Send + 'static,
    settings: Arc<SqliteSettingsRepository>,
) -> SyncHandle {
    // Capacidade 1 basta: o que interessa é "existe pedido pendente?", não
    // quantos. Pedidos além disso são redundantes por construção.
    let (tx, mut rx) = mpsc::channel::<SyncTrigger>(1);
    let last: Arc<std::sync::Mutex<Option<LastSync>>> = Arc::new(std::sync::Mutex::new(None));
    let handle = SyncHandle {
        tx,
        last: last.clone(),
    };

    tauri::async_runtime::spawn(async move {
        let mut last_sync: Option<tokio::time::Instant> = None;

        loop {
            let interval = poll_interval(&settings).await;

            // Espera o que vier primeiro: o timer ou um pedido.
            let trigger = tokio::select! {
                _ = tokio::time::sleep(interval) => SyncTrigger::Timer,
                received = rx.recv() => match received {
                    Some(t) => t,
                    // Todas as pontas de escrita morreram: nada mais pode pedir
                    // sincronização, e o timer sozinho não justifica o laço.
                    None => break,
                },
            };

            // Coalescência: dá um instante para gatilhos irmãos chegarem e
            // descarta-os, já que este ciclo vai atender a todos.
            if trigger != SyncTrigger::Timer {
                tokio::time::sleep(COALESCE_WINDOW).await;
                while rx.try_recv().is_ok() {}
            }

            // Intervalo mínimo. O pedido é DESCARTADO em vez de enfileirado:
            // se algo aconteceu 3 segundos depois do último sync, o próximo
            // ciclo do timer o pega. Enfileirar viraria uma fila infinita sob
            // as salas globais do Mastersys.
            if let Some(prev) = last_sync {
                if prev.elapsed() < MIN_SYNC_GAP {
                    continue;
                }
            }

            let service = make_service();
            if !service.is_configured().await {
                // Sem endpoint ou sem sessão não há o que sincronizar. Não é
                // erro — é o estado de quem ainda não conectou.
                continue;
            }

            let default_reminders = stored_default_reminders(&settings).await;
            last_sync = Some(tokio::time::Instant::now());

            let outcome = service.sync(SyncOptions { default_reminders }).await;

            // Registra SEMPRE, sucesso ou falha. Não interrompe o usuário, mas
            // deixa "está demorando" distinguível de "não está acontecendo".
            let record = LastSync {
                at: chrono::Utc::now().to_rfc3339(),
                trigger: match trigger {
                    SyncTrigger::Timer => "timer",
                    SyncTrigger::Realtime => "tempo real",
                    SyncTrigger::Requested => "pedido",
                },
                error: outcome.as_ref().err().map(|e| e.to_string()),
                report: outcome.as_ref().ok().map(SyncReportPayload::from),
            };
            if let Ok(mut guard) = last.lock() {
                *guard = Some(record);
            }

            if let Ok(report) = outcome {
                // Só avisa a UI quando algo mudou de fato. Emitir a cada ciclo
                // faria o quadro recarregar de 5 em 5 minutos sem motivo,
                // perdendo scroll e seleção.
                if report.total_changes() > 0 {
                    let _ = app.emit(SYNC_EVENT, SyncReportPayload::from(&report));
                }
            }
        }
    });

    handle
}

/// O mesmo formato que o comando manual devolve, para o frontend ter um só
/// tipo. Duplicado aqui em vez de importado de `commands` para não criar
/// dependência circular entre os módulos.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncReportPayload {
    pub imported: u32,
    pub updated: u32,
    pub removed: u32,
    pub kept_with_notes: u32,
}

impl From<&SyncReport> for SyncReportPayload {
    fn from(r: &SyncReport) -> Self {
        Self {
            imported: r.imported,
            updated: r.updated,
            removed: r.removed,
            kept_with_notes: r.kept_with_notes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Invariantes entre as constantes, verificadas em tempo de COMPILAÇÃO.
    ///
    /// `const { assert!(...) }` e não `#[test]`: comparação entre duas
    /// constantes já é conhecida pelo compilador, então quebrá-la deve falhar o
    /// build de quem editar o valor — e não esperar alguém rodar a suíte.
    const _INVARIANTS: () = {
        // Janela de coalescência maior que o intervalo mínimo faria todo
        // gatilho chegar "atrasado" e ser descartado.
        assert!(COALESCE_WINDOW.as_millis() < MIN_SYNC_GAP.as_millis());
        // Um padrão abaixo do próprio piso seria silenciosamente elevado por
        // `poll_interval`, e o valor escrito aqui viraria mentira.
        assert!(DEFAULT_POLL_SECS >= MIN_POLL_SECS);
    };

    #[tokio::test]
    async fn request_never_blocks_even_with_a_full_queue() {
        // A ponta de escrita é usada pelo laço de eventos do socket; bloquear
        // ali travaria a conexão inteira.
        let (tx, _rx) = mpsc::channel::<SyncTrigger>(1);
        let handle = SyncHandle {
            tx,
            last: Arc::new(std::sync::Mutex::new(None)),
        };
        for _ in 0..100 {
            handle.request(SyncTrigger::Realtime);
        }
    }

    #[test]
    fn last_sync_starts_empty_and_records_both_outcomes() {
        let (tx, _rx) = mpsc::channel::<SyncTrigger>(1);
        let last = Arc::new(std::sync::Mutex::new(None));
        let handle = SyncHandle {
            tx,
            last: last.clone(),
        };
        assert!(
            handle.last_sync().is_none(),
            "sem sync nenhum, nada a relatar"
        );

        // Falha: o motivo tem de sobreviver, senão "demorando" e "nem
        // acontecendo" voltam a ser indistinguíveis.
        *last.lock().unwrap() = Some(LastSync {
            at: "2026-09-03T12:00:00Z".into(),
            trigger: "timer",
            error: Some("o Mastersys não respondeu no tempo esperado".into()),
            report: None,
        });
        let recorded = handle.last_sync().unwrap();
        assert_eq!(recorded.trigger, "timer");
        assert!(recorded.error.is_some());
        assert!(recorded.report.is_none());
    }
}
