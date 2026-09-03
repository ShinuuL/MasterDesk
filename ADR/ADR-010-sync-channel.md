# ADR-010 — Canal de sincronização com o Mastersys

**Data:** 2026-09-02
**Status:** Aceito. Polling e canal de tempo real implementados. A ressalva de
manutenção da crate foi apresentada ao DEV e **aceita explicitamente** em
2026-09-02.

## Contexto

Até aqui sincronizar era só manual: um chamado reatribuído ao usuário só
aparecia no quadro quando alguém clicava em "Sincronizar". Para quem atende, isso
significa descobrir trabalho novo por acaso.

O pedido do DEV foi "um webhook para monitorar a sincronização em tempo quase
real, e se pesar muito, planejar um polling".

## Webhook está descartado, e não por preferência

Webhook exigiria o Mastersys fazer `POST` para o MasterDesk, o que implica:

1. **Porta de escuta no MasterDesk** — exatamente a opção (a) que a ADR-006
   rejeitou, por superfície de ataque e por depender de app de terceiro.
2. **Alterar o Mastersys** — o DEV instruiu explicitamente a não tocar naquele
   repositório, e a ADR-006 escolheu a opção (c) justamente por não exigir
   mudança do outro lado.

## Descoberta que abriu uma terceira via

O Mastersys **já tem** Socket.IO e **já emite** os eventos que interessam:

| Evento | Sala | Onde |
|---|---|---|
| `task:created` / `task:updated` / `task:deleted` | `tasks` | `modules/tasks/services/TaskService.ts:54,77,169,197` |
| `ticket:created` / `ticket:updated` | `tickets` | `modules/tickets/services/TicketService.ts:422,1110` |

Servidor em `path: '/api/socket.io'`
(`shared/infra/socket/SocketService.ts:23-30`); `join_room` aceita nome de sala
como string, **sem autenticação alguma** (linha 82).

Ou seja: o MasterDesk pode **assinar** como cliente, sem porta de escuta e sem
alterar o Mastersys. Melhor que webhook em toda dimensão que importa aqui.

A própria UI do suporte **ignora o payload** dos eventos e trata cada um como
"invalide e refaça a busca" (`pages/Tasks.tsx:177-194`). Copiamos esse
tratamento: evento é sinal de que algo mudou, não fonte de dados.

## Decisão: polling é o mecanismo; tempo real é aceleração

Duas camadas, e essa ordem é o ponto central da decisão:

1. **Polling** (implementado) — timer configurável, padrão 5 min, piso de 1 min.
   É o mecanismo **garantido**: sem dependência nova, sem acoplamento ao socket,
   e funciona mesmo que o Mastersys mude o canal de tempo real.
2. **Socket.IO** (implementado) — reduz a latência de minutos para segundos
   empurrando `SyncTrigger::Realtime` na mesma fila. Se falhar, sair do ar ou
   ganhar autenticação, **perde-se latência, não função**.

Essa hierarquia é deliberada. O canal de tempo real do Mastersys não é
autenticado; quando o DEV corrigir isso (e deve — ver "Achado de segurança"), um
cliente que dependesse dele pararia de funcionar. Com o polling como mecanismo
e o socket como aceleração, essa correção custa latência e nada mais.

### Desenho: fila de gatilhos, não `setInterval`

`src-tauri/src/sync_scheduler.rs` espera em `select!` entre um timer e um canal
de pedidos. Duas propriedades vêm de graça disso:

- **Coalescência** (janela de 2 s) e **intervalo mínimo entre syncs** (15 s).
  Necessários porque as salas `tasks`/`tickets` são **globais**: chegam eventos
  de todos os usuários da empresa, não só do dono da máquina. Sem coalescer, um
  time movimentado dispararia sincronização sem parar.
- **Extensibilidade sem reescrever**: o cliente de socket só chama
  `handle.request(SyncTrigger::Realtime)` — coalescência, intervalo mínimo e
  evento para a UI já estavam prontos quando ele entrou.

Pedido dentro do intervalo mínimo é **descartado**, não enfileirado — enfileirar
sob salas globais viraria uma fila infinita, e o próximo ciclo do timer cobre a
mudança de qualquer forma.

### Decisões menores, com o motivo

- **Falha de sync automático é silenciosa.** VPN caída é rotina; um toast a cada
  5 minutos treinaria o usuário a ignorar avisos. O erro aparece quando ele
  sincroniza à mão.
- **Evento para a UI só quando `total_changes() > 0`.** Emitir a cada ciclo faria
  o quadro recarregar de 5 em 5 minutos, perdendo scroll e seleção.
- **Lembretes padrão persistidos** (`mastersys.default_reminders`). O comando
  manual recebe os lembretes da UI; o timer não tem UI. Sem persistir, o mesmo
  item nasceria com lembrete pelo botão e sem lembrete pelo timer — diferença
  invisível que só apareceria como "o alarme não tocou".
- **Cliente no Rust, não no webview.** Três razões: a arquitetura põe integração
  na infraestrutura; CORS não se aplica a cliente nativo; e a CSP de produção
  fechada teria de reabrir `connect-src` para o host do Mastersys.

## Pesquisa da dependência (CLAUDE Rule 2) — `rust_socketio`

Verificado em 2026-09-02, direto nas fontes:

| Critério | Achado | Veredito |
|---|---|---|
| Protocolo | "revision 5 of the socket.io protocol and therefore revision 4 of the engine.io protocol" | ✅ casa com o servidor `socket.io@4.8.3` |
| **Path customizado** | Sem método no builder, **mas** `connect_raw` faz `if url.path() == "/" { url.set_path("/socket.io/") }` — path não-raiz é **preservado**. Então `https://host/api/socket.io/` funciona. | ✅ verificado no código-fonte, não suposto |
| Licença | MIT | ✅ |
| Reconexão | `reconnect`, `reconnect_on_disconnect`, `reconnect_delay(min,max)`, `max_reconnect_attempts` | ✅ |
| **Manutenção** | Último release **0.6.0, abril de 2024**. Último commit **fevereiro de 2025**. 51 issues abertas. Não arquivado. | ⚠️ dormente há ~19 meses |
| Async | Feature `async` declarada **experimental**: "the interface can be object to changes at any time" | ⚠️ usar o cliente síncrono em thread própria |

**A dormência é a única objeção real.** Foi apresentada ao DEV com os números
acima e **aceita**. O risco é contido pelo desenho em duas camadas: se a crate
quebrar num Rust futuro, remove-se a dependência e o polling continua
entregando a função — perde-se latência, não função.

Mitigações concretas adotadas:
- Feature `async` **não** usada (é declarada experimental); cliente síncrono.
- Nenhum erro do canal chega à UI como falha.
- `build_socket_url` é função pura e testada, porque é a parte que erra em
  silêncio: path errado responde 404 no handshake sem mensagem legível.
- O payload dos eventos é **ignorado**, então mudanças no formato do payload do
  Mastersys não podem quebrar o cliente.

## Consequências

**Já valendo:**
- Quadro se atualiza sozinho: tempo real quando o canal está de pé, polling de
  5 min (configurável) como piso garantido.
- `RealtimeSupervisor` liga/desliga o canal em quatro momentos — boot, conectar,
  trocar endereço, desconectar.
- Entrar nas salas acontece no callback de `open`, **não** após `connect()`,
  porque `open` dispara de novo a cada reconexão. Sem isso o cliente ficaria
  conectado mas mudo depois da primeira queda — o mesmo cuidado que o
  `InternalChatContext` do suporte tem ao reemitir `chat:join`.
- Indicador na UI distingue "tempo real" de "a cada N min". Existe porque
  "demorou 5 minutos para aparecer" precisa ter explicação visível, senão parece
  bug.
- `mastersys_realtime_connected`, `mastersys_poll_interval` e
  `mastersys_set_poll_interval` expõem estado e controle.

**Aceito como custo:**
- Uma dependência dormente (ver acima).
- Duas requisições por ciclo de polling por máquina conectada. Com o padrão de
  5 min é desprezível; o piso de 1 min existe para impedir que alguém
  transforme isso em carga.
- Eventos das salas globais chegam de todos os usuários da empresa, então há
  sincronizações disparadas por mudanças que não são suas. A coalescência de 2 s
  e o intervalo mínimo de 15 s são o que mantém isso barato.

## Achado de segurança no Mastersys — reportado, não corrigido

`shared/infra/socket/SocketService.ts:36-56`: `chat:join` aceita o `userId`
**direto do cliente, sem verificar JWT**, e faz `socket.join('user:' + uid)`.
Qualquer cliente conectado entra na sala de qualquer usuário e recebe os eventos
dele. `join_room` (linha 82) tem o mesmo problema com nome de sala arbitrário.

Não tocado — pasta somente leitura, e a correção é decisão do DEV do Mastersys.
É o motivo pelo qual o tempo real aqui é oportunista e o polling é o mecanismo.
