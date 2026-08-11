# ADR-004 — Sistema de Notificações

**Status:** Confirmado 
**Dev:** Decisão é utilizar o plugin do tauri oficial e fazer uma pesquisa mais aprofundada sobre 
o código puro e o puglin comunitário.
**Data:** 2026-08-11

## Contexto

Lembretes de deadline são funcionalidade core (seção 8 do CLAUDE.md / seção 5 do
artifacts.md), com thresholds configuráveis (5m/10m/.../custom) e comportamento
eventualmente incluindo som, ênfase visual, repetição e snooze.

## Opções consideradas

### `tauri-plugin-notification` (oficial, mantido pela org tauri-apps)
- API JS (`sendNotification`, `isPermissionGranted`, `requestPermission`) e API
  Rust (`app.notification().builder()...`) prontas.
- Cobre notificações desktop (Windows/macOS/Linux) e mobile via o mesmo plugin.
- Suporta corpo, ícone e som por notificação; não é responsável por lembrete
  recorrente ou snooze — isso é lógica de aplicação (agendamento) que o MasterDesk
  precisa implementar por cima, no `NotificationService`.

### Plugin comunitário `tauri-plugin-notifications` (fork estendido)
- Adiciona agendamento, ações, canais e push via FCM/APNs.
- Push (FCM/APNs) só é relevante quando houver backend remoto — fora do escopo do
  MasterDesk local-first atual.
- Mantido por terceiros, não pela organização oficial `tauri-apps` — maturidade e
  manutenção de longo prazo precisam ser reavaliadas antes de adotar (Rule 2 do
  CLAUDE.md: verificar status de manutenção).

## Decisão

Adotar o plugin oficial **`tauri-plugin-notification`** para o disparo do
"toast" de sistema, e implementar o **agendamento, threshold, repetição e snooze
como lógica de aplicação própria** (`NotificationService` no MasterDesk, seção 4 do
CLAUDE.md), agendando o disparo via timer no processo Rust — não delegando essa
lógica de negócio ao plugin.

Reavaliar o plugin comunitário estendido apenas se/quando push remoto for
necessário (fora do escopo atual).

## Consequências

- `NotificationService` fica testável isoladamente (Rule 19 do CLAUDE.md — testes
  de cálculo de lembrete), pois o agendamento é código nosso, não do plugin.
- Som e ênfase visual por note dependem de o quanto o webview do Tauri permite
  estilizar/animar o próprio card do note — a validar na Fase 3, não assumir.
- Comportamento de notificação nativa varia por OS (permissões diferentes no
  macOS/Windows/Linux) — a testar em cada plataforma suportada antes de considerar
  a feature completa (seção 25 do artifacts.md).

## Custo de reversão

Baixo. O plugin é uma borda de infraestrutura fina; trocar por outra lib de
notificação não afeta o `NotificationService` em si.
