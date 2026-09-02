# Integração NoteDesk — SUBSTITUÍDO

> **Este documento está obsoleto e descrevia a direção errada da integração.**
> Use [INTEGRACAO_MASTERSYS.md](./INTEGRACAO_MASTERSYS.md) e
> [ADR-006](../ADR/ADR-006-mastersys-integration.md).

## O que estava incorreto

A versão anterior deste documento descrevia o MasterDesk como **cliente** que
empurra tarefas para um sistema chamado NoteDesk, via
`POST /api/v1/tasks/upsert` com `X-NoteDesk-Api-Key`.

A leitura do código real do Mastersys (`alrindoMaster/gerenciador_relatorios_V3`)
mostrou que a direção é a oposta:

- `modules/tasks/services/NoteDeskSyncService.ts` — o **Mastersys** monta o
  payload e o enfileira em `task_notedesk_outbox`.
- `frontend/src/hooks/useNoteDeskBridge.ts` — uma ponte no navegador do usuário
  consome a fila e entrega em `http://127.0.0.1:17882/api/v1/tasks/upsert`.
- O receptor é um app local do Windows, **"Notas Flutuantes"**, cuja chave fica
  em `%LOCALAPPDATA%\NotasFlutuantes\integracao.json`.

Ou seja: o NoteDesk é um **servidor local** que recebe, não um serviço remoto
para onde se empurra. O cliente descrito aqui não tinha nada do outro lado para
conversar.

## O que foi decidido no lugar

O MasterDesk **consulta** a API do Mastersys (somente leitura), autenticado com
as credenciais do próprio usuário. Sem porta de escuta, sem API key
compartilhada, sem dependência de navegador aberto e sem alteração no Mastersys.
As três alternativas consideradas — incluindo implementar o contrato NoteDesk —
estão comparadas no ADR-006.

## Pendência para o DEV

`notedesk-integration-client.js`, na raiz do repositório, é o cliente que este
documento descrevia. Ele implementa o contrato na direção que não é usada e hoje
não tem nenhum consumidor no projeto.

Não foi removido porque é trabalho de outro autor (Collaborative Rule 6). Se não
houver plano de uso, pode ser apagado.
