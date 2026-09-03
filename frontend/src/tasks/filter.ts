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
 * é pessoal). Entrou um filtro que o suporte não tem — `Origem` — porque este
 * quadro mistura tarefas locais com espelhos.
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
): TaskFilterState {
  return {
    ...EMPTY_FILTERS,
    statuses: catalog.filter((s) => s.default_filter).map((s) => s.value),
  };
}

/** Quantos filtros estão ativos — alimenta o badge do botão "Filtros". */
export function countActiveFilters(
  filters: TaskFilterState,
  catalog: MastersysTicketStatus[],
): number {
  const base = defaultFilters(catalog);
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
  if (numeric !== "" && /^\d+$/.test(numeric) && task.external?.ticket === numeric) {
    return true;
  }

  return [task.title, task.description, task.external?.client ?? ""].some((f) =>
    f.toLowerCase().includes(term),
  );
}

/** Aplica um estado de filtro a uma tarefa. */
export function matchesFilters(task: Task, filters: TaskFilterState): boolean {
  const ext = task.external;

  if (filters.origin === "local" && ext !== null) return false;
  if (filters.origin === "mastersys" && ext === null) return false;

  if (filters.statuses.length > 0) {
    // Tarefa local não tem status de origem. Ela sobrevive a um recorte de
    // status porque o filtro é sobre o vocabulário do Mastersys — esconder as
    // locais aqui seria efeito colateral, não intenção. Quem quer só espelhos
    // usa o filtro `Origem`.
    if (ext !== null && !filters.statuses.includes(ext.status_label ?? "")) {
      return false;
    }
  }

  if (filters.clients.length > 0) {
    if (!ext?.client || !filters.clients.includes(ext.client)) return false;
  }

  const wantedTicket = filters.ticket.trim().replace(/^#/, "");
  if (wantedTicket !== "" && ext?.ticket !== wantedTicket) return false;

  if (filters.hasTicket === "yes" && !ext?.ticket) return false;
  if (filters.hasTicket === "no" && ext?.ticket) return false;

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
): Task[] {
  return tasks.filter(
    (t) => matchesFilters(t, filters) && matchesSearch(t, search),
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
    if (t.external?.client) set.add(t.external.client);
  }
  return [...set].sort((a, b) => a.localeCompare(b, "pt-BR"));
}

// ---------------------------------------------------------------------------
// Persistência
// ---------------------------------------------------------------------------

const STORAGE_KEY = "masterdesk.task-filters";

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
export function loadFilters(catalog: MastersysTicketStatus[]): TaskFilterState {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return defaultFilters(catalog);
    const parsed = JSON.parse(raw) as Partial<TaskFilterState>;
    // Mescla sobre o default em vez de confiar no gravado: uma versão anterior
    // pode não ter algum campo, e `undefined` num filtro quebraria o predicado.
    return { ...defaultFilters(catalog), ...parsed };
  } catch {
    // JSON corrompido ou storage indisponível (modo privado, política de
    // grupo). Filtro é conveniência: cair no default é melhor que falhar.
    return defaultFilters(catalog);
  }
}

export function saveFilters(filters: TaskFilterState): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(filters));
  } catch {
    // Ignorado de propósito: não gravar a preferência não pode impedir o
    // usuário de filtrar.
  }
}
