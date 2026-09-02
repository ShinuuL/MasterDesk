-- Fase 3 — Schema de tasks (seção 6 e 8 do CLAUDE.md)
-- SQLite via sqlx. Domínio permanece agnóstico.

CREATE TABLE IF NOT EXISTS tasks (
    id                      TEXT PRIMARY KEY,          -- UUID v4
    title                   TEXT NOT NULL CHECK (length(trim(title)) > 0 AND length(title) <= 200),
    description             TEXT NOT NULL DEFAULT '' CHECK (length(description) <= 20000),
    priority                TEXT NOT NULL DEFAULT 'Medium' CHECK (priority IN ('Low','Medium','High','Urgent')),
    deadline                TEXT,                      -- ISO8601 UTC nullable
    reminder_thresholds     TEXT NOT NULL DEFAULT '[]',-- JSON array de ReminderThreshold
    completed               INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0,1)),
    created_at              TEXT NOT NULL,             -- ISO8601 UTC
    updated_at              TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_completed ON tasks(completed);
CREATE INDEX IF NOT EXISTS idx_tasks_deadline ON tasks(deadline);
