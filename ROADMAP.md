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

**Objetivos:**
- `cargo tauri init`, estrutura de crates (`domain`, `application`, `infrastructure`, `app` do Tauri).
- Configurar workspace Cargo (múltiplos crates) respeitando a seção 4 do CLAUDE.md.
- Setup de `sqlx` + `tauri-plugin-sql` com uma migration vazia de teste (valida ADR-003 na prática).
- Prototipar uma tela mínima (uma nota estática) em React e em Svelte.
- Levar os dois protótipos + mockups para validação com o DEV.
- Só depois de aprovado: fechar definitivamente o frontend e remover a opção descartada.
- Setup de build (Vite) integrado ao `tauri dev`/`tauri build`.
- Comparar em profundidade `tauri-plugin-notification` (código próprio por cima)
  vs. o fork comunitário `tauri-plugin-notifications` (agendamento embutido).
- Critérios: manutenção, testabilidade do `NotificationService`, dependência de
  push/FCM/APNs (fora de escopo atual).
- Entregar adendo ao ADR-004 com a conclusão — não implementar notificações ainda
  (isso é Fase 3).
- Pipeline CI: `cargo check`, `cargo test`, `cargo clippy`, lint do frontend, build Tauri.
- Esqueleto das interfaces/ports citadas na seção 4 do CLAUDE.md (`NoteRepository`,
  `TaskRepository`, `NotificationService`, `WindowService`, `AuthenticationProvider`,
  `SupportSystemProvider`, `AIProvider`) como traits vazias — sem implementação.
- Documentar estrutura de pastas no README do repo.

**Definição de pronto da Fase 1:** app abre uma janela vazia via `tauri dev` no
CI (build verde), frontend definido (React ou Svelte, não ambos), traits de
domínio criadas, migration de teste rodando local.

---

## Fase 2 — Local Notes ✅ Concluída (2026-08-31)

Branch sugerida: `feature/phase2-local-notes`
Pré-requisito: Fase 1 "pronta" conforme critério acima.

> CRUD de notas implementado, persistência SQLite via sqlx funcionando,
> always-on-top validado em Windows (Linux e macOS documentados em ADR-002 addendum).

**Objetivos:**
- Modelar `Note` (title, content, tags, priority, color, size, position, opacity,
  pinning, archive) como tipos fortes no domínio (seção 6 do CLAUDE.md).
- Casos de uso: criar, editar, arquivar, deletar, fixar nota.
- Testes unitários do domínio (Rule 19).
- Componente de nota (sticky note) com os atributos visuais do domínio.
- Editor de conteúdo, seletor de cor, controle de opacidade.
- Drag/resize/position persistidos.
- Implementar `NoteRepository` com `sqlx` sobre SQLite (ADR-003).
- Migration real de `notes` (schema completo com os campos da seção 6).
- Testes de persistência (Rule 19: "Persistence" está explicitamente na lista).
- Implementar `WindowService.set_always_on_top` via API do Tauri.
- **Testar manualmente em Windows, macOS e Linux/Wayland/X11** — a limitação
  documentada no ADR-002 precisa virar um resultado real (funciona/não funciona
  por OS), não permanecer hipotética.
- Documentar limitações encontradas no próprio ADR-002 (seção "Consequências").

**Definição de pronto:** CRUD de notas funcionando ponta a ponta, persistindo,
com always-on-top testado (não só compilado) nos 3 OS.

---

## Fase 3 — Tasks, Deadlines & Notificações 🔄 Próxima / Em preparação

Branch sugerida: `feature/phase3-tasks-notifications`
Pré-requisito: Fase 2 pronta + conclusão da pesquisa de notificação (Fase 1).

**Objetivos:**
- Modelar `Task`, cálculo de deadline, thresholds configuráveis (5m/10m/.../custom).
- Testes de cálculo de deadline e de lembrete (Rule 19, itens explícitos).
- Tela/lista de tasks, indicadores visuais de deadline próximo.
- UI de configuração de thresholds de notificação.
- Implementar a decisão final do adendo ao ADR-004 (Fase 1).
- Agendamento, repetição, snooze como lógica própria sobre o plugin escolhido.
- `TaskRepository` via sqlx, migration de `tasks`.
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

## Fase 5 — Integração Mastersys ✅ Concluída (2026-09-02)

Branch: `feature/phase2-3-notes-tasks`

Desbloqueada porque o contrato foi **validado no código-fonte** do Mastersys
(`alrindoMaster/gerenciador_relatorios_V3`), não suposto — a tabela de
rastreabilidade endpoint→arquivo está no ADR-006.

**Entregue:**
- ADR-006 aceito: MasterDesk **consulta** a API do Mastersys, somente leitura
  (as duas alternativas — implementar o contrato NoteDesk local, ou coexistir
  com o Notas Flutuantes — estão comparadas no ADR).
- `SupportSystemProvider` com contrato real (sem nenhum método de escrita) e
  `MastersysProvider` em `infrastructure` como único módulo que conhece
  HTTP/JWT/JSON.
- `ExternalRef`/`ExternalWorkItem` no domínio; tarefa local segue com
  `external: None` e não depende de integração alguma.
- Reconciliação em `MastersysSyncService`: importa, atualiza, remove o que saiu
  da fila e **preserva** espelhos que têm anotações do usuário.
- `effective_due_date` replicando `getEffectiveDueDate` do Mastersys, com testes.
- Refresh token no cofre do SO (`keyring`); senha nunca persistida; nenhum token
  devolvido ao frontend.
- Painel de configuração em `MastersysPanel.tsx`.

**Pendente para fechar como suportado nos 3 SOs:**
- Fluxo de login + sync validado apenas em **Windows 11**. Cofre em macOS
  (Keychain) e Linux (Secret Service) precisa de validação manual —
  compilação não é validação cross-platform (CLAUDE §19).
- Sincronização é manual (botão). Agendador periódico não foi implementado.
- Falta de permissão em `/api/tickets` aborta o sync inteiro em vez de degradar
  para "só tarefas".

---

## Fase 5.1 — Anotações em tarefas ✅ Concluída (2026-09-02)

- `TaskNote` como entidade própria, filha de `Task` (não um campo de texto):
  cada anotação tem `created_at` próprio, o que dá a linha do tempo, e
  `description` continua sendo o enunciado — que é sobrescrito nos itens vindos
  do Mastersys.
- `task_notes` com `ON DELETE CASCADE` (migration 0005) e `foreign_keys(true)`
  no pool.
- `TaskNoteService` + comandos Tauri + UI de log de atendimento.

---

## Fase 5.2 — Tema claro/escuro/automático ✅ Concluída (2026-09-02)

Antecipa parte da Fase 8. Ver ADR-009.

- Tokens de CSS divididos por **papel** (`--text*`, `--surface*`, `--chrome*`,
  `--action*`) — o `--ink` anterior era texto E fundo de nav/botão ao mesmo
  tempo e não sobrevivia à inversão.
- Modo automático usa a API nativa do Tauri (`theme()` + `onThemeChanged`) como
  fonte da verdade, porque a propagação para `prefers-color-scheme` do webview é
  bug aberto no Linux; a media query fica como fallback.
- Tone-mapping das cores de nota (`noteSurface.ts`): matiz preservado,
  saturação/luminosidade recalculadas, texto por contraste WCAG real.
- Corrigido um defeito pré-existente: `textColorFor` cobria só as 8 cores
  predefinidas, e o rosa do preset não atingia AA nem no tema claro.
- 66 testes de frontend (vitest) cobrindo as 36 combinações cor × tema.

**Pendente:** `onThemeChanged`/`theme()` validados só em Windows 11.

## Fase 6 — IA (advisory) 🔒 Bloqueada (ADR-007)

O pré-requisito "contexto de task/ticket modelado" foi atendido pela Fase 5
(`ExternalRef` + `TaskNote` dão à IA o histórico de um atendimento). Segue
bloqueada por ADR-007: provedor, gestão de segredo e limites de autorização
ainda não decididos. IA nunca executa efeitos colaterais externos sem
autorização explícita futura.

---

## Fase 7 — Mobile

Branch sugerida: `feature/phase7-mobile`
Pré-requisito: Fases 1–3 validadas em desktop (mínimo).

**Objetivos:**
- `tauri ios init` / `tauri android init`, build Rust como lib estática.
- Adaptar frontend para layout mobile (visualização/acesso, não paridade).
- Validar individualmente cada plugin em iOS/Android (SQL, notificação, window) — nenhum plugin é assumido funcional sem teste.
- CI macOS para build iOS (Xcode) + toolchain Android (Gradle/NDK).

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
