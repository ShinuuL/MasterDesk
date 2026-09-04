import { useEffect, useRef, useState } from "react";
import type { MastersysTicketStatus } from "../types";
import {
  countActiveFilters,
  defaultFilters,
  type FilterScope,
  type TaskFilterState,
  type TriState,
} from "../tasks/filter";
import { StatusBadge } from "./StatusBadge";

interface Props {
  filters: TaskFilterState;
  onChange: (next: TaskFilterState) => void;
  catalog: MastersysTicketStatus[];
  /** Clientes presentes nos itens carregados. */
  clients: string[];
  /** Termo de busca, já controlado pelo pai (que faz o debounce). */
  searchInput: string;
  onSearchInput: (value: string) => void;
  /** Ação de busca ao vivo, habilitada a partir de 3 caracteres. */
  onRemoteSearch?: () => void;
  remoteSearching?: boolean;
  /** Qual aba filtra — decide o default contra o qual "ativo" é medido. */
  scope?: FilterScope;
}

/** Mínimo do `GET /api/tickets/search` do Mastersys. */
const REMOTE_SEARCH_MIN = 3;

/**
 * Filtros do quadro de tarefas.
 *
 * Espelha o `TaskFiltersPanel` do Mastersys de propósito — mesmos rótulos,
 * mesmo popover com badge de contagem, mesmo `Limpar filtros` no pé — para
 * quem atende não ter de aprender dois vocabulários. As diferenças são
 * deliberadas e estão documentadas em `tasks/filter.ts`.
 */
export function TaskFilters({
  filters,
  onChange,
  catalog,
  clients,
  searchInput,
  onSearchInput,
  onRemoteSearch,
  remoteSearching,
  scope = "mastersys",
}: Props) {
  const [open, setOpen] = useState(false);
  const popoverRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const active = countActiveFilters(filters, catalog, scope);
  const canRemoteSearch =
    onRemoteSearch !== undefined &&
    searchInput.trim().length >= REMOTE_SEARCH_MIN;

  // Fecha em clique-fora e Esc, como o popover do suporte. O listener só existe
  // enquanto aberto — registrar sempre custaria um handler global à toa.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      const t = e.target as Node;
      if (popoverRef.current?.contains(t) || buttonRef.current?.contains(t)) {
        return;
      }
      setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setOpen(false);
        buttonRef.current?.focus();
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const set = <K extends keyof TaskFilterState>(
    key: K,
    value: TaskFilterState[K],
  ) => onChange({ ...filters, [key]: value });

  const toggleInList = (key: "statuses" | "clients", value: string) => {
    const list = filters[key];
    set(
      key,
      list.includes(value) ? list.filter((v) => v !== value) : [...list, value],
    );
  };

  return (
    <div className="md-taskfilters">
      <div className="md-taskfilters-bar">
        <div className="md-search">
          <input
            className="md-input md-search-input"
            value={searchInput}
            onChange={(e) => onSearchInput(e.target.value)}
            placeholder="Buscar por título, descrição, cliente, nº chamado..."
            aria-label="Buscar tarefas"
          />
          {searchInput !== "" && (
            <button
              className="md-search-clear"
              onClick={() => onSearchInput("")}
              title="Limpar busca"
              aria-label="Limpar busca"
            >
              ✕
            </button>
          )}
        </div>

        {onRemoteSearch && (
          <button
            className="md-btn md-btn--ghost"
            onClick={onRemoteSearch}
            disabled={!canRemoteSearch || remoteSearching}
            title={
              canRemoteSearch
                ? "Consultar o acervo do Mastersys, inclusive chamados fora da sua fila"
                : `Digite ao menos ${REMOTE_SEARCH_MIN} caracteres`
            }
          >
            {remoteSearching ? "Buscando…" : "Buscar no Mastersys"}
          </button>
        )}

        <button
          ref={buttonRef}
          className={`md-btn ${active > 0 ? "md-btn--primary" : "md-btn--ghost"}`}
          onClick={() => setOpen((v) => !v)}
          aria-expanded={open}
        >
          Filtros
          {active > 0 && <span className="md-filter-count">{active}</span>}
        </button>
      </div>

      {open && (
        <div ref={popoverRef} className="md-filter-popover" role="dialog" aria-label="Filtros">
          {/* Status é vocabulário do Mastersys: numa aba de tarefas locais não
              há um único item que o tenha, e o recorte não filtraria nada. */}
          {catalog.length > 0 && scope !== "local" && (
            <fieldset className="md-filter-group">
              <legend>Status</legend>
              <div className="md-filter-chips">
                {catalog.map((s) => {
                  const on = filters.statuses.includes(s.value);
                  return (
                    <button
                      key={s.value}
                      className={`md-filter-chip ${on ? "md-filter-chip--on" : ""}`}
                      onClick={() => toggleInList("statuses", s.value)}
                      aria-pressed={on}
                    >
                      <StatusBadge
                        statusLabel={s.value}
                        catalog={catalog}
                        parked={!s.default_filter}
                      />
                    </button>
                  );
                })}
              </div>
              <p className="md-filter-hint">
                Nada marcado mostra todos. Pós-atendimento, finalizado e
                cancelado começam de fora, como no filtro do suporte.
              </p>
            </fieldset>
          )}

          {clients.length > 0 && (
            <fieldset className="md-filter-group">
              <legend>Cliente</legend>
              <div className="md-filter-chips">
                {clients.map((c) => {
                  const on = filters.clients.includes(c);
                  return (
                    <button
                      key={c}
                      className={`md-filter-chip ${on ? "md-filter-chip--on" : ""}`}
                      onClick={() => toggleInList("clients", c)}
                      aria-pressed={on}
                      title={c}
                    >
                      {c}
                    </button>
                  );
                })}
              </div>
            </fieldset>
          )}

          <div className="md-filter-row">
            <label className="md-filter-field">
              <span>Nº do chamado</span>
              <input
                className="md-input"
                inputMode="numeric"
                value={filters.ticket}
                onChange={(e) => set("ticket", e.target.value)}
                placeholder="Ex.: 1042"
              />
            </label>

            <label className="md-filter-field">
              <span>Vínculo com chamado</span>
              <select
                className="md-input"
                value={filters.hasTicket}
                onChange={(e) => set("hasTicket", e.target.value as TriState)}
              >
                <option value="all">Todas</option>
                <option value="yes">Com chamado</option>
                <option value="no">Sem chamado</option>
              </select>
            </label>
          </div>

          <div className="md-filter-row">
            {/* "Prazo" e não "Agendada": o MasterNote colapsou previsão,
                agendamento do chamado e agendamento da tarefa num único prazo e
                não sabe qual venceu — ver `effective_due_date`. Manter o rótulo
                do suporte mentiria sobre o que está sendo filtrado. */}
            <label className="md-filter-field">
              <span>Prazo de</span>
              <input
                className="md-input"
                type="date"
                value={filters.deadlineFrom}
                onChange={(e) => set("deadlineFrom", e.target.value)}
              />
            </label>
            <label className="md-filter-field">
              <span>Prazo até</span>
              <input
                className="md-input"
                type="date"
                value={filters.deadlineTo}
                onChange={(e) => set("deadlineTo", e.target.value)}
              />
            </label>
          </div>

          {/* `Origem` só existe onde as duas convivem — hoje, a aba
              Concluídos. Nas abas Tarefas e Chamados a separação já é
              estrutural, e oferecer o filtro ali daria ao usuário um jeito de
              esvaziar o próprio quadro ("Mastersys" em Tarefas não casa com
              nada) sem que a tela explique o porquê. */}
          {scope === "done" && (
            <div className="md-filter-row">
              <label className="md-filter-field">
                <span>Origem</span>
                <select
                  className="md-input"
                  value={filters.origin}
                  onChange={(e) =>
                    set("origin", e.target.value as TaskFilterState["origin"])
                  }
                >
                  <option value="all">Todas</option>
                  <option value="local">Locais</option>
                  <option value="mastersys">Mastersys</option>
                </select>
              </label>
            </div>
          )}

          <div className="md-filter-foot">
            <button
              className="md-btn md-btn--ghost"
              onClick={() => onChange(defaultFilters(catalog, scope))}
            >
              Limpar filtros
            </button>
            <span className="md-filter-hint">Os filtros ficam salvos para você.</span>
          </div>
        </div>
      )}
    </div>
  );
}
