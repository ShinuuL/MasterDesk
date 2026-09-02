-- Fase 2 — Schema real de notes (seção 6 do CLAUDE.md + ADR-003)
-- SQLite via sqlx. Domínio permanece agnóstico — esta é a projeção de persistência.

CREATE TABLE IF NOT EXISTS notes (
    id              TEXT PRIMARY KEY,          -- UUID v4
    title           TEXT NOT NULL CHECK (length(trim(title)) > 0 AND length(title) <= 200),
    content         TEXT NOT NULL DEFAULT '' CHECK (length(content) <= 20000),
    tags            TEXT NOT NULL DEFAULT '[]',-- JSON array de strings
    priority        TEXT NOT NULL DEFAULT 'Medium' CHECK (priority IN ('Low','Medium','High','Urgent')),
    deadline        TEXT,                      -- ISO8601 UTC nullable
    color           TEXT NOT NULL DEFAULT '#FFEB3B',
    opacity         REAL NOT NULL DEFAULT 1.0 CHECK (opacity >= 0.1 AND opacity <= 1.0),
    pinned          INTEGER NOT NULL DEFAULT 0 CHECK (pinned IN (0,1)),
    always_on_top   INTEGER NOT NULL DEFAULT 0 CHECK (always_on_top IN (0,1)),
    archived        INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0,1)),
    position_x      REAL NOT NULL DEFAULT 100.0,
    position_y      REAL NOT NULL DEFAULT 100.0,
    size_w          REAL NOT NULL DEFAULT 300.0 CHECK (size_w >= 80 AND size_w <= 4000),
    size_h          REAL NOT NULL DEFAULT 250.0 CHECK (size_h >= 80 AND size_h <= 4000),
    created_at      TEXT NOT NULL,            -- ISO8601 UTC
    updated_at      TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_notes_archived ON notes(archived);
CREATE INDEX IF NOT EXISTS idx_notes_pinned ON notes(pinned);
CREATE INDEX IF NOT EXISTS idx_notes_updated_at ON notes(updated_at);
