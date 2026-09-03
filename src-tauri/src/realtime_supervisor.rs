//! Mantém o canal de tempo real ligado enquanto houver sessão do Mastersys.
//!
//! ## Por que existe um supervisor, e não só uma conexão
//!
//! A conexão precisa nascer e morrer junto com a sessão: conectar sem endpoint
//! configurado é inútil, e continuar conectado depois de "Desconectar" seria
//! errado. Quatro momentos disparam reavaliação — boot, conectar, trocar o
//! endereço e desconectar — e concentrar isso aqui evita repetir a lógica em
//! cada comando.
//!
//! ## Todo I/O daqui sai do runtime async
//!
//! `rust_socketio` é o cliente **síncrono**: `connect()` e `disconnect()` fazem
//! I/O de rede bloqueante. Chamá-los direto de um comando `async` — que é o que
//! este módulo fazia até 2026-09-03 — bloqueia uma worker thread do tokio pelo
//! tempo do handshake. Em host inalcançável isso é o timeout de TCP (~21 s no
//! Windows), e o efeito visível era o **login do Mastersys ficar lento**, porque
//! `mastersys_connect` esperava o socket antes de retornar.
//!
//! Agora tudo vai para `spawn_blocking` e os chamadores retornam na hora. Como
//! consequência, `start` é *fire-and-forget*: quem chama não sabe se conectou.
//! Isso é aceitável e até desejável — o canal é aceleração, e quem quer saber o
//! estado consulta `is_connected()`.
//!
//! ## Aceleração, não mecanismo (ADR-010)
//!
//! Se a conexão não subir, o `sync_scheduler` continua sincronizando por
//! polling; o usuário perde latência, não função. É por isso que nenhum erro
//! daqui chega à UI como falha.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use masterdesk_infrastructure::mastersys_realtime;
use masterdesk_infrastructure::RealtimeConnection;

use crate::sync_scheduler::{SyncHandle, SyncTrigger};

pub struct RealtimeSupervisor {
    connection: Mutex<Option<RealtimeConnection>>,
    /// Distingue tentativas de conexão. Como conectar virou assíncrono, duas
    /// chamadas próximas (trocar o endereço e em seguida conectar, por exemplo)
    /// podem estar em voo ao mesmo tempo; sem isto, a que terminasse por último
    /// venceria, e não necessariamente é a mais recente.
    generation: AtomicU64,
    sync: SyncHandle,
}

impl RealtimeSupervisor {
    pub fn new(sync: SyncHandle) -> Self {
        Self {
            connection: Mutex::new(None),
            generation: AtomicU64::new(0),
            sync,
        }
    }

    /// `true` quando o canal está de fato conectado e nas salas.
    ///
    /// Distingue "ligado" de "conectado" de propósito: a conexão pode existir e
    /// estar caída em reconexão, e a UI precisa contar essa diferença ao
    /// usuário — senão ele acha que está em tempo real e está no polling.
    pub fn is_connected(&self) -> bool {
        self.connection
            .lock()
            .map(|guard| guard.as_ref().is_some_and(|c| c.is_connected()))
            .unwrap_or(false)
    }

    /// Sobe o canal para este endereço. **Retorna imediatamente**; o handshake
    /// acontece numa thread de bloqueio.
    pub fn start(self: &Arc<Self>, base_url: &str) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.take_connection_and_disconnect();

        let this = self.clone();
        let url = base_url.to_string();
        let sync = self.sync.clone();

        // `spawn_blocking` e não `spawn`: o handshake é I/O bloqueante e
        // prenderia uma worker do tokio.
        tauri::async_runtime::spawn_blocking(move || {
            // O callback roda na thread do cliente Socket.IO. Só empurra um
            // gatilho numa fila de capacidade 1, que nunca bloqueia — travar
            // aqui pararia o laço de eventos do socket inteiro.
            let result = mastersys_realtime::connect(&url, move || {
                sync.request(SyncTrigger::Realtime);
            });

            match result {
                Ok(conn) => {
                    // Alguém pediu outra conexão enquanto esta subia: descarta
                    // a nossa, senão a antiga sobreviveria à mais nova.
                    if this.generation.load(Ordering::SeqCst) != generation {
                        conn.disconnect();
                        return;
                    }
                    if let Ok(mut guard) = this.connection.lock() {
                        *guard = Some(conn);
                    }
                }
                Err(_) => {
                    // Ignorado: ver o cabeçalho. O polling cobre.
                }
            }
        });
    }

    /// Desliga o canal. Também retorna imediatamente — `disconnect()` faz I/O.
    pub fn stop(&self) {
        // Invalida qualquer conexão em voo, senão um handshake que termine
        // depois deste `stop` se instalaria como se nada tivesse acontecido.
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.take_connection_and_disconnect();
    }

    /// Tira a conexão de dentro do mutex e a desconecta fora dele.
    ///
    /// A ordem importa: `disconnect()` bloqueia, e fazê-lo com o mutex na mão
    /// prenderia qualquer `is_connected()` — que é chamado pela UI de 30 em 30
    /// segundos — pelo mesmo tempo.
    fn take_connection_and_disconnect(&self) {
        let taken = self
            .connection
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        if let Some(conn) = taken {
            tauri::async_runtime::spawn_blocking(move || conn.disconnect());
        }
    }

    /// Liga se houver endereço, desliga se não houver.
    ///
    /// É o único ponto que decide "deveria estar ligado?", para os comandos não
    /// precisarem saber a regra.
    pub fn reevaluate(self: &Arc<Self>, base_url: Option<&str>) {
        match base_url {
            Some(url) if !url.trim().is_empty() => self.start(url),
            _ => self.stop(),
        }
    }
}
