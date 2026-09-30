-- Coluna do quadro Kanban (A fazer / Em andamento / Aguardando).
--
-- Estritamente LOCAL (decisão do DEV em 2026-09-30): mover um card não
-- escreve nada no Mastersys, e a sincronização não altera esta coluna.
-- "Concluído" continua sendo `completed = 1`, que já existe.
--
-- Default 'todo' para as linhas existentes. Item parado na origem não muda
-- de coluna sozinho (ver `frontend/src/tasks/board.ts`).

ALTER TABLE tasks ADD COLUMN board_column TEXT NOT NULL DEFAULT 'todo'
    CHECK (board_column IN ('todo', 'doing', 'waiting'));
