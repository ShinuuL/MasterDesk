# ADR-001 — Desktop Framework (Linguagem/Runtime)

**Status:** Confirmado — Confirmado no final da página.
**Data:** 2026-08-11

## Contexto

O CLAUDE.md/artifacts.md definem como candidatos iniciais: Rust, Python e TypeScript
(com Angular avaliado como framework de frontend, não linguagem). É necessário decidir
a linguagem/runtime que hospedará a camada `domain` + `application` do MasterDesk,
respeitando os requisitos de: desktop-first, always-on-top, notificações, persistência
local, extensibilidade para Mastersys/IA, e caminho futuro para mobile.

## Opções consideradas

### Rust
- Linguagem compilada, forte em segurança de memória e tipagem estática — alinhado
  com Rule 20 (strong types, testable business logic) do CLAUDE.md.
- Ecossistema maduro de frameworks desktop cross-platform (ver ADR-002).
- Curva de aprendizado mais alta; ciclo de compilação mais lento que Python.

### Python
- Alta velocidade de desenvolvimento, mas historicamente fraco em:
  - Empacotamento/distribuição de apps desktop nativos.
  - Qualidade de GUI nativa (frameworks como PySide/Tkinter ficam distantes do
    visual "nativo" exigido pelas seções 7/9 do artifacts.md sobre customização).
  - Ausência de um caminho mobile realista equivalente ao Tauri.
- Foi descartado como runtime principal da aplicação, mas permanece candidato
  válido para scripts auxiliares/tooling (não para o app em si).

### TypeScript (isolado, sem Rust)
- Cobriria a camada de UI, mas exigiria um runtime desktop tipo Electron para
  rodar como app nativo, o que contraria a orientação de manter binários leves e
  integração OS profunda (always-on-top, tray, notificações nativas) sem a
  sobrecarga do Electron.
- TypeScript continua sendo usado — mas como camada de frontend dentro do Tauri
  (ver ADR-002), não como runtime isolado da aplicação.

## Decisão

Adotar **Rust** como linguagem/runtime da camada `domain`/`application`/
`infrastructure`, com **TypeScript** como linguagem da camada `UI` (frontend web
embarcado). Python fica fora do runtime de produção.

## Consequências

- Reforça a separação de camadas exigida na seção 4 do CLAUDE.md (`UI → Application
  → Domain → Ports → Infrastructure`), já que Rust naturalmente empurra lógica de
  negócio para fora do frontend.
- Exige que os devs tenham (ou desenvolvam) proficiência em Rust.
- Habilita o caminho mobile via Tauri 2 sem trocar de stack (ver ADR-008).
- Compilação Rust é mais lenta que scripts Python — mitigável com `cargo check`
  incremental e CI cacheado (a detalhar na fase de Foundation).

## Custo de reversão

Alto. Trocar a linguagem do core após a Fase 2 (Notas Locais) exigiria reescrever
domínio e persistência. Por isso esta decisão deve ser confirmada pelo DEV antes da
Fase 1.

## Nota do Dev:

- Decisão aceita para a linguagem RUST + TypeScript para a UI.
- Respeitar a linguagem/framework escolhido e fazer pesquisa aprofundada nas documentações.