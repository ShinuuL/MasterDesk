-- Catálogo de status de chamado do Mastersys, espelhado localmente.
--
-- POR QUE ESPELHAR EM VEZ DE HARDCODAR
-- O Mastersys permite cadastrar status próprios em Configurações
-- (`ticket_statuses`, com `TicketStatus = (string & {}) | …` aceitando qualquer
-- string). Uma lista fixa aqui ficaria desatualizada em silêncio: um status
-- novo apareceria sem rótulo, sem cor e — pior — fora do filtro padrão.
--
-- Fonte: `GET /api/ticket-statuses` (exige apenas autenticação, sem permissão
-- especial). Atualizado a cada sincronização.
--
-- É dado de REFERÊNCIA, não de trabalho: pode ser apagado e reconstruído a
-- qualquer momento pela próxima sincronização. Nada aqui é insumo do usuário.

CREATE TABLE IF NOT EXISTS mastersys_status_catalog (
    -- Slug do status na origem (ex. `pos_atendimento`). É a chave de junção com
    -- `tasks.external_status`.
    value          TEXT PRIMARY KEY,
    -- Rótulo em pt-BR já formatado pela origem ("Pós Atendimento"), em vez do
    -- slug humanizado à força no frontend.
    label          TEXT NOT NULL,
    -- Cor hex definida no cadastro do Mastersys. NÃO é usada crua: passa pelo
    -- tone-mapping de `noteSurface()` para respeitar o piso de contraste da
    -- ADR-009 nos dois temas.
    color          TEXT NOT NULL,
    -- `default_filter` da origem: vem pré-marcado no filtro de status.
    -- É 0 exatamente para `finalizado`, `cancelado` e `pos_atendimento`.
    default_filter INTEGER NOT NULL DEFAULT 1 CHECK (default_filter IN (0,1)),
    -- Status terminal na origem (`finalizado`, `cancelado`).
    is_final       INTEGER NOT NULL DEFAULT 0 CHECK (is_final IN (0,1)),
    -- Congela a contagem de SLA na origem. Opt-in do admin, então costuma ser 0
    -- mesmo em status parados — não dá para depender só dele.
    pauses_sla     INTEGER NOT NULL DEFAULT 0 CHECK (pauses_sla IN (0,1)),
    display_order  INTEGER NOT NULL DEFAULT 0,
    updated_at     TEXT NOT NULL              -- ISO8601 UTC
);

-- A UI lista o catálogo na ordem da origem, para o filtro sair na mesma sequência
-- que o atendente já conhece do suporte.
CREATE INDEX IF NOT EXISTS idx_status_catalog_order
    ON mastersys_status_catalog (display_order, value);
