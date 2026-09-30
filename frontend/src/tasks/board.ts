/**
 * Colunas do quadro Kanban — lógica pura, sem React.
 *
 * Quatro colunas visíveis sobre dois dados persistidos:
 * - `completed` → **Concluído** (sempre vence);
 * - `board_column` → **A fazer / Em andamento / Aguardando**.
 *
 * Tudo aqui é local (decisão do DEV em 2026-09-30): mover um card não escreve
 * nada no Mastersys.
 *
 * ## Item parado na origem não muda de coluna sozinho
 *
 * Foi considerado mostrar em "Aguardando" o espelho parado (`status_parked`)
 * que o usuário nunca moveu. Descartado: com `board_column` ainda em `todo`,
 * arrastar esse card para "A fazer" gravaria `todo` de novo e ele não sairia
 * do lugar — exigiria um estado "automático" escondido no banco. A coluna é
 * sempre a escolha do usuário; o card mostra o selo "Parado".
 */

import type { BoardColumn, Task } from "../types";

export type ColumnId = BoardColumn | "done";

export interface ColumnDef {
  id: ColumnId;
  title: string;
}

export const COLUMNS: readonly ColumnDef[] = [
  { id: "todo", title: "A fazer" },
  { id: "doing", title: "Em andamento" },
  { id: "waiting", title: "Aguardando" },
  { id: "done", title: "Concluído" },
];

export function columnOf(task: Task): ColumnId {
  if (task.completed) return "done";
  return task.board_column ?? "todo";
}

export function groupByColumn(tasks: readonly Task[]): Record<ColumnId, Task[]> {
  const out: Record<ColumnId, Task[]> = { todo: [], doing: [], waiting: [], done: [] };
  for (const t of tasks) out[columnOf(t)].push(t);
  return out;
}

/**
 * O que fazer para levar um card até `target`. Separado da chamada à API para
 * ser testável: concluir/reabrir são operações próprias (cancelam/reagendam
 * lembretes), mudar de coluna aberta é só `board_column`.
 */
export type MovePlan =
  | { kind: "none" }
  | { kind: "complete" }
  | { kind: "reopen"; column: BoardColumn }
  | { kind: "column"; column: BoardColumn };

export function planMove(task: Task, target: ColumnId): MovePlan {
  const current = columnOf(task);
  if (current === target) return { kind: "none" };
  if (target === "done") return { kind: "complete" };
  if (task.completed) return { kind: "reopen", column: target };
  return { kind: "column", column: target };
}
