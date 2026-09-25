# Plano de implementação — melhorias pendentes (auditoria de 2026-09-25)

Origem: auditoria somente-leitura de desempenho, segurança e UX feita junto com
a entrega de "manter conectado" + "iniciar com o Windows" (ADR-011). Os itens
de esforço pequeno já foram feitos naquela entrega; este plano cobre o resto.

Cada fase é um PR próprio (regra de escopo do `CLAUDE.md`). A ordem vai do
maior risco para o usuário ao menor. Esforço: **P** ≤ meio dia, **M** 1–2 dias,
**G** 3+ dias. Os `arquivo:linha` são da auditoria e podem ter se deslocado —
confira antes de editar.

---

## Já decidido pelo DEV (2026-09-25)

- **http e https continuam aceitos** no endereço do Mastersys. O painel mostra
  aviso quando o endereço é `http://` fora de loopback.
- **SQLite em WAL + `synchronous=NORMAL`**, com o risco de pasta de rede aceito
  e documentado em `src-tauri/src/lib.rs`.
- Trocar o endereço para **outra origem desconecta** antes de gravar.

---

## Fase 0 — Validar o que já foi entregue *(pré-requisito de tudo)*

A entrega anterior foi escrita **sem compilar o Rust** (a máquina não tinha o
componente C++ do Build Tools).

| # | Passo | Critério de pronto |
|---|---|---|
| 0.1 | Instalar a carga "Desenvolvimento para desktop com C++" no Build Tools 2022 (exige administrador) | `link.exe` disponível |
| 0.2 | `cargo build` + `cargo test --workspace` | verde; inclui os 13 testes novos de sessão lembrada |
| 0.3 | `cargo clippy --workspace -- -D warnings` | sem avisos novos |
| 0.4 | Teste manual no Windows: marcar "Manter conectado" → fechar → reabrir entra direto; "Sair" → reabrir pede senha | ok |
| 0.5 | Autostart: ligar → reiniciar o PC → app abre; conferir Gerenciador de Tarefas → Aplicativos de inicialização; desligar por lá → botão mostra desligado | ok |
| 0.6 | WAL: abrir app, conferir `masterdesk.db-wal` em `%APPDATA%\com.masterdesk.app\`; sincronizar enquanto arrasta nota | sem "database is locked" |
| 0.7 | Trocar endereço do Mastersys conectado → vira desconectado; aviso de `http://` aparece | ok |
| 0.8 | Commits separados: (a) manter conectado + autostart + ADR-011, (b) melhorias da auditoria, (c) WAL + aviso http + este plano | rebase limpo sobre `master` |

---

## Fase 1 — Robustez da sincronização *(alto risco, M)*

### 1.1 Um item inválido derruba a sincronização inteira — **P**
- **Onde:** `mastersys_provider.rs` (`items.push(task.to_work_item(...)?)`,
  ~linhas 620 e 630); `application/src/mastersys.rs:180, 275, 287`
  (`apply_external_update(item)?`); validações em `domain/src/entities.rs:1384-1406`
  (título vazio ou > 200 caracteres, descrição > 20 000).
- **Hoje:** um chamado com descrição longa ou título vazio aborta a rodada, e
  isso se repete a cada ciclo, de modo que ninguém mais recebe atualização.
- **Fazer:**
  1. Ao montar o `ExternalWorkItem`, normalizar: título vazio → `"(sem título) #<número>"`,
     título truncado em 200, descrição truncada em 20 000 com marcador `…`.
     Truncar por `chars()`, não por bytes.
  2. No laço do `sync`, trocar o `?` por coleta: item que ainda falhar vai
     para `SyncReport.skipped: Vec<SkippedItem { reference, reason }>` e a
     rodada continua.
  3. Painel do Mastersys mostra "N itens ignorados" com o motivo.
- **Testes:** título vazio, título de 201 caracteres, descrição de 20 001, título só com
  emoji (conta caracteres, não bytes); um item ruim no meio de 3 bons → 2
  importados + 1 ignorado.
- **Cuidado:** não afrouxar a validação do domínio — a normalização é
  responsabilidade do adaptador (fronteira de entrada externa, CLAUDE §18).

### 1.2 N+1 consultas e nenhuma transação — **M**
- **Onde:** `application/src/mastersys.rs:143-146` já carrega todos os espelhos,
  mas `take_orphan_for` (~235) e `reconcile_item` (~253) chamam
  `find_by_external` de novo por item; retirada faz `count_by_task` +
  `delete_by_task` + `delete` por item (~302-318).
- **Fazer:**
  1. Montar `HashMap<dedup_key, Task>` a partir de `mirrors` uma vez e usar
     no lugar de `find_by_external`.
  2. Usar `counts_by_task()` (já existe) uma vez.
  3. Transação: adicionar ao port `TaskRepository` um método de lote
     (`apply_sync_batch(upserts, deletions)`) implementado com `pool.begin()`
     no SQLite. **Mudança de port → confirmar com o DEV antes** (regra de
     contrato do `CLAUDE.md`).
- **Testes:** os existentes de reancoragem e retirada devem continuar verdes; novo
  teste em que uma falha no meio do lote não deixa estado parcial.

### 1.3 Tempo real dispara sync completo por evento de qualquer usuário — **M**
- **Onde:** `mastersys_realtime.rs:121-127` (payload ignorado);
  `sync_scheduler.rs:252-275`; `mastersys_provider.rs:578-596` (buscas em
  série).
- **Fazer:**
  1. Buscas em paralelo com `tokio::try_join!` (tarefas, catálogo e os dois
     papéis de chamados).
  2. Catálogo de status só a cada N ciclos (ex.: 12) ou quando o sync pedir.
  3. `ticket_ids_with_task`: `Vec` + `contains` → `HashSet`.
  4. Pré-filtrar o evento por `assignedTo`/`createdBy`/`userId` do payload.
     **Antes:** confirmar no código do Mastersys quais campos o evento traz —
     não inventar o formato (Regra 1). Sem confirmação, pular este subitem.
- **Testes:** unitário do filtro de evento com payloads reais capturados.

---

## Fase 2 — Lembretes que de fato notificam *(alto valor, G)*

- **Hoje:** `NotificationService` só guarda agendamentos num `HashMap` em
  memória (`notification_service.rs:41`); `fire_due` só é chamado em teste;
  `tauri-plugin-notification` está comentado (`src-tauri/Cargo.toml`,
  `lib.rs`). A UI oferece "Lembretes" e "Adiar 15m", que não fazem nada visível.
- **Passo 0 — pesquisa (obrigatória, ADR-004):** versão atual do
  `tauri-plugin-notification`, comportamento no Windows (toast exige AUMID; o
  app instalado tem, o `tauri dev` pode não ter), som, ações no toast (snooze
  pelo toast é suportado no desktop? verificar — não assumir). Atualizar
  ADR-004 com a decisão.
- **Fazer:**
  1. Worker `tokio::time::interval(30s)` em `src-tauri` chamando `fire_due`.
  2. Adaptador que entrega o disparo ao plugin (port `NotificationService`
     continua no domínio; o plugin fica na infraestrutura/src-tauri).
  3. Reconstruir agendamentos na abertura a partir das tarefas pendentes
     (hoje se perdem ao fechar o app).
  4. "Adiar 15m" dá retorno: toast interno "adiado até HH:MM".
  5. Respeitar `status_parked` (item parado não lembra — regra já existe).
- **Testes:** cálculo de disparo por limiar (5m…2h, custom), reconstrução após
  reinício, snooze, item parado. Validação manual no Windows com app
  instalado.
- **Enquanto não sair:** rotular os controles como "em breve", para não
  prometer o que não acontece.

---

## Fase 3 — Desempenho da interface *(M)*

| # | Item | Onde | Fazer | Esf. |
|---|---|---|---|---|
| 3.1 | Todos os cards re-renderizam a cada tecla na busca | `TasksBoard.tsx:558-717` (`renderTask` inline) | Extrair `<TaskCard>` com `React.memo`; handlers via `useCallback`; `now` calculado uma vez por render | M |
| 3.2 | `searchInput` no board inteiro | `TasksBoard.tsx:107` | Estado do texto dentro de `TaskFilters`, só o valor com debounce sobe | P |
| 3.3 | Trocar de aba desmonta e recarrega tudo | `App.tsx` (render condicional), `TasksBoard.tsx:220-277` | Hook/contexto compartilhado com `pending`/`completed`/`catalog` no `MainApp`, ou manter painéis montados com `hidden` | M |
| 3.4 | Polling de 30 s na aba "local" | `TasksBoard.tsx:272` | Só nas views que usam `liveSync` | P |
| 3.5 | Foco da janela sem throttle | `TasksBoard.tsx:305-321` | Throttle de ~2 s | P |
| 3.6 | Argon2 na thread do runtime | `local_auth_repository.rs` (`hash_password`/`verify_password`, `hash_session_secret`) | `tokio::task::spawn_blocking`; no usuário inexistente, verificar contra hash fictício (fecha o oráculo de tempo) | P |
| 3.7 | Índice faltando | `tasks(updated_at)` usado em `list_all`/`list_completed` | Migration `0012` com `CREATE INDEX IF NOT EXISTS` | P |

**Critério:** perfilar com React DevTools antes/depois em quadro com ~200
tarefas; digitar na busca não deve re-renderizar cards cujo resultado não
mudou.

---

## Fase 4 — Acessibilidade e mensagens *(M)*

| # | Item | Onde | Fazer | Esf. |
|---|---|---|---|---|
| 4.1 | Modal não prende nem devolve o foco | `Modal.tsx:37-57`, `MastersysPanel.tsx:77-84` | Guardar `document.activeElement` e restaurar; trap de Tab/Shift+Tab (ou `inert` no conteúdo de trás); fechar só se `mousedown` **e** `click` forem no overlay; `aria-labelledby`/`aria-describedby` | P |
| 4.2 | Selos de prioridade abaixo de 4,5:1 | `styles.css:515-518`, tokens `--prio-*` (claro High ≈ 3,6; escuro 2,3–3,3) | Tokens `--prio-*-ink` por tema; teste de contraste junto aos de `theme.test.ts`; erro usar `--danger`, não `--prio-urgent` | P |
| 4.3 | Erros em inglês e com prefixo técnico | `domain/src/errors.rs:9-29`, `entities.rs` (validações), `commands.rs` (`e.to_string()` de Tauri/uuid) | `Display` em pt-BR sem "validation failed:"; mapear erros de janela/parse; `formatError(e)` no frontend | M |
| 4.4 | `role="tablist"` no `<nav>` inteiro | `App.tsx` (nav) | Mover para `.md-tabs`; setas + roving `tabIndex` | P |
| 4.5 | Dezenas de "Deletar" idênticos no leitor de tela | `TasksBoard.tsx:627-700` | `aria-label` com o título da tarefa | P |
| 4.6 | `confirm()` nativo | `TasksBoard.tsx:540`, `NotesBoard.tsx`, `TicketModal.tsx:75` | Diálogo de confirmação sobre o `Modal` | P |
| 4.7 | Enter não envia o login do Mastersys | `MastersysPanel.tsx:304-344` | Envolver em `<form onSubmit>` | P |
| 4.8 | Card de nota não sincroniza edição externa | `NoteCard.tsx:27-28` | Resincronizar `title`/`content` do prop quando não estiver editando | P |
| 4.9 | Barras de rolagem invisíveis / ações só no hover | `styles.css:198-212`, `765-771` | `scrollbar-width: thin`; ações visíveis com `:focus-within` e em `pointer: coarse` | P |

**Atenção 4.3:** mensagens de validação são testadas por `matches!` no tipo,
não pelo texto — traduzir não deve quebrar testes, mas conferir os que
comparam string.

---

## Fase 5 — Segurança e fronteiras *(M, com decisões)*

| # | Item | Fazer | Esf. | Decisão do DEV? |
|---|---|---|---|---|
| 5.1 | Login local só protege a interface: comandos de dados não conferem sessão; pop-outs seguem editando após "Sair" | Guard `require_session(&state)?` nos comandos de nota/tarefa/Mastersys; no logout, fechar janelas `note-*`/`task-*` | M | **Sim** — define se a conta local é barreira real ou só separação de perfis (ADR-005). Com "manter conectado" o guard fica transparente para quem marcou |
| 5.2 | Pop-outs têm as mesmas permissões da `main` | Capability separada para `note-*`/`task-*` (drag, close, always-on-top da própria janela); comandos `mastersys_*` só na `main` via `AppManifest::commands` em `build.rs` | M | Não |
| 5.3 | `style-src 'unsafe-inline'` na CSP | Testar remoção em build de produção (React aplica `style={{}}` via CSSOM; confirmar na documentação do Tauri/WebView2 antes); manter se quebrar e documentar em ADR-009 | P | Não |
| 5.4 | Fallback do cliente HTTP sem timeout | `mastersys_provider.rs:111-117` (`unwrap_or_default`): propagar erro ou recriar com timeout; `connect_timeout(5s)`; política de redirect só mesma origem — **verificar antes** se o reqwest 0.13 já remove `Authorization` em redirect entre hosts | P | Não |
| 5.5 | `user_id` do servidor interpolado sem encoding | `mastersys_provider.rs:348, 581`: usar o `urlencode` que já existe (~637) | P | Não |
| 5.6 | `open_note_window` não valida o id como UUID | Mesmo caminho de `open_task_window`; `clamp_window_size` também em `set_note_window_size` | P | Não |
| 5.7 | Logs de debug restantes | `console.log` de init no `initialization_script` (`commands.rs`) e no `NoteWindowApp` | P | Não |

---

## Fase 6 — Personalização (CLAUDE §9) *(M)*

- Paleta de notas fixa (`NoteCard.tsx:6-9`) → configuração do usuário, com os
  valores atuais como padrão.
- Limite de "vence em breve" fixo em 30 min (`TasksBoard.tsx:567`) →
  configuração, reaproveitando os limiares de lembrete.
- Estilos inline repetidos (`AuthPanel`, `TasksBoard`, `NoteCard`, `App`) →
  classes/tokens (`.md-label` etc.) para o tema cobrir tudo.
- Opcional de ADR-011: "iniciar minimizado na bandeja" quando aberto pelo
  Windows (argumento `--autostart` no `tauri_plugin_autostart::init`).

---

## Resumo de ordem e esforço

| Fase | Tema | Esforço | Bloqueio |
|---|---|---|---|
| 0 | Validar entrega anterior | P | Instalar C++ Build Tools |
| 1 | Robustez da sincronização | M | 1.2 muda port → DEV; 1.3.4 precisa do formato do evento |
| 2 | Lembretes reais | G | Pesquisa + ADR-004 |
| 3 | Desempenho da UI | M | — |
| 4 | Acessibilidade e mensagens | M | — |
| 5 | Segurança e fronteiras | M | 5.1 → DEV |
| 6 | Personalização | M | — |

Fases 3, 4 e 6 são independentes entre si e podem correr em paralelo por
pessoas/agentes diferentes; 1 e 2 mexem em `mastersys.rs`/`notification_service.rs`
e devem ser sequenciais.
