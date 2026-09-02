-- Anotações dentro de tarefas + vínculo com sistema de suporte externo.
--
-- `task_notes` é um agregado filho de `tasks`: ON DELETE CASCADE garante que
-- deletar a tarefa não deixa anotações órfãs (o repositório também apaga
-- explicitamente, porque SQLite só honra FK com `PRAGMA foreign_keys = ON`).
--
-- As colunas `external_*` em `tasks` marcam a origem do item (ADR-006). Uma
-- tarefa local tem todas nulas — o caso padrão. `status_label` guarda o status
-- cru da origem porque o Mastersys permite status customizados: mapear para um
-- enum fechado aqui inventaria valores (Regra 1 do CLAUDE.md).

CREATE TABLE IF NOT EXISTS task_notes (
    id          TEXT PRIMARY KEY,          -- UUID v4
    task_id     TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    content     TEXT NOT NULL CHECK (length(trim(content)) > 0 AND length(content) <= 20000),
    done        INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0,1)),
    created_at  TEXT NOT NULL,             -- ISO8601 UTC
    updated_at  TEXT NOT NULL
);

-- Listagem é sempre "anotações desta tarefa, mais antigas primeiro".
CREATE INDEX IF NOT EXISTS idx_task_notes_task_created
    ON task_notes(task_id, created_at);

ALTER TABLE tasks ADD COLUMN external_system TEXT;
ALTER TABLE tasks ADD COLUMN external_kind   TEXT;
ALTER TABLE tasks ADD COLUMN external_id     TEXT;
ALTER TABLE tasks ADD COLUMN external_client TEXT;
ALTER TABLE tasks ADD COLUMN external_ticket TEXT;
ALTER TABLE tasks ADD COLUMN external_status TEXT;

-- Um item da origem só pode ter um espelho local. O índice é parcial para não
-- colidir entre as tarefas locais (todas com external_id NULL).
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_external_unique
    ON tasks(external_system, external_id)
    WHERE external_id IS NOT NULL;

-- Varredura da sincronização: "todas as tarefas que vieram deste sistema".
CREATE INDEX IF NOT EXISTS idx_tasks_external_system
    ON tasks(external_system)
    WHERE external_system IS NOT NULL;
