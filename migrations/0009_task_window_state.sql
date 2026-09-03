-- Geometria da janela destacada (pop-out) de uma tarefa.
--
-- POR QUE TABELA SEPARADA, E NÃO COLUNAS EM `tasks`
--
-- As notas guardam `position_x/y`, `size_w/h` e `always_on_top` na própria
-- linha de `notes` (migration 0002), e seria tentador copiar. Mas o caso é
-- diferente:
--
-- * Para uma **nota**, estar em algum lugar da tela faz parte do que ela é —
--   é um post-it. Posição é atributo do domínio.
-- * Para uma **tarefa**, a janela é acidental: ela é um item de trabalho que
--   pode, opcionalmente, ser destacado. Posição de janela é detalhe de UI, e
--   o CLAUDE.md §4 manda o domínio não depender de detalhe de UI.
--
-- Na prática isso também evita esticar `Task::reconstitute` (que já carrega
-- `#[allow(clippy::too_many_arguments)]`) com cinco parâmetros de apresentação
-- e mexer em todos os seus call sites.
--
-- CASCADE de propósito: tarefa apagada não deixa geometria órfã. Vale
-- especialmente para espelhos do Mastersys, que o `retire_mirror` apaga quando
-- saem da fila do usuário.

CREATE TABLE IF NOT EXISTS task_window_state (
    task_id       TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    position_x    REAL NOT NULL DEFAULT 120.0,
    position_y    REAL NOT NULL DEFAULT 120.0,
    size_w        REAL NOT NULL DEFAULT 380.0 CHECK (size_w >= 180 AND size_w <= 4096),
    size_h        REAL NOT NULL DEFAULT 300.0 CHECK (size_h >= 140 AND size_h <= 4096),
    always_on_top INTEGER NOT NULL DEFAULT 1 CHECK (always_on_top IN (0,1)),
    updated_at    TEXT NOT NULL              -- ISO8601 UTC
);
