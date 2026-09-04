-- Papel do usuário no item da origem + vínculo manual com chamado.
--
-- PAPÉIS (`external_role_*`)
-- A fila de uma pessoa no suporte tem dois papéis, e até aqui o MasterDesk só
-- conhecia um. No Mastersys (verificado em `TicketRepository.ts`, filtro
-- `attendantId`):
--
--     assigned_to  → Analista Responsável
--     created_by   → Atendente (quem abriu/assumiu o chamado)
--
-- Até esta migração o provider só puxava `assignedTo=<eu>`, então chamado em
-- que a pessoa é atendente simplesmente não aparecia. Agora puxa os dois e
-- grava qual papel vale, para o card poder dizer.
--
-- Ambas as colunas em 0 significam "a origem não informou papel para este
-- item" — não "nenhum papel". É o estado de uma tarefa atribuída a mim cujo
-- chamado é de outra dupla, e nesse caso o app não afirma nada.
--
-- VÍNCULO MANUAL (`link_*`)
-- Tarefa local que aponta para um chamado, criada pelo usuário aqui dentro.
-- Deliberadamente **fora** das colunas `external_*`: aquelas marcam espelho, e
-- espelho que não volta na fila da origem é retirado pela sincronização. O
-- vínculo manual precisa do oposto — sobreviver a todo sync, porque o dono
-- dele é o usuário. Nada disso é escrito de volta no Mastersys; a integração
-- continua estritamente somente-leitura.
--
-- `link_status` é o status personalizado, texto livre. Sem CHECK de valores
-- porque não existe cadastro para validar contra: quem define o vocabulário é
-- quem digita.

ALTER TABLE tasks
    ADD COLUMN external_role_analyst INTEGER NOT NULL DEFAULT 0
        CHECK (external_role_analyst IN (0,1));

ALTER TABLE tasks
    ADD COLUMN external_role_attendant INTEGER NOT NULL DEFAULT 0
        CHECK (external_role_attendant IN (0,1));

ALTER TABLE tasks ADD COLUMN link_ticket TEXT
    CHECK (link_ticket IS NULL OR (length(trim(link_ticket)) > 0 AND length(link_ticket) <= 64));
ALTER TABLE tasks ADD COLUMN link_client TEXT
    CHECK (link_client IS NULL OR length(link_client) <= 200);
ALTER TABLE tasks ADD COLUMN link_status TEXT
    CHECK (link_status IS NULL OR length(link_status) <= 64);

-- "Quais tarefas apontam para o chamado X" — usado ao abrir o modal do chamado
-- e para não criar dois vínculos iguais sem o usuário perceber.
CREATE INDEX IF NOT EXISTS idx_tasks_link_ticket
    ON tasks(link_ticket)
    WHERE link_ticket IS NOT NULL;
