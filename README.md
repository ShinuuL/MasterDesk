# MasterDesk — Scaffold (Fase 1: Foundation)

Estrutura gerada a partir de ADR-001, 002, 003, 004 e 08 (`ADR/`) e do
`ROADMAP.md`.

```text
├── Cargo.toml                  # workspace Rust
├── crates/
│   ├── domain/                 # regras de negócio puras — zero I/O
│   ├── application/            # casos de uso, orquestra os ports do domínio
│   └── infrastructure/         # implementações concretas (sqlx, plugins Tauri)
├── src-tauri/                  # app Tauri (janela, bootstrap, comandos)
│   └── icons/                  # ícones do app (icon.png, icon-mobile.png, icon-app.png)
├── frontend/                   # React + Vite + TypeScript
├── migrations/                 # migrations SQL (sqlx / tauri-plugin-sql)
└── .github/workflows/ci.yml    # cargo fmt/clippy/check/test + build do frontend
```

## O que este scaffold prova

- Direção de dependência correta: `infrastructure → application → domain`,
  nunca o inverso (seção 4 do CLAUDE.md).
- `domain` não importa `tauri`, `sqlx` nem nada de infraestrutura — só
  `serde`, `thiserror`, `uuid`, `chrono`, `async-trait`.
- Todos os ports citados na seção 4 do CLAUDE.md existem como traits vazias:
  `NoteRepository`, `TaskRepository`, `NotificationService`, `WindowService`,
  `AuthenticationProvider`, `SupportSystemProvider`, `AIProvider`.
- CI cobre fmt/clippy/check/test do Rust e build do frontend.
- Ícones do app em `src-tauri/icons/`.

## O que NÃO está aqui (de propósito)

- Nenhuma implementação real de repositório (isso é Fase 2, ADR-003).
- Nenhuma UI de nota/task real (Fase 2/3).
- `tauri-plugin-sql` e `tauri-plugin-notification` estão comentados no
  `Cargo.toml`/`lib.rs` do `src-tauri` — entram nas Fases 2 e 3.

## Status dos ADRs

- **ADR-002** — ✅ React confirmado como frontend (protótipo Svelte descartado)
- **ADR-004** — ✅ Plugin oficial escolhido, addendum criado em `ADR/ADR-004-addendum.md`

## Limitação deste ambiente

Este container não tem toolchain Rust nem os pacotes de sistema do
webview (`libwebkit2gtk`, etc.), então os arquivos `.rs` foram escritos e
revisados manualmente, mas **não foram compilados aqui**. Antes do primeiro
PR, rodar localmente (ou no CI):

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
cargo test --workspace
```

## Fase 1 Completa ✅

Ver `PHASE1-SUMMARY.md` para documentação completa do que foi feito.
