/**
 * Filtro e busca do quadro de tarefas — lógica pura, sem React.
 *
 * ## Por que filtrar no cliente
 *
 * Os espelhos já estão todos em memória (`listPendingTasks` +
 * `listCompletedTasks`), e nenhum método do repositório aceita filtro. Filtrar
 * aqui é instantâneo, funciona offline e não exige índice novo no SQLite.
 *
 * ## Vocabulário
 *
 * Espelha o `TaskFiltersPanel` do Mastersys, restrito ao que o MasterNote
 * realmente persiste. Ficaram de fora, por falta de dado e não por escolha de
 * UI: `Criado por` (criador não é sincronizado), `Vínculo com agendamento`
 * (`linkedSchedules` não é sincronizado) e `Usuários (colunas)` (o quadro daqui
 * é pessoal). Entrou um filtro que o suporte não tem — `Origem` — para quando
 * tarefas locais e espelhos convivem numa mesma lista.
 *
 * ## Um estado de filtro por aba
 *
 * Tarefas (locais), Chamados (espelhos) e Concluídos usam o mesmo predicado,
 * mas defaults e preferências separados — ver [`FilterScope`]. Depois que as
 * abas passaram a separar a origem por estrutura, `Origem` só é oferecido em
 * Concluídos, a única lista onde as duas ainda se misturam.
 */

import type { MastersysTicketStatus, Task } from "../types";

/** Igual ao `TriState` do suporte. */
export type TriState = "all" | "yes" | "no";

export type OriginFilter = "all" | "local" | "mastersys";

export interface TaskFilterState {
  /**
   * Slugs de status da origem que passam. Vazio = **todos**, inclusive parados.
   *
   * Cuidado ao ler: "vazio = todos" é o mesmo comportamento do multiselect do
   * suporte, e não "nenhum". O default NÃO é vazio — ver [`defaultFilters`].
   */
  statuses: string[];
  /** Nomes de cliente que passam. Vazio = todos. */
  clients: string[];
  /** Nº do chamado, casamento exato. Vazio = qualquer. */
  ticket: string;
  /** Item tem chamado vinculado. */
  hasTicket: TriState;
  /** Recorte por prazo (ISO date `YYYY-MM-DD`, do input nativo). */
  deadlineFrom: string;
  deadlineTo: string;
  origin: OriginFilter;
}

export const EMPTY_FILTERS: TaskFilterState = {
  statuses: [],
  clients: [],
  ticket: "",
  hasTicket: "all",
  deadlineFrom: "",
  deadlineTo: "",
  origin: "all",
};

/**
 * Estado inicial: mostra o que a **origem** considera trabalho ativo.
 *
 * O default vem do catálogo (`default_filter`), não de uma lista fixa daqui.
 * Em consequência, `finalizado`, `cancelado` e `pos_atendimento` começam
 * escondidos, exatamente como no filtro do suporte — e um status novo criado no
 * Mastersys já entra com o comportamento que o admin definiu lá.
 *
 * Sem catálogo (nunca sincronizou) devolve `statuses: []`, que mostra tudo. É a
 * escolha certa para o caso vazio: esconder itens por falta de dado de
 * referência faria o quadro parecer quebrado.
 */
export function defaultFilters(
  catalog: MastersysTicketStatus[],
  scope: FilterScope = "mastersys",
): TaskFilterState {
  // Só o quadro de Chamados nasce com recorte de status.
  //
  // Em `local` não existe status de origem para recortar — a aba é de tarefas
  // que nunca vieram do Mastersys. Em `done` a aba já É o recorte (concluídas
  // + parados na origem), e herdar o default do quadro ativo esconderia
  // pós-atendimento e finalizado, que é justamente o que ela mostra.
  if (scope !== "mastersys") return { ...EMPTY_FILTERS };
  return {
    ...EMPTY_FILTERS,
    statuses: catalog.filter((s) => s.default_filter).map((s) => s.value),
  };
}

/**
 * Qual quadro está filtrando.
 *
 * Cada um tem default e preferência **próprios**: o recorte de status que faz
 * sentido na fila de chamados não faz sentido nenhum numa aba de tarefas
 * locais, e uma chave só de `localStorage` faria a escolha de uma aba vazar
 * nas outras.
 */
export type FilterScope = "local" | "mastersys" | "done";

/** Quantos filtros estão ativos — alimenta o badge do botão "Filtros". */
export function countActiveFilters(
  filters: TaskFilterState,
  catalog: MastersysTicketStatus[],
  scope: FilterScope = "mastersys",
): number {
  const base = defaultFilters(catalog, scope);
  let n = 0;
  // Status conta como ativo só quando difere do default, senão o badge
  // marcaria "1" numa tela recém-aberta.
  if (!sameSet(filters.statuses, base.statuses)) n += 1;
  if (filters.clients.length > 0) n += 1;
  if (filters.ticket.trim() !== "") n += 1;
  if (filters.hasTicket !== "all") n += 1;
  if (filters.deadlineFrom !== "" || filters.deadlineTo !== "") n += 1;
  if (filters.origin !== "all") n += 1;
  return n;
}

function sameSet(a: string[], b: string[]): boolean {
  if (a.length !== b.length) return false;
  const sb = new Set(b);
  return a.every((v) => sb.has(v));
}

/**
 * A tarefa está atrasada?
 *
 * Prazo vencido **e** não concluída **e** não parada na origem. O último termo é
 * o que faz um chamado em pós-atendimento parar de aparecer como atrasado: ele
 * está aguardando, e o prazo original não diz mais nada sobre urgência.
 */
export function isOverdue(task: Task, now: Date = new Date()): boolean {
  if (task.completed || task.external?.status_parked) return false;
  if (!task.deadline) return false;
  const dl = new Date(task.deadline);
  return !isNaN(dl.getTime()) && dl.getTime() <= now.getTime();
}

/** `true` quando a origem considera o item parado. Local nunca é. */
export function isParked(task: Task): boolean {
  return task.external?.status_parked === true;
}

/**
 * Número do chamado relacionado, seja pelo espelho ou pelo vínculo manual.
 *
 * O filtro tem de ver o mesmo chamado que o card mostra. Sem isto, uma tarefa
 * vinculada exibia `#991` no selo e desaparecia ao filtrar por 991 ou por "tem
 * chamado" — a UI afirmando uma coisa e o filtro outra.
 */
export function relatedTicket(task: Task): string | null {
  return task.external?.ticket ?? task.link?.ticket ?? null;
}

/** Cliente relacionado, pelo espelho ou pelo vínculo manual. */
export function relatedClient(task: Task): string | null {
  return task.external?.client ?? task.link?.client ?? null;
}

/**
 * Termo de busca casa a tarefa?
 *
 * Cobre os mesmos campos que a busca do suporte alcança em `TaskRepository`:
 * título, descrição, nome do cliente e número do chamado. Sem mínimo de
 * caracteres, como lá.
 */
export function matchesSearch(task: Task, rawTerm: string): boolean {
  const term = rawTerm.trim().toLowerCase();
  if (term === "") return true;

  // `#1042` e `1042` devem achar o mesmo chamado — o suporte aceita o `#` e
  // quem digita costuma incluir.
  const numeric = term.replace(/^#/, "");
  if (numeric !== "" && /^\d+$/.test(numeric) && relatedTicket(task) === numeric) {
    return true;
  }

  return [
    task.title,
    task.description,
    relatedClient(task) ?? "",
    // Status escrito pelo usuário é vocabulário dele: buscar por "aguardando
    // peça" precisa achar as tarefas que ele marcou assim.
    task.link?.custom_status ?? "",
  ].some((f) => f.toLowerCase().includes(term));
}

/**
 * O recorte de status pode decidir sobre este status?
 *
 * Só quando o catálogo da origem o conhece. Um status fora do catálogo é dado
 * que o MasterDesk não sabe classificar, e o default do quadro de Chamados é
 * montado **exclusivamente** com slugs do catálogo (ver [`defaultFilters`]) —
 * então recortar por ele esconderia o item em todas as abas de uma vez:
 * Chamados o corta no filtro, Concluídos só mostra concluído ou parado, e
 * Tarefas só mostra `external === null`. O trabalho desaparecia do app sem que
 * nada na tela explicasse por quê.
 *
 * O CASO CONCRETO (chamado 75249, 2026-09-04)
 * Chamado "Em Desenvolvimento" com tarefa no quadro do Mastersys. O espelho vem
 * da tarefa, e `MastersysTask::effective_status` só consegue usar o status do
 * chamado se a origem enviar `ticketStatus` — campo acrescentado ao `TaskDTO`
 * em 2026-09-03 e ainda não publicado em produção. Sem ele o espelho cai no
 * status da TAREFA (`in_progress`), que não existe em `ticket_statuses`, e o
 * chamado sumia do quadro inteiro.
 *
 * Sem `knownStatuses` (chamada que não tem o catálogo em mão) o recorte vale
 * para tudo, que é o comportamento anterior.
 */
function isStatusFilterable(
  status: string,
  knownStatuses?: readonly string[],
): boolean {
  return knownStatuses === undefined || knownStatuses.includes(status);
}

/**
 * Aplica um estado de filtro a uma tarefa.
 *
 * `knownStatuses` são os slugs que o catálogo da origem conhece. Quando
 * informado, um espelho cujo status **não** está no catálogo escapa do recorte
 * de status — ver [`isStatusFilterable`].
 */
export function matchesFilters(
  task: Task,
  filters: TaskFilterState,
  knownStatuses?: readonly string[],
): boolean {
  const ext = task.external;

  if (filters.origin === "local" && ext !== null) return false;
  if (filters.origin === "mastersys" && ext === null) return false;

  if (filters.statuses.length > 0) {
    // Tarefa local não tem status de origem. Ela sobrevive a um recorte de
    // status porque o filtro é sobre o vocabulário do Mastersys — esconder as
    // locais aqui seria efeito colateral, não intenção. Isso importa em
    // Concluídos, a lista onde locais e espelhos convivem.
    const status = ext?.status_label ?? "";
    if (
      ext !== null &&
      isStatusFilterable(status, knownStatuses) &&
      !filters.statuses.includes(status)
    ) {
      return false;
    }
  }

  // Chamado e cliente valem tanto pelo espelho quanto pelo vínculo manual —
  // ver `relatedTicket`.
  const ticket = relatedTicket(task);
  const client = relatedClient(task);

  if (filters.clients.length > 0) {
    if (!client || !filters.clients.includes(client)) return false;
  }

  const wantedTicket = filters.ticket.trim().replace(/^#/, "");
  if (wantedTicket !== "" && ticket !== wantedTicket) return false;

  if (filters.hasTicket === "yes" && !ticket) return false;
  if (filters.hasTicket === "no" && ticket) return false;

  if (filters.deadlineFrom !== "" || filters.deadlineTo !== "") {
    // Sem prazo nunca casa um recorte por prazo. O suporte faz o oposto
    // (`OR t.scheduled_at IS NULL`), mas lá o recorte é de agenda, onde "sem
    // agendamento" é caso comum; aqui pedir uma faixa de prazo é pedir os que
    // têm prazo.
    if (!task.deadline) return false;
    const dl = new Date(task.deadline);
    if (isNaN(dl.getTime())) return false;

    if (filters.deadlineFrom !== "") {
      const from = new Date(`${filters.deadlineFrom}T00:00:00`);
      if (dl < from) return false;
    }
    if (filters.deadlineTo !== "") {
      // Fim de dia inclusivo: quem escolhe "até 04/09" espera ver o item de
      // 04/09 às 09:00.
      const to = new Date(`${filters.deadlineTo}T23:59:59.999`);
      if (dl > to) return false;
    }
  }

  return true;
}

/** Filtro + busca, na ordem em que a UI aplica. */
export function applyTaskFilters(
  tasks: Task[],
  filters: TaskFilterState,
  search: string,
  knownStatuses?: readonly string[],
): Task[] {
  return tasks.filter(
    (t) => matchesFilters(t, filters, knownStatuses) && matchesSearch(t, search),
  );
}

/**
 * Clientes presentes nos itens carregados, em ordem alfabética.
 *
 * Derivado do que está no quadro, e não de um cadastro: o MasterNote não
 * sincroniza a lista de clientes do Mastersys, e oferecer no filtro um cliente
 * sem item algum seria um beco sem saída.
 */
export function clientsInTasks(tasks: Task[]): string[] {
  const set = new Set<string>();
  for (const t of tasks) {
    // Inclui o cliente do vínculo manual: ele é filtrável como qualquer
    // outro, e a lista precisa oferecer o que o filtro aceita.
    const client = relatedClient(t);
    if (client) set.add(client);
  }
  return [...set].sort((a, b) => a.localeCompare(b, "pt-BR"));
}

// ---------------------------------------------------------------------------
// Persistência
// ---------------------------------------------------------------------------

/**
 * Chave por aba. `mastersys` mantém o nome antigo porque herda o quadro que
 * existia — quem já tinha filtro configurado não o perde; as outras são novas.
 */
const STORAGE_KEYS: Record<FilterScope, string> = {
  mastersys: "masterdesk.task-filters",
  local: "masterdesk.task-filters.local",
  done: "masterdesk.task-filters.done",
};

/**
 * `localStorage`, não SQLite — seguindo o mesmo raciocínio que `theme.ts`
 * documenta para a preferência de tema.
 *
 * O filtro precisa valer no **primeiro render** da lista. Um ida-e-volta
 * assíncrono ao banco mostraria o quadro inteiro por um instante e só depois
 * esconderia os itens parados, ou seja, exatamente o piscar que se quer evitar.
 * Também é preferência per-máquina, então o banco não daria nada em troca.
 *
 * O termo de busca **não** é persistido, como no suporte: busca é intenção do
 * momento, filtro é configuração.
 */
export function loadFilters(
  catalog: MastersysTicketStatus[],
  scope: FilterScope = "mastersys",
): TaskFilterState {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEYS[scope]);
    if (!raw) return defaultFilters(catalog, scope);
    const parsed = JSON.parse(raw) as Partial<TaskFilterState>;
    // Mescla sobre o default em vez de confiar no gravado: uma versão anterior
    // pode não ter algum campo, e `undefined` num filtro quebraria o predicado.
    return { ...defaultFilters(catalog, scope), ...parsed };
  } catch {
    // JSON corrompido ou storage indisponível (modo privado, política de
    // grupo). Filtro é conveniência: cair no default é melhor que falhar.
    return defaultFilters(catalog, scope);
  }
}

export function saveFilters(
  filters: TaskFilterState,
  scope: FilterScope = "mastersys",
): void {
  try {
    window.localStorage.setItem(STORAGE_KEYS[scope], JSON.stringify(filters));
  } catch {
    // Ignorado de propósito: não gravar a preferência não pode impedir o
    // usuário de filtrar.
  }
}
