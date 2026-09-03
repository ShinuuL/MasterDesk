-- "Item parado na origem" — o estado que faz um chamado parar de mentir que
-- está atrasado.
--
-- O CASO CONCRETO
-- Um chamado em `pos_atendimento` continuava aparecendo como atrasado no quadro
-- e disparando lembrete, porque o MasterDesk só olhava `deadline <= agora`.
-- Pós-atendimento é justamente um estado de espera: o prazo original não diz
-- mais nada sobre urgência.
--
-- POR QUE O NOME É GENÉRICO
-- `status_parked` e não `pos_atendimento`: o domínio não pode conhecer o
-- vocabulário do Mastersys (CLAUDE §4 — separação de camadas). Quem traduz
-- status da origem para esta flag é o adapter, em `mastersys_provider.rs`.
--
-- COMO É DERIVADO (ver `MastersysStatus::is_parked`)
--     is_final || pauses_sla || !default_filter
-- O termo que de fato pega `pos_atendimento` é `!default_filter`, porque
-- `pauses_sla` é opt-in do admin e nasce desligado em todos os status. Isso
-- estica um pouco a semântica de `default_filter` ("vem pré-marcado no
-- filtro"), e é assumido de propósito: é o único sinal determinístico que a
-- origem oferece para esse estado.

ALTER TABLE tasks
    ADD COLUMN external_status_parked INTEGER NOT NULL DEFAULT 0
        CHECK (external_status_parked IN (0,1));
