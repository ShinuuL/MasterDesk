import { describe, expect, it } from "vitest";
import type { Task } from "../types";
import { columnOf, groupByColumn, planMove } from "./board";

function task(over: Partial<Task> = {}): Task {
  return {
    id: "t",
    title: "t",
    description: "",
    priority: "Medium",
    deadline: null,
    reminder_thresholds: [],
    completed: false,
    external: null,
    link: null,
    board_column: "todo",
    created_at: "2026-09-30T00:00:00Z",
    updated_at: "2026-09-30T00:00:00Z",
    ...over,
  } as Task;
}

const parked = { status_parked: true } as Task["external"];

describe("columnOf", () => {
  it("concluída vai para Concluído, qualquer que seja a coluna salva", () => {
    expect(columnOf(task({ completed: true, board_column: "doing" }))).toBe("done");
  });
  it("usa a coluna salva", () => {
    expect(columnOf(task({ board_column: "doing" }))).toBe("doing");
  });
  it("item parado na origem não muda de coluna sozinho", () => {
    expect(columnOf(task({ external: parked }))).toBe("todo");
    expect(columnOf(task({ external: parked, board_column: "doing" }))).toBe("doing");
  });
  it("sem o campo (backend antigo) cai em A fazer", () => {
    expect(columnOf(task({ board_column: undefined as never }))).toBe("todo");
  });
});

describe("groupByColumn", () => {
  it("distribui nas quatro colunas", () => {
    const g = groupByColumn([
      task({ id: "a" }),
      task({ id: "b", board_column: "doing" }),
      task({ id: "c", completed: true }),
    ]);
    expect(g.todo.map((t) => t.id)).toEqual(["a"]);
    expect(g.doing.map((t) => t.id)).toEqual(["b"]);
    expect(g.waiting).toEqual([]);
    expect(g.done.map((t) => t.id)).toEqual(["c"]);
  });
});

describe("planMove", () => {
  it("mesma coluna não faz nada", () => {
    expect(planMove(task(), "todo")).toEqual({ kind: "none" });
  });
  it("para Concluído conclui", () => {
    expect(planMove(task(), "done")).toEqual({ kind: "complete" });
  });
  it("de Concluído para uma coluna reabre já na coluna", () => {
    expect(planMove(task({ completed: true }), "doing")).toEqual({ kind: "reopen", column: "doing" });
  });
  it("entre colunas abertas só troca a coluna", () => {
    expect(planMove(task(), "waiting")).toEqual({ kind: "column", column: "waiting" });
  });
  it("todo movimento entre colunas abertas tem efeito visível", () => {
    const t = task({ external: parked, board_column: "waiting" });
    expect(planMove(t, "todo")).toEqual({ kind: "column", column: "todo" });
  });
});
