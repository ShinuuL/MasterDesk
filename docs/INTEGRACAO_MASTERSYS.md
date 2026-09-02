# Integração MasterDesk ↔ Mastersys Suporte

> Decisão e justificativa em [ADR-006](../ADR/ADR-006-mastersys-integration.md).
> Este documento é o guia operacional: como ligar, o que esperar e como
> diagnosticar.

## 1. O que a integração faz

O MasterDesk **puxa** as tarefas e chamados atribuídos a você no Mastersys e os
espelha no seu quadro local.

```
Mastersys Suporte                     MasterDesk (Tauri + SQLite)
──────────────────                    ───────────────────────────
POST /api/auth/login      ◄────────── conectar (usuário + senha)
POST /api/auth/refresh    ◄────────── renovação automática do token
GET  /api/tasks/users/:id ◄────────── sincronizar
GET  /api/tickets?assignedTo=:id ◄───
                                       │
                                       ├─ tarefas espelhadas (somente leitura)
                                       └─ anotações locais (só suas)
```

**Mão única.** O MasterDesk nunca escreve no Mastersys. Não existe método de
escrita em `SupportSystemProvider`. Fechar chamado, comentar ou reatribuir
continuam sendo feitos no Mastersys.

Chamados **não** têm um canal separado: um chamado atribuído a você chega como
item com `kind: "Ticket"`, trazendo cliente, número e status. Se o chamado já
tem uma tarefa no seu quadro do Mastersys, ele entra uma única vez (pela
tarefa), não duas.

## 2. Como ligar

Em **Tarefas → Mastersys**:

1. **Endereço** — a URL base do Mastersys, ex. `https://suporte.suaempresa.com`
   (sem `/api`). Precisa de `http://` ou `https://`.
2. **Entrar** — o mesmo usuário/e-mail e senha que você usa no Mastersys.
3. **Sincronizar agora**.

A senha é usada na chamada de login e descartada. O que fica guardado:

| O que | Onde |
|---|---|
| refresh token | cofre do SO (Windows Credential Manager / Keychain / Secret Service) |
| access token | somente memória |
| endereço, id/nome/e-mail do usuário | `app_settings` no `masterdesk.db` |
| senha | **em lugar nenhum** |

## 3. O que a sincronização faz com os seus dados

| Campo | Dono | No sync |
|---|---|---|
| título, descrição, prioridade, prazo, concluída | Mastersys | **sobrescritos** |
| anotações da tarefa | você | **nunca tocadas** |
| lembretes | você | definidos só na importação; depois é sua escolha |

Editar título ou prazo de um item espelhado não tem efeito duradouro — a
próxima sincronização sobrescreve. O card mostra o carimbo de origem justamente
para deixar isso visível antes de você tentar.

**Item que sai da sua fila** (reatribuído, cancelado) é removido do quadro —
**exceto** se tiver anotações suas. Nesse caso ele fica, marcado como concluído.
Anotação é trabalho manual e não é descartada por uma sincronização.

**Desconectar** aplica a mesma regra: apaga os espelhos sem anotações e mantém
os que têm. Espelhos deixados para trás ficariam congelados no quadro,
indistinguíveis de tarefas locais.

O resumo depois de cada operação diz exatamente o que aconteceu:
`importados`, `atualizados`, `removidos`, `mantidos com anotações`.

## 4. Prioridade e prazo

**Prioridade.** O `TaskDTO` do Mastersys não tem campo de prioridade — tarefas
importadas ficam em `Média`. Chamados usam a prioridade real
(`low/medium/high/critical` → `Baixa/Média/Alta/Urgente`).

**Prazo.** Segue a mesma regra do Mastersys (`getEffectiveDueDate` em
`modules/tasks/utils/overdue.ts`), não "a primeira data que existir":

- previsão do chamado e agendamento do chamado, ambos no futuro → a **mais próxima**
- só um no futuro → esse
- ambos no passado → o **mais recente**
- nenhuma data de chamado → o agendamento da própria tarefa

**Concluído.** Tarefa: `status == "finished"`. Chamado: `closedAt` ou
`resolvedAt` preenchido — não uma lista de status, porque o Mastersys permite
status customizados criados na tela de configuração.

## 5. Diagnóstico

| Mensagem | Causa provável | O que fazer |
|---|---|---|
| "não foi possível conectar ao Mastersys" | endereço errado, VPN fora, host inacessível | conferir o endereço; abrir a mesma URL no navegador |
| "o Mastersys não respondeu no tempo esperado" | rede lenta ou servidor sobrecarregado (timeout de 15s) | tentar de novo |
| "unauthorized" ao entrar | usuário/senha inválidos, ou conta inativa no Mastersys | conferir credenciais |
| "unauthorized" ao sincronizar | refresh token expirado ou revogado | entrar de novo (o token expirado é apagado automaticamente) |
| "seu usuário do Mastersys não tem permissão para esta consulta" | conta sem acesso a `/api/tickets` | pedir permissão ao admin do Mastersys |
| "o cofre de credenciais do sistema não está disponível" | Linux sem agente de Secret Service (GNOME Keyring/KWallet) | iniciar o agente; o app **não** grava token em texto plano |
| "endereço deve começar com http:// ou https://" | endereço sem esquema | usar a URL completa |
| certificado recusado em HTTPS | CA não está na loja de certificados do SO | instalar a CA no SO; não há opção de ignorar validação |

## 6. Segurança

- Nenhuma porta de escuta é aberta no seu computador.
- Nenhum comando Tauri devolve token ao frontend — `mastersys_status` expõe
  endereço, estado da conexão e identidade, nada mais.
- Mensagens de erro são escritas à mão em vez de repassar o erro do cliente
  HTTP, que pode conter a URL completa (CLAUDE §13/18).
- `app_settings` guarda **apenas configuração não sensível**. O banco fica no
  diretório do usuário sem criptografia; segredo ali seria texto plano.

## 7. Onde está o código

| Camada | Arquivo | Papel |
|---|---|---|
| domínio | `crates/domain/src/external.rs` | `ExternalRef`, `ExternalWorkItem`, `SupportIdentity` |
| domínio | `crates/domain/src/ports.rs` | trait `SupportSystemProvider` (sem escrita) |
| infra | `crates/infrastructure/src/mastersys_provider.rs` | **único** módulo que conhece HTTP/JWT/JSON do Mastersys |
| infra | `crates/infrastructure/src/secret_store.rs` | cofre do SO via `keyring` |
| infra | `crates/infrastructure/src/sqlite_settings_repository.rs` | `app_settings` |
| aplicação | `crates/application/src/mastersys.rs` | reconciliação (importar/atualizar/remover/preservar) |
| Tauri | `src-tauri/src/commands.rs` | `mastersys_status/set_endpoint/connect/disconnect/sync` |
| frontend | `frontend/src/components/MastersysPanel.tsx` | painel de configuração |

## 8. Limitações atuais

- **É pull, e manual.** Um item criado no Mastersys aparece na próxima
  sincronização. Agendador periódico é trabalho futuro.
- **Falta de permissão aborta a sincronização inteira** em vez de degradar para
  "só tarefas".
- **Validado só em Windows 11.** Cofre e sincronização precisam de verificação
  manual em macOS e Linux.
- Não há link para abrir o chamado no navegador — a rota da UI do Mastersys
  ainda não foi verificada, e inventá-la contraria a Regra 1 do CLAUDE.md.

## 9. Sobre o NoteDesk / "Notas Flutuantes"

O Mastersys tem uma integração anterior com um app local chamado **NoteDesk**
(`modules/tasks/services/NoteDeskSyncService.ts` + a ponte
`frontend/src/hooks/useNoteDeskBridge.ts`), que entrega em
`http://127.0.0.1:17882/api/v1/tasks/upsert` autenticado por
`X-NoteDesk-Api-Key`.

**O MasterDesk não implementa esse contrato.** A comparação das duas abordagens
e o motivo da escolha estão no ADR-006. O documento antigo
`docs/INTEGRACAO_NOTEDESK.md` descrevia a direção invertida e foi substituído
por este.
