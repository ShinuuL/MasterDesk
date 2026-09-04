import { useEffect, useMemo, useState } from "react";
import type { ExternalWorkItem, MastersysTicketStatus, Task, Priority } from "../types";
import * as api from "../api";
import { TaskNotes } from "./TaskNotes";
import { MastersysPanel } from "./MastersysPanel";
import { TaskFilters } from "./TaskFilters";
import { StatusBadge } from "./StatusBadge";
import { TaskOriginStamp } from "./TaskOriginStamp";
import { TaskFormModal } from "./TaskFormModal";
import { TicketModal } from "./TicketModal";
import {
  applyTaskFilters,
  clientsInTasks,
  isOverdue,
  isParked,
  loadFilters,
  saveFilters,
  type FilterScope,
  type TaskFilterState,
} from "../tasks/filter";

const PRIORITY_VAR: Record<Priority, string> = {
  Low: "var(--prio-low)",
  Medium: "var(--prio-medium)",
  High: "var(--prio-high)",
  Urgent: "var(--prio-urgent)",
};

const PRIORITY_LABEL: Record<Priority, string> = {
  Low: "Baixa",
  Medium: "Média",
  High: "Alta",
  Urgent: "Urgente",
};

function thresholdMinutes(t: unknown): number {
  if (t && typeof t === "object") {
    const obj = t as Record<string, unknown>;
    if ("Minutes" in obj && typeof obj.Minutes === "number") return obj.Minutes;
    if ("Hours" in obj && typeof obj.Hours === "number") return (obj.Hours as number) * 60;
    if ("Custom" in obj) {
      const c = obj.Custom as Record<string, unknown>;
      if (c && typeof c.minutes_before === "number") return c.minutes_before;
    }
  }
  return 0;
}

function formatDeadline(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

/** Qual das três abas de trabalho este quadro está desenhando. */
export type BoardView = "local" | "mastersys" | "completed";

interface Props {
  /**
   * Qual recorte este quadro mostra.
   *
   * | `view`       | aba        | conteúdo |
   * |--------------|------------|----------|
   * | `local`      | Tarefas    | tarefas suas, pendentes (com ou sem vínculo manual) |
   * | `mastersys`  | Chamados   | espelhos da sua fila que a origem considera ativos |
   * | `completed`  | Concluídos | concluídas + espelhos parados na origem |
   *
   * Um componente para as três porque filtro, busca, catálogo de status,
   * anotações, pop-out e reconciliação de janelas valem igual — duplicá-lo
   * faria as telas divergirem na primeira correção feita só de um lado.
   */
  view: BoardView;
}

export function TasksBoard({ view }: Props) {
  const [pending, setPending] = useState<Task[]>([]);
  const [completed, setCompleted] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  /** Diálogo de criação aberto: tarefa comum ou vinculada a um chamado. */
  const [creating, setCreating] = useState<null | "plain" | "link">(null);
  /** Tarefa cujo chamado está aberto para leitura. */
  const [ticketOf, setTicketOf] = useState<Task | null>(null);
  /**
   * Chamado que a nova tarefa vinculada já vem apontando.
   *
   * O "vincular" nasce a partir de um card — o chamado é o daquele item, não
   * algo a redigitar. Quem precisa vincular a um chamado que não está no
   * quadro usa o interruptor dentro de "Nova tarefa", que oferece a busca.
   */
  const [linkSeed, setLinkSeed] = useState<{ ticket: string; client: string | null } | null>(null);

  /** Preferências de filtro são por aba — ver `FilterScope`. */
  const scope: FilterScope = view === "completed" ? "done" : view;

  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [noteCounts, setNoteCounts] = useState<Record<string, number>>({});
  const [showMastersys, setShowMastersys] = useState(false);

  // Filtro e busca. O catálogo entra vazio e é preenchido logo depois; por
  // isso `loadFilters` roda de novo quando ele chega (ver efeito abaixo) —
  // sem catálogo não há como saber quais status são o default da origem.
  const [catalog, setCatalog] = useState<MastersysTicketStatus[]>([]);
  const [filters, setFilters] = useState<TaskFilterState>(() => loadFilters([], scope));
  const [searchInput, setSearchInput] = useState("");
  const [search, setSearch] = useState("");
  const [remoteResults, setRemoteResults] = useState<ExternalWorkItem[] | null>(null);
  const [remoteSearching, setRemoteSearching] = useState(false);
  /**
   * O filtro já foi reconciliado com o catálogo?
   *
   * Trava a gravação até lá. Sem isso, o efeito de salvar rodava no primeiro
   * render — quando o catálogo ainda está vazio e o default é "mostrar tudo" —
   * e gravava `statuses: []`. Na volta, esse valor gravado vencia o default de
   * verdade, e no primeiro uso o pós-atendimento aparecia. Ou seja: o bug era
   * exatamente o comportamento que este recurso existe para corrigir.
   */
  const [hydrated, setHydrated] = useState(false);
  /**
   * Tarefas com janela destacada agora.
   *
   * Vem do gerenciador de janelas, não de estado acumulado aqui — foi o bug do
   * pop-out de nota, onde trocar de aba desmontava o componente, zerava o
   * conjunto e a nota voltava ao quadro com a janela dela ainda aberta.
   */
  const [poppedOut, setPoppedOut] = useState<Set<string>>(new Set());
  /** Como o quadro se mantém atualizado: tempo real (segundos) ou polling. */
  const [liveSync, setLiveSync] = useState<{ realtime: boolean; pollSecs: number } | null>(null);

  /**
   * Alinha as janelas abertas com as tarefas que existem.
   *
   * Duas coisas ao mesmo tempo:
   *
   * 1. Atualiza quais tarefas estão destacadas (para o quadro marcá-las).
   * 2. **Fecha janelas órfãs.** Um espelho do Mastersys que saiu da fila do
   *    usuário é apagado pelo `retire_mirror`, e a janela dele ficaria aberta
   *    mostrando "tarefa indisponível" para sempre. Isso é caso real, não
   *    hipótese: acontece a cada sincronização em que um chamado é reatribuído.
   *
   * Em falha o conjunto fica vazio, e não preservado: em dúvida é melhor o
   * quadro mostrar a tarefa como não-destacada — pior seria marcá-la como
   * destacada sem janela alguma.
   */
  const reconcileWindows = async (existing: Task[]) => {
    try {
      const openIds = await api.openTaskWindowIds();
      const alive = new Set(existing.map((t) => t.id));
      const orphans = openIds.filter((id) => !alive.has(id));
      await Promise.all(
        orphans.map((id) => api.closeTaskWindow(id).catch(() => {})),
      );
      setPoppedOut(new Set(openIds.filter((id) => alive.has(id))));
    } catch {
      setPoppedOut(new Set());
    }
  };

  const handlePopOut = async (id: string) => {
    setError(null);
    try {
      await api.openTaskWindow(id);
      setPoppedOut((prev) => new Set(prev).add(id));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleClosePopOut = async (id: string) => {
    try {
      await api.closeTaskWindow(id);
    } catch (e) {
      setError(String(e));
    } finally {
      setPoppedOut((prev) => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  };

  const refresh = async () => {
    try {
      setLoading(true);
      setError(null);
      const [p, c] = await Promise.all([api.listPendingTasks(), api.listCompletedTasks()]);
      setPending(p);
      setCompleted(c);
      await reconcileWindows([...p, ...c]);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  // Catálogo de status: lê só o banco local, então é rápido e funciona
  // offline. Chega depois do primeiro render, e é aí que o filtro salvo pode
  // finalmente ser reconciliado com o default da origem.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const cat = await api.mastersysStatusCatalog();
        if (cancelled) return;
        setCatalog(cat);
        setFilters(loadFilters(cat, scope));
      } catch {
        // Catálogo é dado de apresentação: sem ele o quadro mostra o slug sem
        // cor e o filtro de status desaparece, mas nada deixa de funcionar.
      } finally {
        // Libera a gravação mesmo em falha: sem catálogo o usuário ainda
        // filtra por cliente e prazo, e essa escolha merece persistir.
        if (!cancelled) setHydrated(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!hydrated) return;
    saveFilters(filters, scope);
  }, [filters, hydrated]);

  // Como a sincronização está funcionando agora. Reconsultado após cada sync
  // automático porque o canal de tempo real pode ter caído nesse meio-tempo.
  useEffect(() => {
    let cancelled = false;
    const check = async () => {
      try {
        const [realtime, pollSecs] = await Promise.all([
          api.mastersysRealtimeConnected(),
          api.mastersysPollInterval(),
        ]);
        if (!cancelled) setLiveSync({ realtime, pollSecs });
      } catch {
        if (!cancelled) setLiveSync(null);
      }
    };
    void check();
    // 30 s: barato (só lê estado em memória do Rust) e suficiente para o
    // indicador não ficar mentindo por muito tempo depois de uma queda.
    const timer = setInterval(() => void check(), 30_000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  // Sincronização automática avisa por evento quando mudou algo. Sem isto o
  // quadro só refletiria a mudança no próximo clique do usuário.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      try {
        const un = await api.onMastersysSynced(() => {
          if (!cancelled) void refresh();
        });
        if (!cancelled) unlisten = un;
        else un();
      } catch {
        // Fora do runtime do Tauri — nada a escutar.
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Quando a janela principal recupera o foco, reconcilia: o usuário pode ter
  // fechado um pop-out pelo ✕ dele, e o quadro não fica sabendo de outra forma.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const un = await getCurrentWindow().onFocusChanged(({ payload }) => {
          if (payload && !cancelled) void refresh();
        });
        if (!cancelled) unlisten = un;
        else un();
      } catch {
        // Fora do runtime do Tauri — nada a observar.
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 300 ms, o mesmo do suporte: filtrar a cada tecla numa lista grande trava a
  // digitação, e esperar mais que isso já se sente como travamento.
  useEffect(() => {
    const timer = setTimeout(() => setSearch(searchInput), 300);
    return () => clearTimeout(timer);
  }, [searchInput]);

  // Trocar o termo invalida o resultado remoto: manter na tela um resultado de
  // outra busca é pior que não mostrar nada.
  useEffect(() => {
    setRemoteResults(null);
  }, [searchInput]);

  // Contador de anotações por tarefa, para o board mostrar o número sem que o
  // usuário precise expandir cada card.
  useEffect(() => {
    const all = [...pending, ...completed];
    if (all.length === 0) return;
    let cancelled = false;
    (async () => {
      const entries = await Promise.all(
        all.map(async (t) => {
          try {
            return [t.id, await api.countTaskNotes(t.id)] as const;
          } catch {
            return [t.id, 0] as const;
          }
        }),
      );
      if (!cancelled) setNoteCounts(Object.fromEntries(entries));
    })();
    return () => {
      cancelled = true;
    };
  }, [pending, completed]);

  const externalCount = useMemo(
    () => [...pending, ...completed].filter((t) => t.external !== null).length,
    [pending, completed],
  );

  /**
   * As três fatias, decididas por **estrutura** e não por filtro.
   *
   * - `localPending` — o que é seu: `external === null`. Tarefa vinculada a um
   *   chamado continua aqui, porque o dono dela é você e nenhum sync a toca.
   * - `mastersysPending` — sua fila no suporte, sem os parados.
   * - `parkedPending` — espelho que a origem considera parado
   *   (pós-atendimento, finalizado). Não está concluído localmente (segue em
   *   `list_pending`), mas também não é trabalho ativo. Antes ficava escondido
   *   pelo filtro padrão sem ter onde aparecer; agora mora em Concluídos.
   *
   * Consequência: nenhum item aparece em duas abas, qualquer que seja o filtro.
   */
  const localPending = useMemo(() => pending.filter((t) => t.external === null), [pending]);
  const mastersysPending = useMemo(
    () => pending.filter((t) => t.external !== null && !isParked(t)),
    [pending],
  );
  const parkedPending = useMemo(() => pending.filter(isParked), [pending]);

  /** A lista pendente desta aba. Concluídos não usa (tem as suas duas). */
  const pendingOfView = view === "local" ? localPending : mastersysPending;

  const visiblePending = useMemo(
    () => applyTaskFilters(pendingOfView, filters, search),
    [pendingOfView, filters, search],
  );
  const visibleCompleted = useMemo(
    () => applyTaskFilters(completed, filters, search),
    [completed, filters, search],
  );
  const visibleParked = useMemo(
    () => applyTaskFilters(parkedPending, filters, search),
    [parkedPending, filters, search],
  );
  const clients = useMemo(
    () => clientsInTasks([...pending, ...completed]),
    [pending, completed],
  );
  /**
   * Itens escondidos pelo filtro **nesta aba**.
   *
   * Contar as duas listas faria a aba Concluídos anunciar pendentes ocultas
   * que ela nunca mostraria — número verdadeiro, resposta errada à pergunta
   * "o que o filtro está me esconde aqui?".
   */
  const hiddenCount =
    view !== "completed"
      ? pendingOfView.length - visiblePending.length
      : completed.length +
        parkedPending.length -
        (visibleCompleted.length + visibleParked.length);

  const handleRemoteSearch = async () => {
    setRemoteSearching(true);
    setError(null);
    try {
      setRemoteResults(await api.mastersysSearchTickets(searchInput));
    } catch (e) {
      setError(String(e));
      setRemoteResults(null);
    } finally {
      setRemoteSearching(false);
    }
  };

  /**
   * Cria uma tarefa LOCAL a partir de um chamado consultado.
   *
   * Não grava espelho de propósito: um chamado que não está atribuído a você
   * não apareceria na próxima sincronização, e o `retire_mirror` apagaria a
   * tarefa junto com qualquer anotação sem aviso. Tarefa local não corre esse
   * risco — em troca, ela não acompanha mudanças do chamado.
   */
  const handleImportAsLocal = async (item: ExternalWorkItem) => {
    setError(null);
    try {
      const ref = item.reference;
      const origin = [
        ref.ticket ? `Chamado #${ref.ticket}` : null,
        ref.client,
      ]
        .filter(Boolean)
        .join(" · ");
      await api.createTask({
        title: item.title,
        description: [origin, item.description].filter(Boolean).join("\n\n"),
        priority: item.priority,
        // `CreateTaskPayload.deadline` é opcional (`string | undefined`), e o
        // item da origem usa `null` para "sem prazo".
        deadline: item.deadline ?? undefined,
      });
      setRemoteResults(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleExpanded = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const handleComplete = async (id: string) => {
    try {
      await api.completeTask(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleReopen = async (id: string) => {
    try {
      await api.reopenTask(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async (task: Task) => {
    const notes = noteCounts[task.id] ?? 0;
    const warning =
      notes > 0
        ? `Deletar "${task.title}" e ${notes} anotação${notes > 1 ? "ões" : ""}?`
        : `Deletar "${task.title}"?`;
    if (!confirm(warning)) return;
    try {
      await api.deleteTask(task.id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSnooze = async (id: string) => {
    try {
      await api.snoozeTask(id);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const renderTask = (t: Task) => {
    // `isOverdue` mora em `tasks/filter.ts` para ser testável e para a regra
    // ser uma só: item parado na origem não é atrasado, aqui e no filtro.
    const overdue = isOverdue(t);
    const parked = isParked(t);
    const dueSoon =
      !t.completed &&
      !parked &&
      t.deadline !== null &&
      new Date(t.deadline) <= new Date(Date.now() + 30 * 60 * 1000) &&
      new Date(t.deadline) > new Date();
    const toneClass = overdue ? "md-task--overdue" : dueSoon ? "md-task--soon" : "";
    const isOpen = expanded.has(t.id);
    const notes = noteCounts[t.id] ?? 0;

    return (
      <article
        key={t.id}
        className={`md-task ${toneClass} ${t.external ? "md-task--external" : ""} ${
          poppedOut.has(t.id) ? "md-task--poppedout" : ""
        }`}
        style={{ borderLeftColor: PRIORITY_VAR[t.priority] }}
      >
        {/* Origem sempre visível — inclusive "Local". Ver `TaskOriginStamp`. */}
        <TaskOriginStamp
          task={t}
          catalog={catalog}
          parked={parked}
          onOpenTicket={
            t.external?.ticket || t.link?.ticket ? () => setTicketOf(t) : undefined
          }
        />

        <div className="md-task-head">
          <span className="md-task-title">{t.title}</span>
          <span className="md-badge" style={{ background: PRIORITY_VAR[t.priority] }}>
            {PRIORITY_LABEL[t.priority]}
          </span>
          {overdue && <span className="md-due md-due--overdue">• atrasada</span>}
          {dueSoon && <span className="md-due md-due--soon">• vence em breve</span>}
          {/* Explica a ausência do "atrasada" num item de prazo vencido — sem
              isto pareceria bug de quem conhece o chamado. */}
          {parked && t.deadline !== null && new Date(t.deadline) <= new Date() && (
            <span className="md-due md-due--parked">• aguardando, sem lembrete</span>
          )}
        </div>

        {t.description && <div className="md-task-desc">{t.description}</div>}

        <div className="md-task-meta">
          <span>
            Prazo: <strong>{formatDeadline(t.deadline)}</strong>
          </span>
          {t.reminder_thresholds && t.reminder_thresholds.length > 0 && (
            <span>
              Lembretes:{" "}
              {t.reminder_thresholds
                .map((r) => {
                  const mins = thresholdMinutes(r);
                  return mins >= 60 ? `${mins / 60}h` : `${mins}m`;
                })
                .join(" · ")}
            </span>
          )}
        </div>

        <div className="md-btn-row">
          {!t.completed ? (
            <>
              <button onClick={() => void handleComplete(t.id)} className="md-btn md-btn--primary">
                Concluir
              </button>
              <button onClick={() => void handleSnooze(t.id)} className="md-btn">
                Adiar 15m
              </button>
            </>
          ) : (
            <button onClick={() => void handleReopen(t.id)} className="md-btn">
              Reabrir
            </button>
          )}

          <button
            onClick={() => toggleExpanded(t.id)}
            className="md-btn md-btn--ghost"
            aria-expanded={isOpen}
            aria-controls={`tasklog-${t.id}`}
          >
            {isOpen ? "Ocultar anotações" : "Anotações"}{" "}
            <span
              className={`md-notes-count ${notes === 0 ? "md-notes-count--empty" : ""}`}
              style={{ marginLeft: 4 }}
            >
              {notes}
            </span>
          </button>

          {/* Vincular nasce aqui, não numa barra no topo: o chamado que se
              quer acompanhar é o deste card, então ele já vem preenchido em
              vez de ser redigitado. Só aparece quando há chamado — vincular
              uma tarefa a uma tarefa local não significa nada. */}
          {(t.external?.ticket || t.link?.ticket) && (
            <button
              onClick={() => {
                setLinkSeed({
                  ticket: (t.external?.ticket ?? t.link?.ticket) as string,
                  client: t.external?.client ?? t.link?.client ?? null,
                });
                setCreating("link");
              }}
              className="md-btn md-btn--ghost"
              title={`Criar uma tarefa sua ligada ao chamado #${
                t.external?.ticket ?? t.link?.ticket
              } — o Mastersys não é alterado`}
            >
              Vincular tarefa
            </button>
          )}

          {poppedOut.has(t.id) ? (
            <button
              onClick={() => void handleClosePopOut(t.id)}
              className="md-btn md-btn--ghost"
              title="Fechar a janela destacada desta tarefa"
            >
              Recolher
            </button>
          ) : (
            <button
              onClick={() => void handlePopOut(t.id)}
              className="md-btn md-btn--ghost"
              title="Abrir esta tarefa em janela própria, por cima das outras"
            >
              Destacar
            </button>
          )}

          <button
            onClick={() => void handleDelete(t)}
            className="md-btn md-btn--danger"
            style={{ marginLeft: "auto" }}
          >
            Deletar
          </button>
        </div>

        {isOpen && (
          <div id={`tasklog-${t.id}`}>
            <TaskNotes
              taskId={t.id}
              initialCount={notes}
              onCountChange={(count) =>
                setNoteCounts((prev) => (prev[t.id] === count ? prev : { ...prev, [t.id]: count }))
              }
            />
          </div>
        )}
      </article>
    );
  };

  /**
   * Quadro vazio de verdade — não "vazio por filtro", que tem mensagem
   * própria. Na aba Concluídos só as concluídas contam: um quadro cheio de
   * pendentes ainda não tem nada a mostrar ali.
   */
  const isEmptyAll =
    !loading &&
    (view === "completed"
      ? completed.length === 0 && parkedPending.length === 0
      : pendingOfView.length === 0);

  return (
    <div
      style={{
        fontFamily: "inherit",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        minHeight: 0,
      }}
    >
      <header className="md-board-header">
        <strong className="md-board-title">
          {view === "local" ? "Tarefas" : view === "mastersys" ? "Chamados" : "Concluídos"}
        </strong>
        {/* O painel da integração pertence às abas que mostram o Mastersys.
            Na aba de tarefas locais ele seria um botão sobre um sistema que
            aquele quadro não usa. */}
        {view !== "local" && (
          <button className="md-btn" onClick={() => setShowMastersys(true)}>
            Mastersys
            {externalCount > 0 && (
              <span className="md-notes-count" style={{ marginLeft: 6 }}>
                {externalCount}
              </span>
            )}
          </button>
        )}
        <span className="md-count">
          {view === "completed"
            ? `${completed.length} concluídas · ${parkedPending.length} aguardando`
            : `${pendingOfView.length} ${view === "local" ? "pendentes" : "na sua fila"}`}
          {hiddenCount > 0 && ` · ${hiddenCount} oculta(s) por filtro`}
        </span>

        {/* Como o quadro se mantém atualizado só diz respeito a quem espelha
            o Mastersys. Em Tarefas locais não há o que sincronizar. */}
        {view !== "local" && liveSync && externalCount > 0 && (
          <span
            className={`md-livesync ${liveSync.realtime ? "md-livesync--on" : ""}`}
            title={
              liveSync.realtime
                ? "Conectado ao canal de tempo real do Mastersys: mudanças aparecem em segundos."
                : `Tempo real indisponível — o quadro se atualiza a cada ${Math.round(
                    liveSync.pollSecs / 60,
                  )} min. Nada deixa de sincronizar, só demora mais.`
            }
          >
            {liveSync.realtime
              ? "tempo real"
              : `a cada ${Math.round(liveSync.pollSecs / 60)} min`}
          </span>
        )}
      </header>

      <TaskFilters
        filters={filters}
        onChange={setFilters}
        catalog={catalog}
        clients={clients}
        searchInput={searchInput}
        onSearchInput={setSearchInput}
        // Busca ao vivo consulta a API do suporte: só faz sentido na aba que
        // fala com ele.
        onRemoteSearch={view === "mastersys" ? handleRemoteSearch : undefined}
        remoteSearching={remoteSearching}
        scope={scope}
      />

      {remoteResults !== null && (
        <section className="md-remote-results">
          <div className="md-eyebrow" style={{ marginBottom: 8 }}>
            Consulta no Mastersys · {remoteResults.length} resultado(s)
          </div>
          {remoteResults.length === 0 ? (
            <div className="md-quiet">Nenhum chamado encontrado para esse termo.</div>
          ) : (
            <>
              <p className="md-filter-hint" style={{ marginTop: 0 }}>
                Isto é consulta, não sincronização. Um chamado que não está
                atribuído a você não pode virar espelho no quadro — a próxima
                sincronização o apagaria. Importar cria uma <strong>tarefa
                local</strong>, que sobrevive, mas não acompanha mudanças do
                chamado.
              </p>
              <ul className="md-remote-list">
                {remoteResults.map((item) => (
                  <li key={item.reference.external_id} className="md-remote-item">
                    <div className="md-stamp">
                      {item.reference.status_label && (
                        <StatusBadge
                          statusLabel={item.reference.status_label}
                          catalog={catalog}
                        />
                      )}
                      {item.reference.ticket && (
                        <span className="md-stamp-ticket">#{item.reference.ticket}</span>
                      )}
                      {item.reference.client && (
                        <span className="md-stamp-client" title={item.reference.client}>
                          {item.reference.client}
                        </span>
                      )}
                    </div>
                    <span className="md-remote-title">{item.title}</span>
                    <button
                      className="md-btn md-btn--ghost"
                      onClick={() => void handleImportAsLocal(item)}
                    >
                      Criar tarefa local
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>
      )}

      {/* Criar deixou de ocupar duas faixas fixas do quadro: o formulário
          inteiro (descrição, lembretes, vínculo) vive no diálogo.
          Só na aba de tarefas locais: chamado não se cria aqui (a integração
          é somente leitura), e concluída não se cria — se conclui. */}
      {view === "local" && (
        <div className="md-create-bar" style={{ gap: 8 }}>
          <button
            onClick={() => {
              setLinkSeed(null);
              setCreating("plain");
            }}
            className="md-primary md-primary-accent"
          >
            Nova tarefa
          </button>
          <span className="md-panel-note" style={{ margin: 0 }}>
            Para vincular uma tarefa a um chamado, use <strong>Vincular
            tarefa</strong> no card dele — ou o interruptor de vínculo aqui, que
            busca o chamado.
          </span>
        </div>
      )}

      {error && (
        <div role="alert" className="md-alert">
          {error}
          <button onClick={() => setError(null)} className="md-alert-dismiss">
            dispensar
          </button>
        </div>
      )}

      <div
        className="scroll-hidden"
        style={{
          flex: 1,
          minHeight: 0,
          padding: 14,
          background: "var(--canvas)",
          overflow: isEmptyAll ? "hidden" : undefined,
          display: isEmptyAll ? "flex" : "block",
          flexDirection: isEmptyAll ? ("column" as const) : undefined,
        }}
      >
        {loading ? (
          <>
            <div className="md-skeleton" />
            <div className="md-skeleton" style={{ width: "92%" }} />
          </>
        ) : isEmptyAll ? (
          <div className="md-empty" role="status">
            <div className="md-empty-illus" aria-hidden>
              <svg
                width="26"
                height="26"
                viewBox="0 0 24 24"
                fill="none"
                aria-hidden
                style={{ position: "relative", zIndex: 1 }}
              >
                <rect
                  x="5"
                  y="5"
                  width="14"
                  height="14"
                  rx="3"
                  fill="var(--surface-plain)"
                  stroke="var(--text)"
                  strokeWidth="1.4"
                />
                <path d="M8 10h8M8 13h5" stroke="var(--text)" strokeWidth="1.3" strokeLinecap="round" />
                <circle cx="17" cy="7" r="3" fill="var(--accent)" stroke="var(--text)" strokeWidth="1.2" />
                <path
                  d="M15.6 7.2 16.6 8.2 18.4 6.2"
                  stroke="var(--accent-ink)"
                  strokeWidth="1.2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
            </div>
            {view === "local" ? (
              <>
                <h3>Nenhuma tarefa sua ainda</h3>
                <p>
                  Esta aba é só do que é seu: tarefas que você cria, com
                  prioridade, prazo e lembretes — inclusive as que você vincula
                  a um chamado. Os chamados atribuídos a você ficam na aba{" "}
                  <strong>Chamados</strong>.
                </p>
                <button
                  className="md-empty-cta md-empty-cta--primary"
                  onClick={() => {
                    setLinkSeed(null);
                    setCreating("plain");
                  }}
                >
                  Criar primeira tarefa
                </button>
              </>
            ) : view === "mastersys" ? (
              <>
                <h3>Nenhum chamado na sua fila</h3>
                <p>
                  Aqui aparecem as tarefas e os chamados do Mastersys em que
                  você é analista responsável ou atendente. Se você já conectou
                  e ainda está vazio, pode ser que nada esteja atribuído a você
                  — ou que tudo esteja em pós-atendimento, na aba{" "}
                  <strong>Concluídos</strong>.
                </p>
                <button
                  className="md-empty-cta md-empty-cta--primary"
                  onClick={() => setShowMastersys(true)}
                >
                  Conectar o Mastersys
                </button>
              </>
            ) : (
              <>
                <h3>Nada concluído ainda</h3>
                <p>
                  Aqui ficam duas coisas: o que você concluir (com as anotações
                  e o histórico — reabrir devolve ao quadro) e os chamados que o
                  Mastersys considera parados, como pós-atendimento e
                  finalizado.
                </p>
              </>
            )}
          </div>
        ) : view !== "completed" ? (
          <>
            <h3 className="md-eyebrow" style={{ margin: "2px 0 10px" }}>
              {view === "local" ? "Pendentes" : "Na sua fila"}
            </h3>
            {visiblePending.length === 0 ? (
              <div className="md-quiet" style={{ marginBottom: 12 }}>
                {pendingOfView.length === 0
                  ? view === "local"
                    ? "Nenhuma tarefa pendente — bom trabalho."
                    : "Nenhum chamado ativo na sua fila."
                  : "Nada aqui casa com o filtro atual."}
              </div>
            ) : (
              visiblePending.map(renderTask)
            )}
            {/* Ponte para a aba Concluídos: quem concluiu algo aqui, ou viu um
                chamado ir para pós-atendimento, precisa saber para onde o item
                foi — senão parece que desapareceu. */}
            {view === "mastersys" && parkedPending.length > 0 && (
              <p className="md-panel-note" style={{ marginTop: 18 }}>
                {parkedPending.length} chamado(s) parado(s) na origem
                (pós-atendimento, finalizado) na aba <strong>Concluídos</strong>.
              </p>
            )}
            {view === "local" && completed.length > 0 && (
              <p className="md-panel-note" style={{ marginTop: 18 }}>
                {completed.length} concluída(s) na aba <strong>Concluídos</strong>.
              </p>
            )}
          </>
        ) : (
          <>
            {/* Duas seções, porque são dois estados diferentes: o chamado que a
                origem parou não está concluído por você — está aguardando lá. */}
            <h3 className="md-eyebrow" style={{ margin: "2px 0 10px" }}>
              Aguardando na origem{" "}
              {visibleParked.length > 0 && (
                <span style={{ fontWeight: 500, textTransform: "none", letterSpacing: 0 }}>
                  · {visibleParked.length}
                </span>
              )}
            </h3>
            {visibleParked.length === 0 ? (
              <div className="md-quiet" style={{ marginBottom: 12 }}>
                {parkedPending.length === 0
                  ? "Nenhum chamado em pós-atendimento ou finalizado na sua fila."
                  : "Nenhum casa com o filtro atual."}
              </div>
            ) : (
              <>
                <p className="md-panel-note" style={{ marginTop: 0 }}>
                  Chamados que o Mastersys considera parados. Não contam como
                  atrasados nem geram lembrete, e voltam ao quadro de Tarefas
                  sozinhos se o status mudar na origem.
                </p>
                {visibleParked.map(renderTask)}
              </>
            )}

            <h3 className="md-eyebrow" style={{ margin: "18px 0 10px" }}>
              {/* Só "Concluídas": a lista mistura o que você concluiu aqui com
                  espelhos que a origem fechou (`closed_at`/`resolved_at`), e
                  dizer "por você" afirmaria autoria que não é verdade. */}
              Concluídas{" "}
              {visibleCompleted.length > 0 && (
                <span style={{ fontWeight: 500, textTransform: "none", letterSpacing: 0 }}>
                  · {visibleCompleted.length}
                </span>
              )}
            </h3>
            {visibleCompleted.length === 0 ? (
              <div className="md-quiet">
                {completed.length === 0
                  ? "Nenhuma tarefa concluída ainda."
                  : "Nenhuma concluída casa com o filtro atual."}
              </div>
            ) : (
              visibleCompleted.map(renderTask)
            )}
          </>
        )}
      </div>

      {creating && (
        <TaskFormModal
          mode={creating}
          initialLink={linkSeed}
          onClose={() => {
            setCreating(null);
            setLinkSeed(null);
          }}
          onCreated={() => void refresh()}
        />
      )}

      {ticketOf && (
        <TicketModal
          task={ticketOf}
          catalog={catalog}
          parked={isParked(ticketOf)}
          onClose={() => setTicketOf(null)}
          onLinkChanged={() => void refresh()}
        />
      )}

      {showMastersys && (
        <MastersysPanel
          onClose={() => setShowMastersys(false)}
          onTasksChanged={() => void refresh()}
        />
      )}
    </div>
  );
}
