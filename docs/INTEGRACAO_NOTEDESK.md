# Integração MasterDesk ↔ NoteDesk

> Cliente isolado: `notedesk-integration-client.js` — sem dependências, Node.js 18+, API key apenas no servidor.

## 1. Visão geral

- **MasterDesk** mantém notas/tasks locais (SQLite `masterdesk.db`) e pode espelhar tasks para um sistema externo **NoteDesk** via `POST /api/v1/tasks/upsert`.
- O cliente `NoteDeskClient` encapsula autenticação (`X-NoteDesk-Api-Key`), timeout, abort e tratamento de erro tipado (`NoteDeskIntegrationError`).
- Arquitetura segue `CLAUDE.md §10` (Mastersys/NoteDesk como `SupportSystemProvider` externo) e `AGENTS.md` (domain nunca depende de http).

```
MasterDesk (Tauri+SQLite) ──> NoteDeskClient (Node) ──> NoteDesk API
     │  masterdesk.db               │  endpoint+apiKey          │  /health  /tasks/upsert
     └─ Task/Note ──────────────────┘                           └─ source_system
```

## 2. Configuração

Variáveis de ambiente **apenas no servidor** (nunca no bundle Tauri/frontend):

```env
NOTEDESK_ENDPOINT=https://notedesk.exemplo.com
NOTEDESK_API_KEY=sk_live_...        # nunca commitar
NOTEDESK_SOURCE_SYSTEM=masterdesk   # identifica origem no NoteDesk
```

Ou via construtor:

```js
const { NoteDeskClient } = require('./notedesk-integration-client.js');
const client = new NoteDeskClient({
  endpoint: process.env.NOTEDESK_ENDPOINT,
  apiKey: process.env.NOTEDESK_API_KEY,
  sourceSystem: 'masterdesk',
  timeoutMs: 10000, // opcional
});
```

## 3. Uso

### Health check

```js
await client.health(); // GET /api/v1/health -> {status:"ok", ...}
```

### Upsert de Task (espelhar tarefa local)

```js
// Task vinda de masterdesk-application (TaskService)
await client.upsertTask({
  external_task_id: task.id, // UUID da task local
  title: task.title,
  description: task.description,
  priority: task.priority, // Low/Medium/High/Urgent
  deadline: task.deadline, // ISO8601 ou null
  reminder_thresholds: task.reminder_thresholds,
  completed: task.completed,
  assigned_user: { id: user.id, name: user.username } // obrigatório
  // source_system é injetado automaticamente
});
```

Regras do cliente (validação local):
- `external_task_id` obrigatório
- `assigned_user.id` e `assigned_user.name` obrigatórios
- `source_system` sobrescrito para o valor do construtor

### Tratamento de erro

```js
const { NoteDeskIntegrationError } = require('./notedesk-integration-client.js');
try {
  await client.upsertTask(task);
} catch (e) {
  if (e instanceof NoteDeskIntegrationError) {
    console.error(e.message, e.status, e.response);
  }
}
```

## 4. Integração com MasterDesk (proposto)

- **Application layer:** novo `SupportSystemProvider` → `NoteDeskProvider` que usa `NoteDeskClient` (infrastructure). Métodos: `pushTask(task: Task)`, `is_configured(): bool`.
- **Tauri command:** `sync_task_to_notedesk(id)` chama `TaskService` + `NoteDeskProvider` após `task_repo.save`.
- **Frontend:** botão "Sincronizar com NoteDesk" em `TasksBoard.tsx` (opcional, após Fase 6 IA).
- **Segurança:** `NOTEDESK_API_KEY` via `MASTERDESK_NOTEDESK_API_KEY` env ou OS keychain (tauri-plugin-stronghold), nunca logada.

## 5. Como testar localmente (sem NoteDesk real)

### Mock com `npx` http-echo ou json-server

```bash
# 1) subir mock
npx json-server --watch mock-notedesk.json --port 3001 &
# mock-notedesk.json deve ter {"health":{"status":"ok"}}

# 2) testar cliente com timeout
node -e "
const {NoteDeskClient}=require('./notedesk-integration-client.js');
const c=new NoteDeskClient({endpoint:'http://localhost:3001', apiKey:'test', sourceSystem:'masterdesk'});
c.health().then(r=>console.log('health ok',r)).catch(e=>console.error('health fail',e.message));
"
```

Ou teste unitário simples (sem rede) — validação de argumentos já ocorre sem endpoint:

```bash
node -e "
const {NoteDeskClient}=require('./notedesk-integration-client.js');
try { new NoteDeskClient({endpoint:'', apiKey:'x', sourceSystem:'y'}) } catch(e){ console.log('endpoint obrigatório:', e.message) }
try { const c=new NoteDeskClient({endpoint:'http://x', apiKey:'x', sourceSystem:'masterdesk'}); c.upsertTask({}) } catch(e){ console.log('validação:', e.message) }
"
# esperado: endpoint é obrigatório / task.external_task_id é obrigatório
"
```

## 6. Status atual (2026-09-01)

- **Cliente** `notedesk-integration-client.js` **funcionando**: validado com `node -e` (validações de `endpoint/apiKey/sourceSystem` e `external_task_id/assigned_user`), sem dependências, com `AbortController` + `fetch` nativo Node 18+.
- **MasterDesk** ainda **não consome** o cliente (Fase 5 Mastersys/NoteDesk está 🔒 Bloqueada por ADR-006 até o DEV fornecer o contrato real da API). O esqueleto `SupportSystemProvider` existe como trait vazia (`crates/domain/src/ports.rs`), e a Fase 4 (auth local) foi concluída sem acoplar domínio ao HTTP.
- **Próximo passo para ativar:** definir `NOTEDESK_ENDPOINT` real + implementar `NoteDeskProvider` em `crates/infrastructure` e expor `sync_task` no `src-tauri/src/commands.rs`. O documento acima já descreve o contrato esperado (`POST /api/v1/tasks/upsert` com `X-NoteDesk-Api-Key` e `source_system`).

## 7. Checklist antes de produção

- [ ] `NOTEDESK_API_KEY` em vault/keychain, não em `.env` commitado
- [ ] `endpoint` com `https` e certificado válido
- [ ] Teste `health` em staging antes de `upsertTask`
- [ ] Rate-limit / retry com backoff no `NoteDeskClient.request` (já tem timeout/abort)
- [ ] Log sem vazar `apiKey` (cliente atual não loga headers)

---
Gerado para MasterDesk `feature/phase2-3-notes-tasks` — cliente em `notedesk-integration-client.js` (79 linhas, zero deps).
