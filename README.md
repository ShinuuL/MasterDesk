# MasterDesk — Scaffold (Fase 1: Foundation)

Estrutura gerada a partir de ADR-001, 002, 003, 004 e 008 (`docs/adr/`) e do
`docs/ROADMAP.md`.

```text
├── Cargo.toml                  # workspace Rust
├── crates/
│   ├── domain/                 # regras de negócio puras — zero I/O
│   ├── application/            # casos de uso, orquestra os ports do domínio
│   └── infrastructure/         # implementações concretas (sqlx, plugins Tauri)
├── src-tauri/                  # app Tauri (janela, bootstrap, comandos)
├── frontend/                   # protótipo React (ver pendência abaixo)
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

## O que NÃO está aqui (de propósito)

- Nenhuma implementação real de repositório (isso é Fase 2, ADR-003).
- Nenhuma UI de nota/task real (Fase 2/3 — e o item pendente do ADR-002
  abaixo bloqueia isso).
- Nenhum ícone de app real — `tauri.conf.json` referencia
  `icons/icon.png`, que **não existe ainda**. Gerar com `tauri icon` antes
  do primeiro build real.
- `tauri-plugin-sql` e `tauri-plugin-notification` estão comentados no
  `Cargo.toml`/`lib.rs` do `src-tauri` — entram nas Fases 2 e 3.

## ⚠️ Pendências herdadas dos ADRs (bloqueiam parte da Fase 1)

1. **ADR-002** — a nota do DEV pede pesquisa mais aprofundada e mock-ups
   antes de travar o frontend. Este scaffold só tem o protótipo **React**.
   Falta criar o protótipo **Svelte** equivalente e levar os dois para
   validação antes de qualquer UI de domínio real.
2. **ADR-004** — falta o adendo comparando `tauri-plugin-notification` puro
   vs. o fork comunitário, conforme pedido na nota do DEV.

## Limitação deste ambiente de scaffold

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

## Próximo passo sugerido

1. `git init`, primeiro commit deste scaffold na branch
   `feature/phase1-foundation`.
2. Rodar os comandos acima localmente para pegar qualquer erro de sintaxe
   Rust que este ambiente não conseguiu validar.
3. Resolver as duas pendências de ADR antes de iniciar UI de domínio real.
