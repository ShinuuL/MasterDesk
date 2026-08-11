# ADR-008 — Estratégia Mobile

**Status:** Confirmado — aguardando confirmação do DEV
**Dev:** Autorizado o uso do Tauri Mobile, a decisão atende os requisitos.
**Data:** 2026-08-11

## Contexto

Mobile é definido como "visualização/acesso", não paridade total com o desktop
(seção 14 do CLAUDE.md). As opções candidatas eram: framework de UI compartilhado,
app companion, web app responsivo, Tauri mobile ou frontend mobile separado
consumindo a mesma API.

## Opções consideradas

### Tauri Mobile (iOS/Android), reaproveitando o mesmo frontend do desktop
- Já decidido como framework desktop no ADR-002, então o suporte mobile vem "de
  graça" na mesma stack: mesmo backend Rust compilado como lib estática, mesmo
  frontend TypeScript, com shells nativos (Swift/Kotlin) gerados pela própria
  ferramenta (`tauri ios init` / `tauri android init`).
- Suporte mobile é estável desde a versão 2.0.0 estável (out/2024), mas nem todos
  os plugins de desktop têm paridade em mobile ainda — cada plugin usado
  (notificação, SQL, window) precisa ser validado individualmente em iOS/Android
  antes de assumir que funciona.
- Requer macOS + Xcode completo para builds/publicação iOS; Android requer
  toolchain Gradle/NDK — isso é custo de infraestrutura de CI a planejar.

### App companion nativo separado (Swift/Kotlin puro)
- Melhor integração nativa possível, mas duplica esforço de UI e quebra o
  princípio de "Shared Domain/API" da seção 14 — descartado para a fase inicial.

### Web app responsivo (PWA)
- Mais simples de entregar rápido (sem lojas de app), mas sem acesso a
  notificações push nativas robustas em iOS sem trabalho extra, e sem reaproveitar
  o binário Rust diretamente (precisaria expor a lógica via uma API HTTP local ou
  remota, que ainda não existe).
- Fica como opção de fallback/complementar, não como estratégia primária.

## Decisão

Adotar **Tauri Mobile (iOS/Android)** como estratégia primária, reaproveitando o
mesmo domínio Rust e frontend TypeScript do desktop, com o entendimento explícito
de que:

1. Mobile é client de "visualização/acesso" na Fase 7, não uma reimplementação de
   todas as features desktop (always-on-top, por exemplo, não existe em mobile).
2. Cada plugin (notificação, SQL, janela) será validado individualmente em
   iOS/Android antes de ser considerado disponível nessa plataforma.
3. Um app companion nativo ou PWA permanece como alternativa a reavaliar via nova
   ADR se o Tauri Mobile se mostrar insuficiente durante a Fase 7.

## Consequências

- Nenhuma decisão de stack adicional é necessária para habilitar mobile — reduz
  risco de retrabalho.
- CI precisa eventualmente rodar em macOS (para build iOS) — custo de
  infraestrutura a considerar apenas quando a Fase 7 começar, não agora.
- A Fase 7 só deve iniciar depois que o core desktop (Fases 1–3) estiver validado,
  conforme a ordem de desenvolvimento da seção 24 do CLAUDE.md.

## Custo de reversão

Médio. Como o domínio é compartilhado, trocar a camada mobile por PWA ou app nativo
no futuro não afeta o core — afeta apenas a camada de UI mobile.
