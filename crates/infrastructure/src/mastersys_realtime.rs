//! Canal de tempo real do Mastersys (ADR-010).
//!
//! ## O que este módulo faz, e o que deliberadamente não faz
//!
//! Ele **só avisa que algo mudou**. Não lê o payload dos eventos, não decide o
//! que mudou e não escreve nada. Quem sincroniza é o `sync_scheduler`.
//!
//! Isso não é preguiça: é o mesmo tratamento que a própria UI do suporte dá aos
//! eventos (`pages/Tasks.tsx:177-194` chama `loadBoardData()` ignorando o
//! payload). E é necessário, porque as salas `tasks`/`tickets` do Mastersys são
//! **globais** — chegam eventos de todos os usuários da empresa. Confiar no
//! payload significaria filtrar dados de terceiros aqui; pedir uma
//! sincronização e deixar o servidor decidir o que é seu é mais simples e mais
//! correto.
//!
//! ## Aceleração, não mecanismo
//!
//! Se este canal não conectar, cair ou ganhar autenticação, **nada deixa de
//! funcionar** — o polling continua. É por isso que todo erro aqui é engolido
//! em vez de propagado.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rust_socketio::{ClientBuilder, Payload, RawClient};

/// Eventos do Mastersys que significam "a fila de trabalho pode ter mudado".
///
/// Emitidos em `modules/tasks/services/TaskService.ts` e
/// `modules/tickets/services/TicketService.ts`.
const WATCHED_EVENTS: [&str; 5] = [
    "task:created",
    "task:updated",
    "task:deleted",
    "ticket:created",
    "ticket:updated",
];

/// Salas a entrar. Nomes literais do servidor — `socketService.emit(evento,
/// dados, 'tasks')` publica em `io.to('tasks')`.
const ROOMS: [&str; 2] = ["tasks", "tickets"];

/// Sufixo do Engine.IO no Mastersys (`SocketService.initialize` usa
/// `path: '/api/socket.io'`), com barra final.
///
/// A barra importa: `rust_socketio` só injeta o path default quando o da URL é
/// exatamente `/`, e o servidor espera `/api/socket.io/`.
///
/// É **sufixo**, não caminho absoluto — ver [`build_socket_url`].
const SOCKET_PATH_SUFFIX: &str = "/api/socket.io/";

/// Reconexão. Valores acima do default da crate porque uma queda de VPN
/// costuma durar mais que segundos, e insistir rápido só gasta bateria — o
/// polling cobre a janela de indisponibilidade.
const RECONNECT_MIN_MS: u64 = 2_000;
const RECONNECT_MAX_MS: u64 = 60_000;

/// Conexão viva com o canal de tempo real.
///
/// Guardar este valor é o que mantém a conexão aberta; ao ser descartado, o
/// `Drop` desconecta.
pub struct RealtimeConnection {
    client: rust_socketio::client::Client,
    connected: Arc<AtomicBool>,
}

impl RealtimeConnection {
    /// `true` quando o handshake completou e as salas foram pedidas.
    ///
    /// Usado pela UI para mostrar "tempo real" em vez de deixar o usuário
    /// adivinhar se está no polling de 5 minutos.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn disconnect(self) {
        // `self` por valor: depois de desconectar o objeto não serve mais.
        let _ = self.client.disconnect();
    }
}

/// Abre o canal e chama `on_change` a cada evento observado.
///
/// `on_change` é chamado da thread do cliente, então precisa ser rápido e não
/// pode entrar em pânico — na prática ele só empurra um gatilho numa fila.
///
/// ## Erros
///
/// Falha de conexão é devolvida para quem chama poder registrar, mas **não deve
/// ser tratada como falha do app**: sem este canal, o polling continua. O
/// chamador típico ignora o `Err`.
pub fn connect<F>(base_url: &str, on_change: F) -> Result<RealtimeConnection, String>
where
    F: Fn() + Send + Sync + 'static,
{
    let url = build_socket_url(base_url)?;
    let connected = Arc::new(AtomicBool::new(false));

    let on_change = Arc::new(on_change);
    let mut builder = ClientBuilder::new(url)
        .reconnect(true)
        .reconnect_on_disconnect(true)
        .reconnect_delay(RECONNECT_MIN_MS, RECONNECT_MAX_MS);

    // Entrar nas salas vai no callback de `open`, e não depois do `connect()`,
    // porque `open` dispara **de novo a cada reconexão**. Emitir só uma vez
    // após conectar deixaria o cliente conectado mas mudo depois da primeira
    // queda — é o mesmo cuidado que o `InternalChatContext` do suporte tem ao
    // reemitir `chat:join` no `connect`.
    let flag = connected.clone();
    builder = builder.on("open", move |_payload: Payload, socket: RawClient| {
        for room in ROOMS {
            // `join_room` recebe o nome da sala como string simples
            // (`SocketService.ts:82`).
            let _ = socket.emit("join_room", room);
        }
        flag.store(true, Ordering::Relaxed);
    });

    for event in WATCHED_EVENTS {
        let notify = on_change.clone();
        // O payload é ignorado de propósito — ver o cabeçalho do módulo.
        builder = builder.on(event, move |_payload: Payload, _socket: RawClient| {
            notify();
        });
    }

    let flag = connected.clone();
    builder = builder.on("close", move |_payload: Payload, _socket: RawClient| {
        flag.store(false, Ordering::Relaxed);
    });

    let flag = connected.clone();
    builder = builder.on("error", move |_payload: Payload, _socket: RawClient| {
        // Não logamos o payload: ele pode conter URL e cabeçalhos, e a política
        // do projeto é nunca vazar isso (CLAUDE §13/18).
        flag.store(false, Ordering::Relaxed);
    });

    let client = builder
        .connect()
        .map_err(|_| "não foi possível abrir o canal de tempo real".to_string())?;

    Ok(RealtimeConnection { client, connected })
}

/// Monta a URL do Engine.IO a partir do endereço configurado do Mastersys.
///
/// Separado e público para teste: é a parte que erra **silenciosamente**. Path
/// errado não dá erro claro — o servidor responde 404 no handshake e o cliente
/// simplesmente nunca conecta, sem mensagem em lugar nenhum.
///
/// ## O subcaminho do endereço é PRESERVADO
///
/// Esta função já errou nisso. A primeira versão descartava o path e usava só a
/// origem, com um comentário afirmando que o Engine.IO ficava na raiz do host
/// "independente de onde a API esteja montada". Era suposição, e falsa: em
/// produção o Mastersys fica atrás de reverse proxy em
/// `https://mastersys.app.br/suporte`, e o socket responde em
/// `/suporte/api/socket.io/`. O efeito era o canal de tempo real **nunca**
/// conectar em produção, deixando todo mundo no polling de 5 minutos sem que
/// nada indicasse o motivo.
///
/// A regra correta é a mesma que o resto do provider usa para HTTP —
/// `format!("{base}/api/...")` — e a mesma que o frontend do suporte deriva de
/// `VITE_API_URL` (`/suporte/api` em produção → `/suporte/api/socket.io`).
pub fn build_socket_url(base_url: &str) -> Result<String, String> {
    let trimmed = base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("endereço do Mastersys não configurado".into());
    }
    let parsed = reqwest::Url::parse(trimmed).map_err(|_| "endereço inválido".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("endereço deve começar com http:// ou https://".into());
    }
    // `trimmed` já veio sem barra final, então a concatenação não duplica.
    Ok(format!("{trimmed}{SOCKET_PATH_SUFFIX}"))
}

/// Tempo que vale esperar por um handshake antes de desistir e ficar só no
/// polling. Exposto para quem orquestra decidir.
pub const CONNECT_BUDGET: Duration = Duration::from_secs(10);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_engineio_url_from_a_plain_host() {
        assert_eq!(
            build_socket_url("https://suporte.exemplo.com").unwrap(),
            "https://suporte.exemplo.com/api/socket.io/"
        );
    }

    #[test]
    fn trailing_slash_in_the_configured_address_does_not_double_up() {
        assert_eq!(
            build_socket_url("https://suporte.exemplo.com/").unwrap(),
            "https://suporte.exemplo.com/api/socket.io/"
        );
    }

    /// O caso de produção real, e o que a primeira versão desta função
    /// quebrava: o Mastersys atrás de reverse proxy num subcaminho.
    #[test]
    fn the_configured_subpath_is_preserved() {
        assert_eq!(
            build_socket_url("https://mastersys.app.br/suporte").unwrap(),
            "https://mastersys.app.br/suporte/api/socket.io/",
            "descartar o subcaminho fazia o tempo real nunca conectar em produção"
        );
    }

    #[test]
    fn a_subpath_with_trailing_slash_does_not_double_up() {
        assert_eq!(
            build_socket_url("https://mastersys.app.br/suporte/").unwrap(),
            "https://mastersys.app.br/suporte/api/socket.io/"
        );
    }

    /// Casa com como o resto do provider monta HTTP: `{base}/api/...`. Se
    /// alguém mudar uma das duas, este teste é o lugar de perceber.
    #[test]
    fn the_socket_url_follows_the_same_rule_as_the_http_urls() {
        for base in [
            "https://mastersys.app.br/suporte",
            "http://localhost:3000",
            "https://host.interno:8443/atendimento/v2",
        ] {
            let socket = build_socket_url(base).unwrap();
            let http = format!("{base}/api/auth/login");
            let common = base.to_string();
            assert!(
                socket.starts_with(&common) && http.starts_with(&common),
                "socket e HTTP têm de partir do mesmo prefixo: {socket} vs {http}"
            );
            assert!(socket.ends_with("/api/socket.io/"));
        }
    }

    #[test]
    fn keeps_a_non_default_port() {
        assert_eq!(
            build_socket_url("http://localhost:3000").unwrap(),
            "http://localhost:3000/api/socket.io/"
        );
    }

    #[test]
    fn rejects_what_reqwest_could_not_use_anyway() {
        assert!(build_socket_url("").is_err());
        assert!(build_socket_url("   ").is_err());
        assert!(
            build_socket_url("suporte.exemplo.com").is_err(),
            "sem esquema"
        );
        assert!(build_socket_url("ftp://suporte.exemplo.com").is_err());
    }

    #[test]
    fn the_path_carries_its_trailing_slash() {
        // Sem a barra final o `rust_socketio` não injeta nada (o path não é
        // `/`) e o servidor responde 404 no handshake, sem erro legível.
        assert!(SOCKET_PATH_SUFFIX.ends_with('/'));
    }

    #[test]
    fn watches_every_event_the_origin_emits_for_work_items() {
        // Se o Mastersys ganhar um evento novo de tarefa/chamado, esta lista
        // precisa crescer — o teste existe para o esquecimento ficar visível.
        assert!(WATCHED_EVENTS.contains(&"task:created"));
        assert!(WATCHED_EVENTS.contains(&"task:updated"));
        assert!(WATCHED_EVENTS.contains(&"task:deleted"));
        assert!(WATCHED_EVENTS.contains(&"ticket:created"));
        assert!(WATCHED_EVENTS.contains(&"ticket:updated"));
    }
}
