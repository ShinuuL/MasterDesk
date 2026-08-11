# ADR-005 — Autenticação

**Status:** Não iniciado (deliberadamente)

## Por que este ADR está vazio por enquanto

Conforme a seção 24 do CLAUDE.md ("Development Order"), a autenticação é Fase 4,
posterior a Foundation, Local Notes e Tasks/Deadlines. Além disso, o próprio
CLAUDE.md (seção 11) e o artifacts.md (seção 8) proíbem assumir o mecanismo de
autenticação do futuro Mastersys sem validação.

## O que precisa acontecer antes de preencher este ADR

- Fases 1–3 concluídas (Foundation, Notes, Tasks/Deadlines).
- Definição do mecanismo de `AuthenticationProvider` local/dev (não depende de
  Mastersys) — pode ser pesquisado antes, mas a decisão de armazenamento seguro de
  sessão deve considerar a lib de keyring do SO escolhida (a pesquisar quando esta
  fase começar).
- Informação oficial do DEV sobre o mecanismo real de autenticação do Mastersys
  (endpoint, tipo de token, fluxo — nada disso pode ser inventado, seção 11 do
  CLAUDE.md).

## Ação

Reabrir este ADR no início da Fase 4.
