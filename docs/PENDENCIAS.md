# Pendências — quadro de tarefas, pop-out e sincronização

Aberto em 2026-09-02, durante a implementação do plano de filtros/busca/status.
Complementa o `ROADMAP.md` (que descreve fases) com o que ficou em aberto item a
item. Fecha-se um item apagando-o daqui e registrando no ADR correspondente.

---

## ✅ Concluído (Etapa 1, verificado)

- Catálogo de status espelhado (`GET /api/ticket-statuses` → `mastersys_status_catalog`).
- Selo de status colorido, cor da origem via `noteSurface()` (contraste AA nos dois temas).
- `ExternalRef.status_parked`: item parado não conta como atrasado nem agenda lembrete.
- Filtros do quadro espelhando o vocabulário do suporte, com default vindo de `default_filter`.
- Busca local (300 ms de debounce) + busca ao vivo no Mastersys, com importação como tarefa local.
- CSP de produção definida e o acoplamento com o `<style>` inline documentado em ADR-009.

### Etapa 2 — pop-out de tarefas e bugs do pop-out de notas

- **B1 corrigido** — capability declara as permissões de escrita de janela, com
  `core:window:allow-start-dragging` à frente. Arrastar volta a funcionar.
- **B2 corrigido** — posição validada contra os monitores reais
  (`position_is_reachable`), com centralização quando a posição salva não é
  alcançável. 11 testes cobrindo monitor removido, monitor à esquerda
  (coordenada negativa), sliver na borda e coordenada não-finita.
- **B3 corrigido** — `open_note_window_ids` / `open_task_window_ids`: a lista de
  janelas abertas vem do gerenciador de janelas, não de estado de componente.
- **B4 corrigido** — em falha a reconciliação agora falha **aberta** (mostra a
  nota) em vez de fechada.
- **Pop-out de tarefa** — comandos, roteamento `#task=`, `TaskWindowApp`,
  geometria em `task_window_state`, fecha janela órfã de espelho retirado, e o
  close-to-tray passou a contar janelas `task-` também.
- **B6 corrigido** (regressão introduzida junto com o pop-out de tarefa, filmada
  pelo DEV em 2026-09-03): a janela destacada **agitava freneticamente**. Os
  comandos de geometria gravavam no banco **e** aplicavam a posição na janela;
  como quem os chamava era o listener de `onMoved`/`onResized`, o ciclo era
  gravar → mover → `onMoved` → gravar, e não convergia porque a coordenada ia e
  voltava convertida por escala. Separado em `save_task_window_*`, que só
  persistem. O pop-out de nota nunca teve o laço porque o `onMoved` dele chama
  `update_note`, que só grava — o desvio desse padrão foi meu.
- **B5 resolvido removendo** (decisão do DEV em 2026-09-03): o port
  `WindowService` e o `TauriWindowService` foram apagados. Eram código morto que
  parecia vivo — ignoravam o `note_id` e operavam sempre na janela `main`, e o
  `set_opacity` validava a faixa para não fazer nada. Nada os construía. O
  controle real de pop-out são os comandos `*_note_window` / `*_task_window`.
- **Geometria agora é debounceada (250 ms)** nos dois pop-outs. `onMoved`
  dispara a cada pixel, então arrastar emitia centenas de escritas no SQLite.
  Nas notas isso nunca havia sido exercitado: até a correção de B1 o arrasto não
  funcionava, e o listener não era chamado de fato.
- **Sincronização automática** — `sync_scheduler` com polling de 5 min
  (configurável, piso de 1 min), coalescência de 2 s e intervalo mínimo de 15 s.
- **Canal de tempo real** — `rust_socketio` (dependência dormente aceita pelo
  DEV), `RealtimeSupervisor` ligando/desligando junto com a sessão, salas
  reentradas a cada reconexão, e indicador na UI distinguindo tempo real de
  polling. ADR-010.

Cobertura: **180 testes Rust**, 93 testes TS, `tsc` limpo, build de produção ok.

---

## 📋 Pendente

### Primeiro uso por outros usuários — decidido: fica manual, documentado

Levantado pelo DEV em 2026-09-03: *"eu vejo as tarefas pois eu já conectei antes
com meu usuário, teria que pensar para quando for para outros usuários."*

**Decisão do DEV no mesmo dia: deixar como está e documentar.**

O primeiro uso de cada pessoa exige digitar o endereço do Mastersys no painel de
integração, e só depois usuário e senha:

```
https://mastersys.app.br/suporte
```

Sem `/api` no final e sem barra final — o provider concatena os caminhos.

Foram consideradas e **rejeitadas** duas alternativas:

- **Endereço embutido no build.** Removeria o passo, mas amarra o binário a um
  cliente específico. Rejeitado para o MasterNote continuar genérico.
- **Descoberta por domínio** (digitar só `mastersys.app.br` e o app tentar
  `/suporte`, `/api`, a raiz). Rejeitado por fazer requisições às cegas e
  produzir mensagens de erro ainda mais confusas quando nenhuma funciona.

**O que sustenta a decisão:** o endereço é passo de instalação, não de uso
diário — feito uma vez por máquina. E as mensagens de erro melhoraram no mesmo
dia: um 401 agora diz o motivo real (senha errada, conta inativa, sessão
expirada), e o painel mostra o resultado da última sincronização com a mensagem
de falha. Quem errar o endereço vê "não foi possível conectar ao Mastersys —
verifique o endereço e a rede", que aponta para onde olhar.

**O que fica pendente disto:** o guia de instalação para os outros usuários
precisa trazer o endereço em destaque.
[INTEGRACAO_MASTERDESK_PRODUCAO.md](../../INTEGRACAO_MASTERDESK_PRODUCAO.md) (na
pasta acima do repositório) já tem o endereço e um checklist de diagnóstico;
falta apenas ele chegar a quem for instalar.

## 📦 Distribuição para a equipe — o que avisar

### Desinstalar o "MasterDesk" antes de instalar o "MasterNote"

O `identifier` continua `com.masterdesk.app` (para não orfanar o banco), mas o
`productName` virou `MasterNote`. No Tauri 2 o diretório de instalação e a chave
de desinstalação derivam do **productName**, e não há `upgradeCode` fixado no
`tauri.conf.json`. Resultado para quem já tem a versão anterior:

- o instalador novo cria um aplicativo **separado**, não uma atualização;
- "MasterDesk" continua listado em Aplicativos e Recursos;
- os dois compartilham o **mesmo banco**, porque o identifier é o mesmo;
- e o executável antigo **ainda funciona**, com os bugs já corrigidos.

O risco prático é um colega abrir o atalho antigo e reportar como bug a janela
agitando ou o pós-atendimento contando como atrasado — coisas resolvidas.

**Instrução para o guia:** desinstalar "MasterDesk" primeiro. Os dados são
preservados, porque vivem em `%APPDATA%/com.masterdesk.app/` e não na pasta de
instalação.

Fixar um `upgradeCode` para o MSI atualizar no lugar foi considerado e
descartado: resolveria só o MSI (não o NSIS), exigiria descobrir o UUID gerado
para o nome antigo, e para um 0.1.0 interno desinstalar é mais simples e
verificável.

### Testes antes do deploy do `ticketStatus`

A alteração no backend do suporte (`TaskDTO.ticketStatus`) está com o outro dev.
**Enquanto ela não subir**, o comportamento é o compatível: sem o campo, o
MasterNote usa o status da tarefa e detecta item parado apenas para chamados que
chegam pelo ramo de *chamados* — não pelos que têm tarefa no quadro.

Então, nesse intervalo, é **esperado** que alguns itens em pós-atendimento ainda
apareçam como atrasados. Não é bug novo; é a lacuna que aquele deploy fecha.

## 🐛 Causa raiz no Mastersys — reatribuição da tarefa na mudança de status

Relatado pelo DEV em 2026-09-04: *"o Note acompanhou a mudança de status mas não
manteve atribuído a meu usuário embora o analista não tenha mudado."*

**O MasterDesk não tinha bug aqui.** A causa está em
`backend/src/modules/tickets/services/TicketService.ts` (~linha 810), na
automação que roda ao mudar o status de um chamado:

```ts
const targetAssigneeId = data.assignedTo ?? ticket.assignedTo ?? (isTechnician ? userId : null);
// ...
const taskAssigneeId = data.status === 'em_analise' ? (ticket.createdBy ?? userId) : userId;
```

Para qualquer status "ativo" que não seja `em_analise` nem `nao_conformidade`, a
tarefa vinculada é reatribuída a **`userId` — quem mudou o status**, ignorando
`ticket.assignedTo` (o analista responsável). O `targetAssigneeId` calculado
logo acima, que traz o analista, não é usado nesse ramo.

Consequência no MasterDesk: a tarefa sai de `GET /api/tasks/users/<eu>` e o
espelho `task-<id>` fica órfão, embora o chamado continue sendo meu. O quadro
mostrava o item ser retirado logo depois de "acompanhar" a mudança de status.

**Decisão do DEV em 2026-09-04: não mexer no Mastersys; contornar no
MasterDesk.** O contorno é a **reancoragem** em `MastersysSyncService::sync`:
quando um espelho órfão e um item novo apontam para o **mesmo número de
chamado**, o espelho é reapontado para o id novo em vez de retirado — id local,
anotações e lembretes preservados. Sete testes cobrem o caminho, incluindo os
casos em que a reancoragem **não** deve acontecer (chamado diferente, item
cancelado, dois itens disputando o mesmo espelho).

O que fica pendente: a correção de verdade é usar `ticket.assignedTo` como dono
da tarefa nessa automação, caindo em quem mudou o status só quando não há
analista. Enquanto não for feita, no Mastersys a tarefa continua trocando de
dono — o contorno resolve o quadro do MasterDesk, não o quadro do suporte.

---

## 🔒 Achado de segurança no Mastersys — reportado, não corrigido

`backend/src/shared/infra/socket/SocketService.ts:36-56`: `chat:join` aceita o
`userId` **direto do cliente, sem verificar JWT**, e faz
`socket.join('user:' + uid)`. Qualquer cliente conectado entra na sala de
qualquer usuário e recebe os eventos dele. `join_room` (linha 82) aceita nome de
sala arbitrário com o mesmo problema.

Não tocado: a pasta é somente leitura e a correção é decisão do DEV do Mastersys.

Consequência para cá: é por isso que o tempo real é **oportunista**. Quando isso
for corrigido, o cliente do MasterDesk terá de mandar token no handshake; até
lá, e depois, o polling cobre a lacuna sem perda de função.

---

## ❓ Decisões pendentes do DEV

1. **`category` do chamado** — descartado em 2026-09-02 ("era somente status").
   Se voltar à pauta: `TaskDTO` não expõe `category`, só `TicketDTO`, então
   ficaria vazia na maioria dos cards. Dá para preencher parcialmente cruzando
   com os chamados já buscados (mapa `ticket_id → category`), mas um chamado
   atribuído a outra pessoa não estaria nesse mapa.
2. **Assinatura de código** — dispensada pelo DEV em 2026-09-02. Sem ela o
   instalador dispara SmartScreen ("aplicativo não reconhecido") em cada
   instalação, e antivírus corporativo pode pôr em quarentena.
3. **WebView2 no instalador** — hoje `downloadBootstrapper` (padrão), que exige
   internet na máquina do usuário. Se houver Windows 10 no parque, considerar
   `offlineInstaller` (+~150 MB, funciona sem rede).

---

## ⚠️ Limitações conhecidas, por limitação da origem

- ~~**`status_parked` não cobre o ramo de tarefas.**~~ **Resolvido em
  2026-09-03** com a única alteração feita no repositório do suporte:
  `TaskDTO.ticketStatus` (`tk.status as ticket_status` nos três SELECTs que já
  faziam JOIN com `tickets`, + o campo no DTO). Aditivo, 26 linhas, nenhuma
  remoção. O MasterNote passa a preferir o status do chamado sobre o da tarefa
  em `MastersysTask::effective_status` — o que também dá cor ao selo, já que
  status de tarefa não existe em `ticket_statuses` e cairia no cinza.
  Compatível com Mastersys anterior à mudança: sem o campo, volta ao status da
  tarefa.
- **Chamado aberto mais antigo que a janela não aparece.** A janela de 90 dias
  (`mastersys.ticket_window_days`) existe porque `GET /api/tickets` não tem
  `LIMIT` nem filtro de status padrão. Exposta em `mastersys_status` para a
  ausência ser legível na UI.
- **Tarefa concluída no Mastersys não chega** — `TaskRepository.findAll` aplica
  `AND t.status != 'finished'`. O espelho é apagado localmente (ou marcado
  concluído, se tiver anotações). Tarefa interna (`is_internal = 1`) também
  nunca sincroniza, e o endpoint não tem parâmetro para pedi-la.
- **Prazo é um só.** `forecast_date`, `scheduled_for` e `scheduled_at` são
  colapsados num `deadline` e não se sabe qual venceu — por isso o filtro se
  chama "Prazo", não "Agendada".
