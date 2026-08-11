-- Migration de teste da Fase 1 — só valida que o pipeline de migrations
-- funciona. O schema real de `notes`/`tasks` entra nas Fases 2 e 3.
CREATE TABLE IF NOT EXISTS _masterdesk_migration_check (
    id INTEGER PRIMARY KEY,
    checked_at TEXT NOT NULL DEFAULT (datetime('now'))
);
