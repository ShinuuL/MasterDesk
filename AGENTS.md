# AGENTS.md — MasterDesk Development Instructions

## Project

Desktop-first notes/tasks app (Tauri 2 + Rust + TypeScript). Integrates with Mastersys support system eventually.

## Critical Rules

1. **Never guess** — if unsure about APIs, OS behavior, library capabilities, or security, **ask the DEV**.
2. **Research before implementing** — use Context7 MCP for library docs, verify maintenance/licensing, document decisions in ADRs.

## Tech Stack (Confirmed)

- **Backend:** Rust (domain/application/infrastructure crates)
- **Frontend:** TypeScript + React (pending final validation with Svelte — ADR-002)
- **Desktop:** Tauri 2
- **Persistence:** SQLite via `sqlx` + `tauri-plugin-sql` (ADR-003)
- **Notifications:** TBD — research pending (ADR-004)

## Project Structure

```
├── Cargo.toml              # Workspace root
├── crates/
│   ├── domain/             # Pure business logic — NO tauri/sqlx/reqwest deps
│   ├── application/        # Use cases, orchestrates domain ports
│   └── infrastructure/     # Concrete implementations (sqlx, Tauri plugins)
├── src-tauri/              # Tauri app (window, bootstrap, commands)
├── frontend/               # React prototype (Vite)
├── migrations/             # SQL migrations (sqlx)
```

## Architecture

Layered (section 4 of CLAUDE.md):
```
UI → Application → Domain → Interfaces/Ports → Infrastructure
```

**Domain crate must NEVER depend on:** tauri, sqlx, reqwest, or any infra/UI.

Ports exist as empty traits: `NoteRepository`, `TaskRepository`, `NotificationService`, `WindowService`, `AuthenticationProvider`, `SupportSystemProvider`, `AIProvider`.

## Build & Verification Commands

```bash
# Rust checks (from project root)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo test --workspace

# Frontend (from frontend/)
npm run build
```

**Note:** App icon not yet generated. Run `tauri icon` before first real build.

## Current State

- **Branch:** `feature/phase1-foundation`
- **Phase 0:** ✅ Complete (ADRs 001-004, 008 accepted)
- **Phase 1:** In progress (scaffold exists, not yet committed)
- **Blocked:** Phases 4-6 (auth/Mastersys/AI) await API contracts from DEV

## Pending from ADRs

1. **ADR-002:** Create Svelte prototype alongside React, bring both for DEV validation before locking frontend choice.
2. **ADR-004:** Research `tauri-plugin-notification` vs community fork — must complete before Phase 3.

## Key Constraints

- Always-on-top has known bugs on Linux/Wayland and macOS fullscreen — test manually per OS, don't assume.
- Mastersys integration is external — never couple domain to it.
- AI is advisory only — never auto-execute side effects.
- Mobile (Phase 7) is visualization/access only, not feature parity.

## Escalation Format

When uncertain, use:
```
What is known: ...
What is unknown: ...
Why it matters: ...
Options: ...
Recommendation: ...
Decision required: ...
```

## PR Integration Rule

Before creating a PR: fetch main, rebase, format, lint, test, build, verify no unrelated files changed.

## Reference

Full details in `CLAUDE.md` and `ADR/` directory. This file focuses on what agents miss most often.
