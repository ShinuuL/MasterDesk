import { describe, expect, it } from "vitest";
import {
  applyTaskFilters,
  clientsInTasks,
  countActiveFilters,
  defaultFilters,
  EMPTY_FILTERS,
  isOverdue,
  matchesFilters,
  matchesSearch,
} from "./filter";
import type { ExternalRef, MastersysTicketStatus, Task } from "../types";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/** Catálogo com os defaults reais da origem: os três parados fora do filtro. */
const CATALOG: MastersysTicketStatus[] = [
  st("novo", "Novo", true, false),
  st("em_atendimento", "Em Atendimento", true, false),
  st("pos_atendimento", "Pós Atendimento", false, false),
  st("finalizado", "Finalizado", false, true),
  st("cancelado", "Cancelado", false, true),
];

function st(
  value: string,
  label: string,
  default_filter: boolean,
  is_final: boolean,
): MastersysTicketStatus {
  return {
    value,
    label,
    color: "#3b82f6",
    default_filter,
    is_final,
    pauses_sla: false,
    display_order: 1,
  };
}

function task(over: Partial<Task> = {}): Task {
  return {
    id: "t1",
    title: "tarefa",
    description: "",
    priority: "Medium",
    deadline: null,
    reminder_thresholds: [],
    completed: false,
    external: null,
    link: null,
    created_at: "2026-09-01T10:00:00Z",
    updated_at: "2026-09-01T10:00:00Z",
    ...over,
  };
}

function ext(over: Partial<ExternalRef> = {}): ExternalRef {
  return {
    system: "Mastersys",
    kind: "Ticket",
    external_id: "ticket-75071",
    client: "Lua & Sol - Boutique",
    ticket: "75071",
    status_label: "em_atendimento",
    status_parked: false,
    role_analyst: false,
    role_attendant: false,
    ...over,
  };
}

/** O item exato do `melhoria.png`: chamado em pós-atendimento, prazo vencido. */
function posAtendimento(): Task {
  return task({
    title: "REL. FATURAMENTO VENDA INDICADORES LOJA",
    deadline: "2026-09-04T12:00:00Z",
    external: ext({ status_label: "pos_atendimento", status_parked: true }),
  });
}

const AFTER_DEADLINE = new Date("2026-09-10T12:00:00Z");

// ---------------------------------------------------------------------------
// Atraso — o caso que motivou a mudança
// ---------------------------------------------------------------------------

describe("isOverdue", () => {
  it("não marca item parado como atrasado, mesmo com prazo vencido", () => {
    expect(isOverdue(posAtendimento(), AFTER_DEADLINE)).toBe(false);
  });

  it("marca atrasado um item ativo com o mesmo prazo vencido", () => {
    // Muda só `status_parked`, para provar que é ele que decide.
    const active = task({
      deadline: "2026-09-04T12:00:00Z",
      external: ext({ status_label: "em_atendimento", status_parked: false }),
    });
    expect(isOverdue(active, AFTER_DEADLINE)).toBe(true);
  });

  it("não marca concluída nem sem prazo", () => {
    expect(
      isOverdue(task({ deadline: "2026-09-04T12:00:00Z", completed: true }), AFTER_DEADLINE),
    ).toBe(false);
    expect(isOverdue(task({ deadline: null }), AFTER_DEADLINE)).toBe(false);
  });

  it("não marca atrasado antes do prazo", () => {
    const t = task({ deadline: "2026-09-04T12:00:00Z" });
    expect(isOverdue(t, new Date("2026-09-01T12:00:00Z"))).toBe(false);
  });

  it("ignora prazo inválido em vez de tratar como vencido", () => {
    expect(isOverdue(task({ deadline: "nao-e-data" }), AFTER_DEADLINE)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Default do filtro — "só aparecer se o usuário selecionar"
// ---------------------------------------------------------------------------

describe("defaultFilters", () => {
  it("esconde pós-atendimento, finalizado e cancelado por padrão", () => {
    const f = defaultFilters(CATALOG);
    expect(f.statuses).toEqual(["novo", "em_atendimento"]);
    expect(matchesFilters(posAtendimento(), f)).toBe(false);
  });

  it("revela pós-atendimento quando o usuário marca aquele status", () => {
    const f = { ...defaultFilters(CATALOG), statuses: ["pos_atendimento"] };
    expect(matchesFilters(posAtendimento(), f)).toBe(true);
  });

  it("sem catálogo mostra tudo, em vez de esconder por falta de dado", () => {
    const f = defaultFilters([]);
    expect(f.statuses).toEqual([]);
    expect(matchesFilters(posAtendimento(), f)).toBe(true);
  });

  it("um status novo criado na origem herda o default do admin", () => {
    const withNew = [...CATALOG, st("aguardando_peca", "Aguardando Peça", false, false)];
    expect(defaultFilters(withNew).statuses).not.toContain("aguardando_peca");
  });
});

describe("countActiveFilters", () => {
  it("é zero na tela recém-aberta, apesar de statuses estar preenchido", () => {
    expect(countActiveFilters(defaultFilters(CATALOG), CATALOG)).toBe(0);
  });

  it("conta status alterado, cliente, chamado, vínculo, prazo e origem", () => {
    const f = {
      ...defaultFilters(CATALOG),
      statuses: ["novo"],
      clients: ["Lua & Sol - Boutique"],
      ticket: "75071",
      hasTicket: "yes" as const,
      deadlineFrom: "2026-09-01",
      origin: "mastersys" as const,
    };
    expect(countActiveFilters(f, CATALOG)).toBe(6);
  });

  it("não conta status quando é o mesmo conjunto em outra ordem", () => {
    const f = { ...defaultFilters(CATALOG), statuses: ["em_atendimento", "novo"] };
    expect(countActiveFilters(f, CATALOG)).toBe(0);
  });
});

// ---------------------------------------------------------------------------
// Busca
// ---------------------------------------------------------------------------

describe("matchesSearch", () => {
  const t = task({
    title: "REL. FATURAMENTO",
    description: "análise do relatório de vendas",
    external: ext(),
  });

  it("casa título, descrição e cliente, sem diferenciar caixa", () => {
    expect(matchesSearch(t, "faturamento")).toBe(true);
    expect(matchesSearch(t, "RELATÓRIO".toLowerCase())).toBe(true);
    expect(matchesSearch(t, "lua & sol")).toBe(true);
  });

  it("casa o nº do chamado com e sem #", () => {
    expect(matchesSearch(t, "75071")).toBe(true);
    expect(matchesSearch(t, "#75071")).toBe(true);
  });

  it("não casa número que só aparece parcialmente no chamado", () => {
    expect(matchesSearch(t, "7507")).toBe(false);
  });

  it("termo vazio passa tudo", () => {
    expect(matchesSearch(t, "   ")).toBe(true);
  });

  it("não casa termo ausente", () => {
    expect(matchesSearch(t, "zzz")).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// Demais filtros
// ---------------------------------------------------------------------------

describe("matchesFilters", () => {
  it("filtra por cliente", () => {
    const f = { ...EMPTY_FILTERS, clients: ["Outro Cliente"] };
    expect(matchesFilters(task({ external: ext() }), f)).toBe(false);
    expect(
      matchesFilters(task({ external: ext({ client: "Outro Cliente" }) }), f),
    ).toBe(true);
  });

  it("filtra por nº do chamado, aceitando #", () => {
    const t = task({ external: ext() });
    expect(matchesFilters(t, { ...EMPTY_FILTERS, ticket: "75071" })).toBe(true);
    expect(matchesFilters(t, { ...EMPTY_FILTERS, ticket: "#75071" })).toBe(true);
    expect(matchesFilters(t, { ...EMPTY_FILTERS, ticket: "1" })).toBe(false);
  });

  it("filtra por vínculo com chamado", () => {
    const withTicket = task({ external: ext() });
    const local = task();
    expect(matchesFilters(withTicket, { ...EMPTY_FILTERS, hasTicket: "yes" })).toBe(true);
    expect(matchesFilters(local, { ...EMPTY_FILTERS, hasTicket: "yes" })).toBe(false);
    expect(matchesFilters(local, { ...EMPTY_FILTERS, hasTicket: "no" })).toBe(true);
  });

  it("filtra por origem", () => {
    expect(matchesFilters(task(), { ...EMPTY_FILTERS, origin: "local" })).toBe(true);
    expect(
      matchesFilters(task({ external: ext() }), { ...EMPTY_FILTERS, origin: "local" }),
    ).toBe(false);
    expect(
      matchesFilters(task({ external: ext() }), { ...EMPTY_FILTERS, origin: "mastersys" }),
    ).toBe(true);
  });

  it("recorte de prazo inclui o dia final inteiro", () => {
    // Item às 09:00 de 04/09 tem de passar em "até 04/09".
    const t = task({ deadline: "2026-09-04T09:00:00Z" });
    expect(
      matchesFilters(t, { ...EMPTY_FILTERS, deadlineTo: "2026-09-04" }),
    ).toBe(true);
    expect(
      matchesFilters(t, { ...EMPTY_FILTERS, deadlineTo: "2026-09-03" }),
    ).toBe(false);
    expect(
      matchesFilters(t, { ...EMPTY_FILTERS, deadlineFrom: "2026-09-05" }),
    ).toBe(false);
  });

  it("item sem prazo não passa em recorte por prazo", () => {
    expect(
      matchesFilters(task({ deadline: null }), {
        ...EMPTY_FILTERS,
        deadlineFrom: "2026-09-01",
      }),
    ).toBe(false);
  });

  it("tarefa local sobrevive a um recorte de status", () => {
    // Status é vocabulário do Mastersys; esconder as locais aí seria efeito
    // colateral. Quem quer só espelhos usa o filtro Origem.
    const f = defaultFilters(CATALOG);
    expect(matchesFilters(task(), f)).toBe(true);
  });
});

describe("applyTaskFilters", () => {
  it("aplica filtro e busca juntos", () => {
    const list = [
      task({ id: "a", title: "alfa", external: ext({ status_label: "novo" }) }),
      task({ id: "b", title: "beta", external: ext({ status_label: "novo" }) }),
      posAtendimento(),
    ];
    const out = applyTaskFilters(list, defaultFilters(CATALOG), "alfa");
    expect(out.map((t) => t.id)).toEqual(["a"]);
  });
});

describe("clientsInTasks", () => {
  it("deduplica, ignora locais e ordena em pt-BR", () => {
    const list = [
      task({ external: ext({ client: "Zeta" }) }),
      task({ external: ext({ client: "Ácme" }) }),
      task({ external: ext({ client: "Zeta" }) }),
      task(),
      task({ external: ext({ client: null }) }),
    ];
    expect(clientsInTasks(list)).toEqual(["Ácme", "Zeta"]);
  });
});

// ---------------------------------------------------------------------------
// Vínculo manual — a tarefa é local, mas tem chamado e cliente
// ---------------------------------------------------------------------------

/** Tarefa local vinculada a um chamado pelo próprio usuário. */
function linked(over: Partial<Task> = {}): Task {
  return task({
    id: "vinc",
    title: "trocar fonte",
    link: { ticket: "991", client: "Padaria Central", custom_status: "aguardando peça" },
    ...over,
  });
}

describe("tarefa com vínculo manual", () => {
  it("é encontrada pelo número do chamado, com ou sem #", () => {
    expect(matchesSearch(linked(), "991")).toBe(true);
    expect(matchesSearch(linked(), "#991")).toBe(true);
    expect(matchesSearch(linked(), "992")).toBe(false);
  });

  it("é encontrada pelo cliente e pelo status personalizado", () => {
    expect(matchesSearch(linked(), "padaria")).toBe(true);
    expect(matchesSearch(linked(), "aguardando peça")).toBe(true);
  });

  it("conta como 'tem chamado' — o card mostra #991, o filtro tem de concordar", () => {
    expect(matchesFilters(linked(), { ...EMPTY_FILTERS, hasTicket: "yes" })).toBe(true);
    expect(matchesFilters(linked(), { ...EMPTY_FILTERS, hasTicket: "no" })).toBe(false);
    expect(matchesFilters(linked(), { ...EMPTY_FILTERS, ticket: "991" })).toBe(true);
    expect(matchesFilters(linked(), { ...EMPTY_FILTERS, ticket: "12" })).toBe(false);
  });

  it("continua sendo de origem local, não espelho", () => {
    expect(matchesFilters(linked(), { ...EMPTY_FILTERS, origin: "local" })).toBe(true);
    expect(matchesFilters(linked(), { ...EMPTY_FILTERS, origin: "mastersys" })).toBe(false);
  });

  it("sobrevive ao filtro de status do Mastersys, como qualquer local", () => {
    // O status personalizado não pertence ao vocabulário da origem, então um
    // recorte por status de chamado não pode escondê-la.
    expect(matchesFilters(linked(), defaultFilters(CATALOG))).toBe(true);
  });

  it("aparece na lista de clientes do filtro", () => {
    expect(clientsInTasks([linked(), task({ external: ext({ client: "Zeta" }) })])).toEqual([
      "Padaria Central",
      "Zeta",
    ]);
  });

  it("nunca é tratada como parada — parado é estado da origem", () => {
    expect(matchesFilters(linked(), { ...EMPTY_FILTERS, hasTicket: "yes" })).toBe(true);
    expect(isOverdue(linked({ deadline: "2020-01-01T00:00:00Z" }))).toBe(true);
  });
});

describe("default por aba (FilterScope)", () => {
  it("no quadro ativo, começa com os status que a origem chama de trabalho ativo", () => {
    expect(defaultFilters(CATALOG, "mastersys").statuses).toEqual([
      "novo",
      "em_atendimento",
    ]);
  });

  it("na aba Concluídos, começa SEM recorte de status", () => {
    // A aba já é o recorte (concluídas + parados na origem). Herdar o default
    // do quadro ativo esconderia pós-atendimento e finalizado — exatamente o
    // que ela existe para mostrar.
    expect(defaultFilters(CATALOG, "done").statuses).toEqual([]);
    expect(countActiveFilters(defaultFilters(CATALOG, "done"), CATALOG, "done")).toBe(0);
  });

  it("o mesmo filtro conta como ativo ou não conforme a aba", () => {
    const semStatus = { ...EMPTY_FILTERS };
    // No quadro ativo, "nenhum status marcado" é um desvio do default.
    expect(countActiveFilters(semStatus, CATALOG, "mastersys")).toBe(1);
    // Na aba Concluídos é o próprio default.
    expect(countActiveFilters(semStatus, CATALOG, "done")).toBe(0);
  });

  it("um espelho em pós-atendimento passa pelo filtro da aba Concluídos", () => {
    expect(matchesFilters(posAtendimento(), defaultFilters(CATALOG, "done"))).toBe(true);
    // E é escondido no quadro ativo, como antes.
    expect(matchesFilters(posAtendimento(), defaultFilters(CATALOG, "mastersys"))).toBe(false);
  });
});

describe("escopo local (aba Tarefas)", () => {
  it("começa sem recorte de status — não existe status de origem para recortar", () => {
    expect(defaultFilters(CATALOG, "local").statuses).toEqual([]);
    expect(countActiveFilters(defaultFilters(CATALOG, "local"), CATALOG, "local")).toBe(0);
  });

  it("tarefa vinculada continua sendo de origem local, e é o que a aba mostra", () => {
    // A partição das abas é estrutural (`external === null`), mas o filtro de
    // origem tem de concordar com ela para a aba Concluídos não mentir.
    expect(matchesFilters(linked(), { ...EMPTY_FILTERS, origin: "local" })).toBe(true);
    expect(matchesFilters(linked(), defaultFilters(CATALOG, "local"))).toBe(true);
  });
});
