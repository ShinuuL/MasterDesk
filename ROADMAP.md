# MasterDesk — Roadmap de Fases

Baseado na seção 24 do CLAUDE.md (Development Order) e nos ADRs aceitos
(001, 002, 003, 004, 008). ADR-005/006/007 permanecem bloqueados.

Stack confirmada: **Rust + Tauri 2 + TypeScript** (frontend a validar entre
React/Svelte na Fase 1), **SQLite via sqlx**, **tauri-plugin-notification**,
**Tauri Mobile** para a Fase 7.

---

## Fase 0 — Research & ADRs ✅ Concluída

ADR-001, 002, 003, 004, 008 aceitos pelo DEV. Pendências abertas herdadas para a
Fase 1 (ver seção "Pré-requisitos" abaixo).

---

## Fase 1 — Foundation

Branch sugerida: `feature/phase1-foundation`

Objetivo: esqueleto do projeto rodando (janela abre, layout base, CI verde),
sem features de domínio ainda.

**Pré-requisitos herdados dos ADRs (bloqueiam parte da Fase 1):**
- ADR-002 pede "pesquisa mais aprofundada e mock-ups para validação" antes de
  travar React — **não iniciar componentes de UI reais até isso ser resolvido.**
- ADR-004 pede pesquisa aprofundada comparando implementação própria vs. plugin
  comunitário de notificação — não bloqueia o setup, mas deve ser entregue nesta
  fase, antes da Fase 3.

**Dev 1 — Scaffold Rust/Tauri**
- `cargo tauri init`, estrutura de crates (`domain`, `application`, `infrastructure`, `app` do Tauri).
- Configurar workspace Cargo (múltiplos crates) respeitando a seção 4 do CLAUDE.md.
- Setup de `sqlx` + `tauri-plugin-sql` com uma migration vazia de teste (valida ADR-003 na prática).

**Dev 2 — Frontend + validação React vs Svelte (item pendente do ADR-002)**
- Prototipar uma tela mínima (uma nota estática) em React e em Svelte.
- Levar os dois protótipos + mockups para validação com o DEV.
- Só depois de aprovado: fechar definitivamente o frontend e remover a opção descartada.
- Setup de build (Vite) integrado ao `tauri dev`/`tauri build`.

**Dev 3 — Notificações: pesquisa aprofundada pendente do ADR-004**
- Comparar em profundidade `tauri-plugin-notification` (código próprio por cima)
  vs. o fork comunitário `tauri-plugin-notifications` (agendamento embutido).
- Critérios: manutenção, testabilidade do `NotificationService`, dependência de
  push/FCM/APNs (fora de escopo atual).
- Entregar adendo ao ADR-004 com a conclusão — não implementar notificações ainda
  (isso é Fase 3).

**Dev 4 — CI/CD e arquitetura**
- Pipeline CI: `cargo check`, `cargo test`, `cargo clippy`, lint do frontend, build Tauri.
- Esqueleto das interfaces/ports citadas na seção 4 do CLAUDE.md (`NoteRepository`,
  `TaskRepository`, `NotificationService`, `WindowService`, `AuthenticationProvider`,
  `SupportSystemProvider`, `AIProvider`) como traits vazias — sem implementação.
- Documentar estrutura de pastas no README do repo.

**Definição de pronto da Fase 1:** app abre uma janela vazia via `tauri dev` no
CI (build verde), frontend definido (React ou Svelte, não ambos), traits de
domínio criadas, migration de teste rodando local.

---

## Fase 2 — Local Notes

Branch sugerida: `feature/phase2-local-notes`
Pré-requisito: Fase 1 "pronta" conforme critério acima.

**Dev 1 — Domínio de Notes**
- Modelar `Note` (title, content, tags, priority, color, size, position, opacity,
  pinning, archive) como tipos fortes no domínio (seção 6 do CLAUDE.md).
- Casos de uso: criar, editar, arquivar, deletar, fixar nota.
- Testes unitários do domínio (Rule 19).

**Dev 2 — UI de Notes**
- Componente de nota (sticky note) com os atributos visuais do domínio.
- Editor de conteúdo, seletor de cor, controle de opacidade.
- Drag/resize/position persistidos.

**Dev 3 — Persistência de Notes**
- Implementar `NoteRepository` com `sqlx` sobre SQLite (ADR-003).
- Migration real de `notes` (schema completo com os campos da seção 6).
- Testes de persistência (Rule 19: "Persistence" está explicitamente na lista).

**Dev 4 — Always-on-top (validação real, não assumida)**
- Implementar `WindowService.set_always_on_top` via API do Tauri.
- **Testar manualmente em Windows, macOS e Linux/Wayland/X11** — a limitação
  documentada no ADR-002 precisa virar um resultado real (funciona/não funciona
  por OS), não permanecer hipotética.
- Documentar limitações encontradas no próprio ADR-002 (seção "Consequências").

**Definição de pronto:** CRUD de notas funcionando ponta a ponta, persistindo,
com always-on-top testado (não só compilado) nos 3 OS.

---

## Fase 3 — Tasks, Deadlines & Notificações

Branch sugerida: `feature/phase3-tasks-notifications`
Pré-requisito: Fase 2 pronta + conclusão da pesquisa de notificação (Fase 1, Dev 3).

**Dev 1 — Domínio de Tasks/Deadlines**
- Modelar `Task`, cálculo de deadline, thresholds configuráveis (5m/10m/.../custom).
- Testes de cálculo de deadline e de lembrete (Rule 19, itens explícitos).

**Dev 2 — UI de Tasks e configuração de lembretes**
- Tela/lista de tasks, indicadores visuais de deadline próximo.
- UI de configuração de thresholds de notificação.

**Dev 3 — `NotificationService` + `TaskRepository`**
- Implementar a decisão final do adendo ao ADR-004 (Fase 1).
- Agendamento, repetição, snooze como lógica própria sobre o plugin escolhido.
- `TaskRepository` via sqlx, migration de `tasks`.

**Dev 4 — Validação cross-platform de notificações**
- Testar permissões de notificação em Windows/macOS/Linux (comportamento difere
  por OS, conforme já sinalizado no ADR-004).
- Documentar resultado real por OS.

**Definição de pronto:** tasks com deadline geram lembretes reais testados nos
3 OS, com snooze/repetição funcionando.

---

## Fase 4 — Autenticação 🔒 Bloqueada (ADR-005)

Só inicia depois da Fase 3 **e** com definição de `AuthenticationProvider`
local. Segue bloqueada para qualquer suposição sobre o mecanismo real do
Mastersys (Regra 1 do CLAUDE.md).

## Fase 5 — Integração Mastersys 🔒 Bloqueada (ADR-006)

Só inicia quando o DEV fornecer a documentação/contrato real da API do
Mastersys. Nenhuma tarefa pode ser aberta antes disso.

## Fase 6 — IA (advisory) 🔒 Bloqueada (ADR-007)

Só inicia após Fase 5, com o contexto de task/ticket já modelado. IA nunca
executa efeitos colaterais externos sem autorização explícita futura.

---

## Fase 7 — Mobile

Branch sugerida: `feature/phase7-mobile`
Pré-requisito: Fases 1–3 validadas em desktop (mínimo).

**Dev 1** — `tauri ios init` / `tauri android init`, build Rust como lib estática.
**Dev 2** — Adaptar frontend para layout mobile (visualização/acesso, não paridade).
**Dev 3** — Validar individualmente cada plugin em iOS/Android (SQL, notificação, window) — nenhum plugin é assumido funcional sem teste.
**Dev 4** — CI macOS para build iOS (Xcode) + toolchain Android (Gradle/NDK).

---

## Fase 8 — Customização de UI & Polimento

Branch sugerida: `feature/phase8-ui-customization`
Pode começar em paralelo à Fase 7 se houver capacidade, pois não depende de mobile.

- Temas, fontes, tamanhos, sombras, bordas arredondadas (seção 9 do CLAUDE.md).
- Atalhos de teclado configuráveis.
- Idioma e formatos de data/hora.
- Comportamento de inicialização (auto-start, restaurar posição de notas).

---

## Regra geral entre fases

Nenhuma fase seguinte começa antes da "Definição de pronto" da fase anterior
ser atingida, conforme a seção 24 do CLAUDE.md — exceto onde marcado
"pode começar em paralelo" acima. Cada fase segue a Integration Rule do
CLAUDE.md antes de PR: rebase em main, format, lint, test, build, checagem de
arquivos não relacionados.
