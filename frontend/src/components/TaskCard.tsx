import { memo } from "react";
import type { Priority, Task } from "../types";
import { isOverdue, isParked } from "../tasks/filter";

export const PRIORITY_LABEL: Record<Priority, string> = {
  Low: "Baixa",
  Medium: "Média",
  High: "Alta",
  Urgent: "Urgente",
};

const PRIORITY_VAR: Record<Priority, string> = {
  Low: "var(--prio-low)",
  Medium: "var(--prio-medium)",
  High: "var(--prio-high)",
  Urgent: "var(--prio-urgent)",
};

/** Janela de "vence em breve" — a mesma do card antigo. */
const SOON_MS = 30 * 60 * 1000;

function dueLabel(t: Task, now: number): { text: string; tone: "late" | "soon" | "parked" | "plain" } | null {
  if (!t.deadline) return null;
  const d = new Date(t.deadline);
  if (isNaN(d.getTime())) return null;
  if (t.completed) {
    return { text: d.toLocaleDateString("pt-BR", { day: "2-digit", month: "short" }), tone: "plain" };
  }
  // A regra de atraso é a do filtro (`isOverdue`): item parado na origem não
  // atrasa. Uma regra só, aqui e no filtro "Atrasados".
  if (isParked(t)) return { text: "Parado", tone: "parked" };
  if (isOverdue(t)) return { text: "Atrasado", tone: "late" };
  const sameDay = new Date(now).toDateString() === d.toDateString();
  const time = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  if (d.getTime() - now <= SOON_MS) return { text: `Vence ${time}`, tone: "soon" };
  return {
    text: sameDay ? `Hoje ${time}` : d.toLocaleDateString("pt-BR", { day: "2-digit", month: "short" }),
    tone: "plain",
  };
}

interface Props {
  task: Task;
  selected: boolean;
  poppedOut: boolean;
  notes: number;
  /** `Date.now()` do render do quadro — calculado uma vez, não por card. */
  now: number;
  onSelect: (id: string) => void;
}

/**
 * Card compacto do quadro Kanban. Só mostra; toda ação mora no painel de
 * detalhe, para o quadro continuar legível com dezenas de itens.
 *
 * `memo` com props primitivas: digitar na busca ou trocar a seleção não
 * redesenha os cards que não mudaram (achado de desempenho da auditoria).
 */
export const TaskCard = memo(function TaskCard({ task: t, selected, poppedOut, notes, now, onSelect }: Props) {
  const ticket = t.external?.ticket ?? t.link?.ticket ?? null;
  const origin = t.external ? (ticket ? `Chamado #${ticket}` : "Mastersys") : ticket ? `Local · #${ticket}` : "Local";
  const client = t.external?.client ?? t.link?.client ?? null;
  const due = dueLabel(t, now);

  return (
    <button
      type="button"
      className="md-kcard"
      aria-pressed={selected}
      onClick={() => onSelect(t.id)}
      aria-label={`${t.title} — ${origin}, prioridade ${PRIORITY_LABEL[t.priority]}${due ? `, ${due.text}` : ""}`}
    >
      <span className="md-kcard-top">
        <span className={`md-pill ${t.external ? "md-pill--ticket" : ""}`}>{origin}</span>
        {poppedOut && <span className="md-pill md-pill--quiet">Destacada</span>}
        <span style={{ flex: 1 }} />
        {due && <span className={`md-kcard-due md-kcard-due--${due.tone}`}>{due.text}</span>}
      </span>
      <span className="md-kcard-title">{t.title}</span>
      <span className="md-kcard-meta">
        <span className="md-kcard-dot" style={{ background: PRIORITY_VAR[t.priority] }} aria-hidden />
        <span>{PRIORITY_LABEL[t.priority]}</span>
        {client && (
          <>
            <span aria-hidden>·</span>
            <span className="md-kcard-client" title={client}>{client}</span>
          </>
        )}
        <span style={{ flex: 1 }} />
        {notes > 0 && <span>{notes} {notes === 1 ? "anotação" : "anotações"}</span>}
      </span>
    </button>
  );
});
