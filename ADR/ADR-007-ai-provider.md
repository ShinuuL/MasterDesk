# ADR-007 — Arquitetura do Provider de IA

**Status:** Não iniciado (deliberadamente)

## Por que este ADR está vazio por enquanto

Conforme a seção 24 do CLAUDE.md, IA é Fase 6, e a seção 12 exige que a IA seja
apenas consultiva, nunca execute efeitos colaterais externos sem autorização
explícita futura. Escolher provider/SDK de IA agora seria antecipar uma decisão
sem o contexto de segurança e de dados que a Fase 5 (Mastersys) vai gerar.

## O que precisa acontecer antes de preencher este ADR

- Fases 1–5 concluídas (o AI precisa de contexto de task/ticket já modelado).
- Pesquisa (quando esta fase começar) de providers de IA compatíveis com Rust,
  política de dados/retensão de cada um, e custo.
- Definição de exatamente qual contexto mínimo autorizado é enviado à IA (seção
  10 do artifacts.md, passo 4: "AI receives the minimum authorized context").
- Modelo de `AIProvider` como port, nunca acoplado a um SDK específico
  diretamente no domínio.

## Ação

Reabrir este ADR no início da Fase 6.
