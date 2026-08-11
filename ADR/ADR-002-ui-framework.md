# ADR-002 — UI / Window Management Framework

**Status:** Confirmado — React escolhido como frontend (validado em 2026-08-11).
**Data:** 2026-08-11

## Contexto

MasterDesk precisa de: janelas always-on-top configuráveis, customização visual
profunda (tema, opacidade, bordas, fontes), boa DX para um frontend rico, e um
caminho plausível para mobile (visualização/acesso). As opções obrigatórias de
pesquisa eram Tauri, Slint, egui e Iced.

## Opções consideradas

### Tauri 2
- Stable release atual: **v2.10.1** (mar/2026), licença MIT/Apache-2.0.
- Suporta oficialmente Linux, macOS, Windows, Android e iOS, com o suporte mobile
  estabilizado desde a versão estável de outubro de 2024 (Tauri 2.0.0).
- Frontend em TypeScript/HTML/CSS (React, Vue, Svelte, etc.), backend em Rust —
  exatamente a divisão UI/Domain que o CLAUDE.md pede.
- Binários pequenos e footprint de memória bem menor que Electron, por não
  empacotar Chromium/Node.
- **Limitação confirmada (não hipotética):** `always_on_top` tem bugs conhecidos
  em Linux/Wayland, onde a chamada pode falhar silenciosamente (issues abertas no
  repositório oficial). `visible_on_all_workspaces` não é suportado no Windows.
  Em macOS, há relatos de janelas always-on-top que não ficam acima de apps em
  fullscreen. Isso deve ser documentado como limitação conhecida (seção 6 do
  artifacts.md exige isso), não contornado com workaround não confiável.
- Plugins oficiais cobrem notificação e SQL (ver ADR-003/ADR-004).

### Slint
- Licenciamento triplo: GPLv3 (open source), **Royalty-free** (uso comercial em
  desktop/mobile/web sem custo) e comercial pago — não exige custo para o nosso
  caso de uso proprietário se optarmos pela licença royalty-free.
- Runtime muito leve (<300 KiB RAM), DSL declarativa própria (.slint), API estável
  desde a 1.x.
- Suporte mobile: conforme a documentação/roadmap do próprio projeto, o suporte a
  Android/iOS ainda está em estágio inicial/"a fazer" em partes do ecossistema —
  menos maduro que o mobile do Tauri, que já está estável desde 2024.
- Exigiria aprender uma DSL própria em vez de reaproveitar conhecimento web/TS.

### egui / Iced
- Ambos são GUI Rust "puros" (sem webview): egui é immediate-mode (rápido para
  prototipar, mas menos adequado a UIs de formulário/tema ricas e configuráveis);
  Iced é mais estruturado (inspirado em Elm) mas com lacunas de documentação.
- Nenhum dos dois tem uma solução mobile pronta e madura equivalente à do Tauri.
- Customização visual profunda (temas, sombras, bordas arredondadas) é mais
  trabalhosa de implementar do que em um frontend web com CSS.

## Decisão

Adotar **Tauri 2** para janelas/empacotamento desktop, com frontend em
**TypeScript + React** (decisão validada na Fase 1, protótipo Svelte descartado).

Manter **Slint como alternativa documentada** caso, durante a Fase 2, os bugs de
always-on-top no Linux/Wayland do Tauri se mostrem bloqueantes — nesse caso,
reavaliar via nova ADR antes de trocar de framework.

## Consequências

- Habilita reaproveitamento de UI web (CSS/temas) para a customização exigida na
  seção 7/9 do artifacts.md.
- As limitações de always-on-top em Wayland/macOS fullscreen/Windows workspaces
  devem ser testadas manualmente em cada OS suportado (Rule 19/25 do CLAUDE.md:
  compilar não é validar) e documentadas na Fase 2, não assumidas como resolvidas.
- Mobile fica desbloqueado nativamente pelo mesmo framework (ver ADR-008).

## Custo de reversão

Médio-alto. Trocar de Tauri para Slint depois da Fase 2 exigiria reescrever toda a
camada de UI (mas não o domínio Rust, que é framework-agnóstico).

## Nota dos Devs:

- Decisão aceita: Tauri 2 + Slint como alternativa para o always-on-top.
- **Frontend confirmado: React** (protótipo Svelte descartado em 2026-08-11).