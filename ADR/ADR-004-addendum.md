# ADR-004 — Addendum: Pesquisa Profunda: Plugin de Notificação

**Data:** 2026-08-11
**Status:** Concluído

## Comparação Detalhada

### `tauri-plugin-notification` (Oficial — tauri-apps)

| Aspecto | Detalhe |
|---------|---------|
| Mantenedor | tauri-apps (organização oficial) |
| Versão atual | 2.3.3 |
| Licença | Apache-2.0 OR MIT |
| Stars | 17 |
| Forks | 9 |
| Último release | ~9 meses atrás |
| Plataformas | Linux, Windows, macOS, Android, iOS |

**Funcionalidades:**
- `sendNotification({ title, body })`
- `isPermissionGranted()` / `requestPermission()`
- Suporte a ícone e som por notificação
- API Rust: `app.notification().builder()...`

**Limitações:**
- **Sem agendamento embutido** — lembretes precisam de código próprio
- Sem suporte a ações (botões na notificação)
- Sem canais de notificação
- Sem push remoto (FCM/APNs)

---

### `tauri-plugin-notifications` (Comunidade — Choochmeque)

| Aspecto | Detalhe |
|---------|---------|
| Mantenedor | Choochmeque (terceiro) |
| Versão atual | 0.4.6 |
| Licença | MIT |
| Stars | 76 |
| Forks | 15 |
| Criado | Outubro 2025 |
| Commits | 524 |
| Plataformas | Linux, Windows, macOS, Android, iOS |

**Funcionalidades:**
- Tudo do oficial +
- **Agendamento embutido:** `Schedule.at(date)` e `Schedule.interval()`
- **Notificações com ações** (botões interativos)
- **Canais de notificação** (agrupamento)
- **Push remoto:** FCM, APNs, UnifiedPush
- **Conteúdo rico:** large body, inbox style
- Exemplos prontos em `examples/notifications-demo/`

**Limitações:**
- Mantido por terceiro (risco de manutenção)
- Criado há ~10 meses (relativamente novo)
- Não é plugin oficial da organização tauri-apps

---

## Análise de Risco

| Critério | Oficial | Comunidade |
|----------|---------|------------|
| Manutenção de longo prazo | ✅ Alta (org Tauri) | ⚠️ Incerta (terceiro) |
| Funcionalidade embutida | ⚠️ Básica | ✅ Rica |
| Agendamento | ❌ Não tem | ✅ Tem |
| Push remoto | ❌ Não tem | ✅ Tem (FCM/APNs) |
| Comunidade/adoção | ✅ Maior | ⚠️ Menor |
| Estabilidade | ✅ Estável | ⚠️ Em evolução |

---

## Recomendação Final

**Manter o plugin oficial `tauri-plugin-notification`** para o disparo básico de notificações.

**Razões:**
1. **Manutenção garantida** — mantido pela organização Tauri, não depende de um desenvolvedor individual
2. **Escopo do MasterDesk** — é local-first, não precisa de push remoto (FCM/APNs) agora
3. **Agendamento é lógica de negócio** — o `NotificationService` do MasterDesk deve controlar isso (como decidido originalmente)
4. **Custo de reversão baixo** — se no futuro precisar de push remoto, trocar o plugin não afeta o domínio

**Quando reavaliar:**
- Se o MasterDesk precisar de push remoto (integração com Mastersys)
- Se precisar de notificações com ações interativas
- Se o plugin oficial parar de ser mantido

---

## Conclusão do ADR-004

A decisão original está **confirmada**:
- Usar `tauri-plugin-notification` (oficial)
- Agendamento, threshold, repetição e snooze = código próprio no `NotificationService`
- Plugin comunitário documentado como alternativa para futuro (push remoto)
