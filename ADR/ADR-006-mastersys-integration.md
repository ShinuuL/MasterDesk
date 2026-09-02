# ADR-006 — Integração Mastersys

**Status:** Aceito (2026-09-02) — substitui a versão "Bloqueado por falta de contrato"

## Contexto

O MasterDesk precisa mostrar, no quadro local de tarefas, as tarefas e chamados
atribuídos ao usuário no **Mastersys Suporte**
(`alrindoMaster/gerenciador_relatorios_V3`).

Este ADR estava bloqueado pela seção 10 do CLAUDE.md — "Never implement a
Mastersys API call until the real API contract has been validated". O bloqueio
caiu porque o contrato foi **lido do código-fonte do Mastersys**, não suposto.
Cada endpoint e cada campo usados aqui têm origem rastreável:

| O que | Onde foi verificado |
|---|---|
| `POST /api/auth/login` `{identifier, password}` | `modules/users/routes.ts:99`, `controllers/AuthController.ts` (`loginSchema`) |
| Resposta `{success, data:{accessToken, refreshToken, user:{id,name,email,…}}}` | `modules/users/services/AuthService.ts` (`login`) |
| `POST /api/auth/refresh` `{refreshToken}` → `{success, data:{accessToken}}` | `routes.ts:127`, `AuthService.refreshToken` |
| `Authorization: Bearer <accessToken>` | `shared/infra/http/middlewares/authMiddleware.ts` |
| Prefixos `/api/tasks`, `/api/tickets` | `shared/infra/http/app.ts` |
| `GET /api/tasks/users/:userId` → array **cru** de `TaskDTO` | `modules/tasks/routes.ts`, `TaskController.getByUser` (`res.json(tasks)`) |
| Campos de `TaskDTO` | `modules/tasks/dtos/TaskDTO.ts` |
| `GET /api/tickets?assignedTo=<id>` → `{success, data:[TicketDTO]}` | `modules/tickets/routes.ts`, `TicketController.list` + `filtersSchema` |
| Campos de `TicketDTO`, prioridades `low\|medium\|high\|critical` | `modules/tickets/dtos/index.ts` |
| Regra de prazo efetivo | `modules/tasks/utils/overdue.ts` (`getEffectiveDueDate`) |

### Descoberta que mudou a decisão

O Mastersys **já tem** uma integração pronta com um app local de notas,
chamada NoteDesk (`modules/tasks/services/NoteDeskSyncService.ts` +
`frontend/src/hooks/useNoteDeskBridge.ts`). Ela funciona assim: o backend
enfileira um payload em `task_notedesk_outbox` e uma ponte no navegador do
usuário entrega em `POST http://127.0.0.1:17882/api/v1/tasks/upsert`, autenticado
por `X-NoteDesk-Api-Key`, no app "Notas Flutuantes"
(`%LOCALAPPDATA%\NotasFlutuantes\integracao.json`).

Isso era relevante porque o `docs/INTEGRACAO_NOTEDESK.md` deste repositório
descrevia a direção **invertida** (um cliente do MasterDesk empurrando para o
NoteDesk), o que não corresponde ao contrato real.

## Opções

### (a) MasterDesk implementa o contrato NoteDesk (servidor HTTP local)

O MasterDesk sobe um servidor em `127.0.0.1:17882` com `GET /api/v1/health` e
`POST /api/v1/tasks/upsert`, e a ponte existente do Mastersys entrega nele.

- **A favor:** zero mudança no backend Mastersys; a fila garante entrega mesmo
  com o app fechado; empurrado em vez de consultado, então chega quase na hora.
- **Contra:** exige que o MasterDesk substitua o Notas Flutuantes (a porta e a
  chave são únicas por máquina); abre uma porta de escuta no computador do
  usuário, com toda a superfície de ataque que isso traz (autenticação por API
  key em texto num JSON, CORS, Private Network Access do Chrome); torna o
  MasterDesk dependente de o navegador do usuário estar aberto na página do
  Mastersys, porque é o navegador que faz a entrega.

### (b) Coexistir com o Notas Flutuantes em outra porta

Mesmo contrato, porta diferente.

- **Contra:** exige mudança no Mastersys (segunda fila e segunda configuração
  por usuário), ou seja, deixa de ser "integração sem tocar no outro lado".
  Herda todos os contras de (a).

### (c) MasterDesk consulta a API do Mastersys — **escolhida**

O MasterDesk autentica com as credenciais do próprio usuário e lê
`GET /api/tasks/users/:userId` e `GET /api/tickets?assignedTo=`.

- **A favor:** nenhuma porta de escuta e nenhuma API key compartilhada — a
  superfície de ataque desaparece; não depende de navegador aberto nem de app
  de terceiro instalado; não exige nenhuma alteração no Mastersys; o MasterDesk
  fica dono do seu próprio ciclo de sincronização.
- **Contra:** é *pull*, então há latência até a próxima sincronização; exige
  guardar um token de sessão na máquina; consome os endpoints de leitura
  (mitigado por serem consultas simples e sob demanda).

## Decisão

**Opção (c): o MasterDesk consulta a API do Mastersys, em modo somente leitura.**

Decidido pelo DEV em 2026-09-02.

### Somente leitura, deliberadamente

`SupportSystemProvider` não tem nenhum método de escrita. Fechar chamado,
comentar ou reatribuir continuam sendo feitos no Mastersys. Isso mantém o
MasterDesk fora do caminho crítico do suporte: um bug local não pode alterar
registro de atendimento de cliente (CLAUDE §12/18).

### Modelo de dados

Uma `Task` do MasterDesk pode carregar uma `ExternalRef` opcional. `None` = tarefa
puramente local, que é o caso padrão e continua funcionando sem qualquer
integração configurada (CLAUDE §5: "Tasks must not require a Mastersys ticket").

Propriedade de campos por sincronização:

| Campo | Dono | Comportamento no sync |
|---|---|---|
| título, descrição, prioridade, prazo, concluída | Mastersys | sobrescritos |
| anotações da tarefa (`task_notes`) | usuário | nunca tocadas |
| thresholds de lembrete | usuário | só definidos na importação inicial |

Espelhos que saem da fila do usuário são apagados — **exceto** os que têm
anotações, que ficam marcados como concluídos. Anotação é trabalho manual e não
pode ser descartada por uma sincronização.

### Identificação dos itens

`external_id` é prefixado por origem: `task-<id>` e `ticket-<id>`. O id 12 de
`tasks` e o id 12 de `tickets` são itens diferentes; sem o prefixo colidiriam no
índice único local. Um chamado que já tem tarefa no quadro não é importado duas
vezes: a varredura de chamados salta os `ticketId` já vistos nas tarefas.

### Prazo

`effective_due_date` replica `getEffectiveDueDate` do Mastersys. A regra não é
"a primeira data que existir": havendo previsão **e** agendamento do chamado,
vale a mais próxima entre as futuras; se ambas passaram, vale a mais recente.
Manter a mesma regra importa porque é ela que decide a hora do lembrete
(CLAUDE §19 — cálculo de prazo é business-critical). Coberto por testes.

`TaskDTO` do Mastersys **não tem campo de prioridade**. Tarefas importadas
ficam em `Medium`; chamados usam a prioridade real do chamado. Derivar
prioridade de outra coisa seria inventar um dado que a origem não tem.

`TicketStatus` no Mastersys aceita status customizados criados na tela de
configuração, então "concluído" para chamados é decidido por `closedAt`/
`resolvedAt` (timestamps estáveis) e não por uma lista de slugs.

### Credenciais

- A senha é usada na chamada de login e descartada; nunca é persistida.
- O **refresh token** vai para o cofre nativo do SO (`SecretStore` sobre
  `keyring`): Windows Credential Manager, macOS Keychain, Linux Secret Service.
- O **access token** vive só em memória.
- Endpoint, id, nome e e-mail do usuário — configuração, não segredo — ficam em
  `app_settings` no SQLite.
- Nenhum comando Tauri devolve token ao frontend. `MastersysStatus` expõe
  endpoint, `connected` e identidade; nada mais.
- Mensagens de erro são escritas à mão em vez de repassar o `Display` do
  `reqwest::Error`, que pode conter a URL completa (CLAUDE §13/18).

## Consequências

### Positivas

- Nenhuma porta de escuta no computador do usuário; nenhuma API key
  compartilhada em arquivo de texto.
- Zero alteração no Mastersys — a integração não precisa de deploy do outro lado.
- O domínio continua sem saber que HTTP existe: `MastersysProvider` é o único
  módulo com `reqwest`, e vive em `infrastructure`.
- Trocar o Mastersys por outro sistema de suporte é implementar
  `SupportSystemProvider` de novo, sem tocar em `application`/`domain`.

### Negativas e limitações conhecidas

- **É pull.** Uma tarefa criada no Mastersys aparece no MasterDesk na próxima
  sincronização, não no instante. Hoje a sincronização é manual (botão
  "Sincronizar agora"); um agendador periódico é trabalho futuro.
- **Sem cofre, sem sessão.** Em Linux headless (sem GNOME Keyring/KWallet) o
  `keyring` não tem backend e o login falha com mensagem explícita, em vez de
  degradar para token em texto plano.
- **Não validado nos três SOs.** O código compila e é testado nos três, mas o
  fluxo real de login+sync foi exercitado apenas em Windows 11. O cofre no
  macOS e no Linux precisa de validação manual antes de declarar suporte
  (CLAUDE §19: "Compilation alone is not cross-platform validation").
- **TLS depende da loja do SO.** As features default de `reqwest` 0.13 usam
  `rustls` + `rustls-platform-verifier`, que valida contra a loja de
  certificados do sistema. CA corporativa instalada no Windows funciona;
  certificado autoassinado **não** instalado no SO é recusado — por decisão, não
  há opção de ignorar validação.
- **Permissões.** Se o usuário do Mastersys não tiver acesso a
  `GET /api/tickets`, a sincronização retorna erro de permissão. Hoje isso
  aborta a sincronização inteira; degradar para "só tarefas" é melhoria futura.

## Política de dependências (CLAUDE §16)

| | reqwest | keyring |
|---|---|---|
| Versão | 0.13 | 3.6 |
| Propósito | cliente HTTP para a API do Mastersys | cofre nativo de credenciais |
| Documentação | https://docs.rs/reqwest | https://docs.rs/keyring |
| Licença | MIT OR Apache-2.0 | MIT OR Apache-2.0 |
| SOs | Windows/macOS/Linux (TLS via loja do SO, sem OpenSSL) | Credential Manager / Keychain / Secret Service |
| Manutenção | ativa (0.13.4, 2026-05) | ativa (3.6.3, 2025-07) |
| Limitações | ver TLS acima | Linux exige agente de Secret Service **em execução** (runtime) e `libdbus-1-dev` **para compilar** — a feature `sync-secret-service` linka via `libdbus-sys`, que é um crate `-sys`. Já adicionado ao CI. |
| Alternativas | `ureq` (sem async), `tauri-plugin-http` (pensado para o frontend chamar, não o Rust) | `tauri-plugin-stronghold` (exige senha própria), keyring 4.x |
| Por que | já estava no grafo de dependências via Tauri; default sem OpenSSL simplifica build e CI | API estável e amplamente usada; **4.x foi evitada** porque migrou para `keyring-core` com registro explícito de store e documentação ainda escassa |

## Trabalho futuro

- Sincronização periódica em segundo plano, com intervalo configurável.
- Degradar por endpoint em vez de abortar (só tarefas, quando faltar permissão
  de chamados).
- Abrir o chamado no navegador a partir do card (precisa da rota da UI do
  Mastersys, ainda não verificada).
- Validar cofre e sincronização em macOS e Linux.
