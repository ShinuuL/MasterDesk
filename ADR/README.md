# MasterDesk — ADRs (Fase 0)

Branch: `research/phase0-stack-and-architecture`

| ADR | Tema | Status |
|---|---|---|
| [ADR-001](./ADR-001-desktop-framework.md) | Linguagem/runtime desktop | Proposto — aguarda DEV |
| [ADR-002](./ADR-002-ui-framework.md) | Framework de UI/janela | Proposto — aguarda DEV |
| [ADR-003](./ADR-003-persistence.md) | Persistência local | Proposto — aguarda DEV |
| [ADR-004](./ADR-004-notifications.md) | Notificações | Proposto — aguarda DEV |
| [ADR-005](./ADR-005-authentication.md) | Autenticação | Bloqueado até Fase 4 |
| [ADR-006](./ADR-006-mastersys-integration.md) | Integração Mastersys | **Aceito (2026-09-02)** — contrato validado no código do Mastersys |
| [ADR-007](./ADR-007-ai-provider.md) | Provider de IA | Bloqueado até Fase 6 |
| [ADR-008](./ADR-008-mobile-strategy.md) | Estratégia mobile | Proposto — aguarda DEV |
| [ADR-009](./ADR-009-theming.md) | Tema claro/escuro/automático | **Aceito (2026-09-02)** |

## Recomendação consolidada (ADR-001/002/003/004/008)

```text
Rust (core/domain/application)
  +
Tauri 2 (janela/empacotamento desktop + mobile)
  +
TypeScript + React (frontend/UI)
  +
SQLite via sqlx / tauri-plugin-sql (persistência)
  +
tauri-plugin-notification (toasts de sistema)
```

Isso confirma a hipótese original do artifacts.md (seção 14), mas agora com
pesquisa concreta por trás — incluindo uma limitação real e documentada que antes
não estava explícita: bugs conhecidos de `always_on_top` no Linux/Wayland e
inconsistências em macOS fullscreen/Windows workspaces, que precisam ser
validados manualmente na Fase 2 antes de declarar a feature "cross-platform".

**Nenhum destes ADRs deve ser tratado como decisão final.** Cada um está marcado
"Proposto" porque, pela seção 22/31 do CLAUDE.md, decisões arquiteturais com
consequências materialmente diferentes exigem confirmação explícita do DEV antes
da Fase 1 (Foundation).

## Próximo passo

1. DEV revisa e aprova (ou pede ajuste em) ADR-001, 002, 003, 004 e 008.
2. Assim que aprovados, muda-se o status para "Aceito" e inicia-se a Fase 1
   (Foundation) na branch principal de implementação.
3. ADR-007 permanece bloqueado até a Fase 6.

## Atualização — 2026-09-02

- **ADR-006 desbloqueado e aceito.** O bloqueio era falta do contrato real da
  API. O contrato foi lido do código-fonte do Mastersys
  (`alrindoMaster/gerenciador_relatorios_V3`) e cada endpoint/campo usado tem
  origem rastreável na tabela do próprio ADR — nada foi suposto (CLAUDE §10 e
  Regra 1). Decisão: o MasterDesk **consulta** a API em modo somente leitura.
- **ADR-009 criado e aceito.** Tema claro/escuro/automático. Documenta por que
  a detecção do tema do SO não pode depender só de `prefers-color-scheme` no
  Tauri, e a regra de tone-mapping das cores de nota.
