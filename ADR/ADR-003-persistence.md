# ADR-003 — Persistência Local

**Status:** Confirmado — Decisão aceita pelos Devs será **SQLite via `sqlx`**.
**Data:** 2026-08-11

## Contexto

O core precisa funcionar 100% offline (seção 22 do artifacts.md), persistindo
Notes, Tasks, Settings, configuração de lembretes, estado de janela e metadados de
integração. A seção 21 sugere SQLite como candidato, mas não pré-aprovado.

## Opções consideradas

### SQLite via `rusqlite`
- Wrapper síncrono e leve sobre SQLite, API ergonômica, muito usado em apps Tauri.
- Sem overhead de runtime assíncrono quando não necessário.

### SQLite via `sqlx`
- Toolkit assíncrono, com migrations integradas e verificação de queries em tempo
  de compilação (quando usado com banco disponível no build).
- Existe plugin oficial do Tauri (`tauri-plugin-sql`) que já usa `sqlx` por baixo
  dos panos e expõe migrations como parte do ciclo de vida do app.
- Requer runtime async (tokio), que o Tauri já traz por padrão — sem custo extra
  relevante.

### Diesel / SeaORM (ORMs completos)
- Mais poder de abstração, mas adicionam complexidade (Diesel: geração de schema
  em build time; SeaORM: mais "peso" de ORM) desnecessária para o volume de dados
  local de um app de notas/tasks.

## Decisão

Adotar **SQLite** como motor de persistência, acessado via **`sqlx`**, aproveitando
o plugin oficial `tauri-plugin-sql` para reduzir código de integração e ganhar
migrations versionadas prontas (`migrations/*.sql`).

`NoteRepository` e `TaskRepository` (seção 20 do CLAUDE.md) serão implementados como
adapters de infraestrutura sobre esse pool `sqlx`, nunca expostos diretamente ao
domínio.

## Consequências

- Domínio permanece agnóstico ao SQLite (via trait `NoteRepository`/`TaskRepository`).
- Migrations versionadas facilitam evolução do schema (Notes, Tasks, Settings).
- Abre caminho natural para, no futuro, trocar SQLite por outro backend (ex.: um
  servidor de sync) sem tocar no domínio — apenas nova implementação do port.
- Precisa decidir localização do arquivo `.db` por OS (usar API de path do Tauri,
  `app_data_dir`) — a validar na implementação (Fase 1/2).

## Custo de reversão

Baixo-médio, graças ao isolamento via repository/port.
