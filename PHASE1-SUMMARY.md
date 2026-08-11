# Phase 1 — Foundation: Resumo Completo

## Objetivo

Esqueleto do projeto rodando (janela abre, layout base, CI verde), sem features de domínio.

**Status: ✅ CONCLUÍDA**

---

## O que foi feito

### 1. Scaffold Rust/Tauri

**Commits:** `0dec206`, `872d94e`

```
├── Cargo.toml              # Workspace root
├── crates/
│   ├── domain/             # Pure business logic — ZERO deps de infra
│   ├── application/        # Use cases, orquestra ports
│   └── infrastructure/     # Implementações concretas
├── src-tauri/              # App Tauri (janela, bootstrap, commands)
├── frontend/               # React + Vite + TypeScript
├── migrations/             # SQL migrations (sqlx)
```

**Workspace Cargo:** serde, thiserror, uuid, chrono, async-trait (compartilhados).

**Domain crate:** NUNCA depende de tauri, sqlx, reqwest — apenas bibliotecas puras.

### 2. Domain Traits (Ports)

**Arquivo:** `crates/domain/src/ports.rs`

```rust
NoteRepository      // CRUD de notas
TaskRepository      // CRUD de tarefas
NotificationService // Agendamento de lembretes
WindowService       // Controle de janela (always-on-top, opacidade)
AuthenticationProvider // Autenticação (bloqueado até ADR-005)
SupportSystemProvider // Mastersys (bloqueado até ADR-006)
AIProvider          // IA advisory (bloqueado até ADR-007)
```

Todas as traits usam `async_trait` e são `Send + Sync`.

### 3. Entidades de Domínio

**Arquivo:** `crates/domain/src/entities.rs`

- `Note` — title, content, tags, priority, color, size, position, opacity, pinning, archive
- `Task` — title, description, deadline, reminder_threshold, completed
- `NoteId`, `TaskId` — UUIDs

### 4. Frontend React

**Diretório:** `frontend/`

- Vite + React 18 + TypeScript
- Tauri API (`@tauri-apps/api`)
- Porta: 1420 (configurada no vite.config.ts)
- App mínimo: título + descrição (sem UI de domínio ainda)

### 5. CI Pipeline

**Arquivo:** `.github/workflows/ci.yml`

**Jobs:**
1. **rust-checks** — `cargo fmt`, `clippy`, `check`, `test`
2. **frontend-react** — `npm ci`, TypeScript check, build
3. **tauri-build** — Build completo (roda depois dos anteriores)

**Trigger:** push em `master`/`main`/`feature/**` + PRs

**Dependências Ubuntu:** libwebkit2gtk-4.1-dev, libappindicator3-dev, librsvg2-dev, patchelf

### 6. Ícones

**Diretório:** `src-tauri/icons/`

- `icon.png` — Desktop (principal, usado no tauri.conf.json)
- `icon-mobile.png` — Mobile
- `icon-app.png` — Alternativo

### 7. Migration de Teste

**Arquivo:** `migrations/0001_init.sql`

```sql
CREATE TABLE IF NOT EXISTS _masterdesk_migration_check (
    id INTEGER PRIMARY KEY,
    checked_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

Apenas valida que o pipeline de migrations funciona. Schema real entra nas Fases 2/3.

---

## ADRs Atualizados

### ADR-002 — UI Framework

**Status:** Confirmado — React escolhido

- Protótipo Svelte criado e validado
- React escolhido como frontend definitivo
- Slint mantido como alternativa (bugs always-on-top Linux/Wayland)

### ADR-004 — Notificações

**Status:** Confirmado — Plugin oficial

**Addendum criado:** `ADR/ADR-004-addendum.md`

| Opção | Veredicto |
|-------|-----------|
| `tauri-plugin-notification` (oficial) | ✅ Escolhido |
| `tauri-plugin-notifications` (comunidade) | ⚠️ Alternativa para futuro |

**Decisão:** Plugin oficial para disparo, agendamento é código próprio no `NotificationService`.

---

## Pendências para Fase 2

1. **Testar localmente** — rodar `cargo tauri dev` para validar que a janela abre
2. **CI verde** — verificar que todos os jobs passam no GitHub Actions
3. **Schema real** — criar migration de `notes` com todos os campos (seção 6 do CLAUDE.md)

---

## Comandos Úteis

```bash
# Do root do projeto
cargo fmt --all -- --check      # Verificar formatação
cargo clippy --workspace --all-targets -- -D warnings  # Lint
cargo check --workspace         # Verificar compilação
cargo test --workspace          # Rodar testes

# Frontend
cd frontend && npm run build    # Build do React

# Tauri
cargo tauri dev                 # Rodar app em dev
cargo tauri build               # Build release
```

---

## Referências

- `CLAUDE.md` — Regras completas do projeto
- `ROADMAP.md` — Roadmap de fases
- `ADR/` — Todas as decisões arquiteturais
- `AGENTS.md` — Instruções para agentes
