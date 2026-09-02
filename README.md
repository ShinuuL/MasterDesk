# MasterDesk

Notas e tarefas de desktop, extensível, com integração opcional ao Mastersys
Suporte. Rust + Tauri 2 + TypeScript/React, SQLite via sqlx.

> **Estado atual:** Fases 1–5 concluídas. O quadro de notas e tarefas funciona
> ponta a ponta, com anotações dentro de tarefas, tema claro/escuro/automático e
> sincronização somente-leitura com o Mastersys. Detalhe por fase — incluindo o
> que ainda **não** foi validado fora do Windows — no [ROADMAP](./ROADMAP.md).

## Comandos

**Pré-requisito no Linux:** `libdbus-1-dev` e `pkg-config`. A feature
`sync-secret-service` do `keyring` linka com o D-Bus do sistema via
`libdbus-sys`; sem o dev package o build falha na compilação, não em runtime.
Windows e macOS não precisam de nada além do toolchain.

```bash
# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace              # 123 testes

# Frontend
npm --prefix frontend test          # 66 testes (vitest)
npm --prefix frontend run build

# App
cargo tauri dev
```

## Funcionalidades

| | Onde | Documentação |
|---|---|---|
| Notas (sticky notes, pop-out, always-on-top) | `frontend/src/components/NoteCard.tsx` | ADR-002, ADR-003 |
| Tarefas com prazo e lembretes | `crates/application/src/tasks.rs` | ADR-004 |
| **Anotações dentro de tarefas** | `crates/domain/src/task_notes.rs` | ROADMAP Fase 5.1 |
| **Tema claro/escuro/automático** | `frontend/src/theme/` | [ADR-009](./ADR/ADR-009-theming.md) |
| **Integração Mastersys (somente leitura)** | `crates/infrastructure/src/mastersys_provider.rs` | [ADR-006](./ADR/ADR-006-mastersys-integration.md), [guia](./docs/INTEGRACAO_MASTERSYS.md) |
| Autenticação local | `crates/infrastructure/src/local_auth_repository.rs` | ADR-005 |

## Dependências relevantes (CLAUDE §16)

| Crate/pacote | Versão | Para quê | Licença |
|---|---|---|---|
| `tauri` | 2 | janela, tray, empacotamento | MIT/Apache-2.0 |
| `sqlx` | 0.8 | SQLite; migrations **embutidas** via `migrate!` | MIT/Apache-2.0 |
| `argon2` | 0.5 | hash de senha local (nunca plaintext) | MIT/Apache-2.0 |
| `reqwest` | 0.13 | HTTP para a API do Mastersys; TLS `rustls` + loja do SO, sem OpenSSL | MIT/Apache-2.0 |
| `keyring` | 3.6 | cofre nativo do SO para o refresh token (3.x de propósito — ver ADR-006) | MIT/Apache-2.0 |
| `react` | 18 | UI | MIT |
| `vitest` | 3 | testes do frontend (dev) | MIT |

## Segurança em uma linha

Senha nunca é persistida. Refresh token vai para o Credential Manager /
Keychain / Secret Service, nunca para o SQLite — `app_settings` é só para
configuração não sensível, porque o banco fica sem criptografia no diretório do
usuário. Nenhum comando Tauri devolve token ao frontend.

---

# Histórico — scaffold da Fase 1

> A seção abaixo é o README original do scaffold, preservado como registro.
> Partes dela já não descrevem o estado atual do repositório.


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
