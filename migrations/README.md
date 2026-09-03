# Migrations

Aplicadas por `sqlx::migrate!("../migrations")` em `src-tauri/src/lib.rs`, com o
SQL **embutido no binário** em tempo de compilação (não são lidas do disco em
runtime, então não precisam ser empacotadas no instalador).

## ⚠️ Nunca edite uma migration já aplicada — nem os comentários

O `sqlx` guarda um **checksum do conteúdo do arquivo** em `_sqlx_migrations`.
Na próxima abertura ele compara, e qualquer diferença faz `.run()` falhar com
`VersionMismatch`. Como `lib.rs` usa `.expect(...)`, o app **entra em pânico no
boot** — não degrada, não avisa, simplesmente não abre.

O checksum cobre o arquivo inteiro. Trocar uma palavra num comentário, corrigir
um acento ou reindentar é suficiente para quebrar.

Isto aconteceu de verdade em 2026-09-03: um rename de produto trocou
"MasterDesk" por "MasterNote" num comentário de `0008`, e só não virou um app
que não abre porque foi revertido antes de rodar.

**Regra prática:** arquivo em `migrations/` que já rodou em qualquer máquina é
imutável. Precisa mudar algo? Crie a migration seguinte.

### Como saber se já foi aplicada

Se existe em `_sqlx_migrations` de algum banco — inclusive o seu, em
`%APPDATA%/<identifier>/masterdesk.db` — já foi. Na prática: se você rodou o app
depois de criar o arquivo, considere imutável.

## Ordem atual

| Arquivo | O que introduz |
|---|---|
| `0001_init.sql` | Base do schema |
| `0002_notes.sql` | `notes`, com posição/tamanho/opacidade/always-on-top |
| `0003_tasks.sql` | `tasks` |
| `0004_auth.sql` | `users` (autenticação local) |
| `0005_task_notes_and_external.sql` | `task_notes` + colunas `external_*` em `tasks` |
| `0006_app_settings.sql` | `app_settings` (configuração não sensível) |
| `0007_mastersys_status_catalog.sql` | Catálogo de status espelhado do Mastersys |
| `0008_external_status_parked.sql` | `tasks.external_status_parked` |
| `0009_task_window_state.sql` | Geometria do pop-out de tarefa |

## Nomes que não podem mudar

`masterdesk.db` e o `identifier` (`com.masterdesk.app`) definem onde o banco
vive. Ver o comentário longo em `src-tauri/src/lib.rs`, no ponto em que
`app_data_dir()` é resolvido.
