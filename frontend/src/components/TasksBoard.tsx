import { useCallback, useEffect, useMemo, useState } from "react";
import type { ExternalWorkItem, MastersysTicketStatus, Task, Priority } from "../types";
import * as api from "../api";
import { TaskNotes } from "./TaskNotes";
import { MastersysPanel } from "./MastersysPanel";
import { TaskFilters } from "./TaskFilters";
import { StatusBadge } from "./StatusBadge";
import { TaskOriginStamp } from "./TaskOriginStamp";
import { TaskFormModal } from "./TaskFormModal";
import { TicketModal } from "./TicketModal";
import { TaskCard } from "./TaskCard";
import { COLUMNS, columnOf, groupByColumn, planMove, type ColumnId } from "../tasks/board";
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

/** Qual recorte do quadro Kanban está sendo desenhado. */
export type BoardView = "board" | "mastersys";

/**
 * Coluna "Concluído" mostra só as mais recentes: ela só cresce, e centenas de
 * cards antigos empurrariam o que importa para fora da tela. O resto continua
 * alcançável pela busca.
 */
const DONE_COLUMN_LIMIT = 30;

interface Props {
  /**
   * Qual recorte este quadro mostra.
   *
   * | `view`      | seção    | conteúdo |
   * |-------------|----------|----------|
   * | `board`     | Tarefas  | só tarefas locais (inclusive as vinculadas a um chamado) |
   * | `mastersys` | Chamados | só os espelhos do Mastersys |
   *
   * Sem interseção: cada item aparece numa seção só (pedido do DEV em
   * 2026-09-30 — misturar tudo no Quadro o deixava igual a Chamados).
   *
   * As duas são o mesmo Kanban (A fazer / Em andamento / Aguardando /
   * Concluído). A coluna é só local — mover não altera o Mastersys.
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

  /** Preferências de filtro por seção — ver `FilterScope`. */
  const scope: FilterScope = view === "board" ? "local" : "mastersys";

  /** Card aberto no painel de detalhe. */
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selectCard = useCallback(
    (id: string) => setSelectedId((cur) => (cur === id ? null : id)),
    [],
  );

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

  /**
   * Recarrega o quadro.
   *
   * ## Por que existe `silent`
   *
   * O esqueleto de carregamento substitui a lista inteira. Mostrá-lo numa
   * recarga de fundo — sincronização automática, volta do foco da janela — faz
   * a tela **piscar**: a lista desaparece e volta, perdendo posição de rolagem
   * e o que o usuário estava lendo. E recarga de fundo é a maioria delas.
   *
   * Então o esqueleto vale só para a primeira carga, quando de fato não há
   * nada na tela para preservar. Depois disso a troca é silenciosa: os dados
   * novos entram no lugar dos velhos e o React só mexe no que mudou.
   *
   * Erro também é silenciado na recarga de fundo: o quadro na tela continua
   * válido, e uma faixa vermelha aparecendo sozinha durante uma sincronização
   * assusta sem oferecer ação. Recarga pedida pelo usuário mostra o erro.
   */
  const refresh = async ({ silent = false }: { silent?: boolean } = {}) => {
    try {
      if (!silent) {
        setLoading(true);
        setError(null);
      }
      const [p, c] = await Promise.all([api.listPendingTasks(), api.listCompletedTasks()]);
      setPending(p);
      setCompleted(c);
      await reconcileWindows([...p, ...c]);
    } catch (e) {
      if (!silent) setError(String(e));
    } finally {
      if (!silent) setLoading(false);
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
          // Silencioso: é recarga de fundo, e o usuário não pediu nada.
          if (!cancelled) void refresh({ silent: true });
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
          if (payload && !cancelled) {
            // Silencioso: voltar para o app não pode apagar o quadro que o
            // usuário estava olhando um segundo antes.
            void refresh({ silent: true });
            // O contador também: o usuário pode ter escrito uma anotação na
            // janela destacada de uma tarefa, e o card no quadro precisa
            // refletir isso. É uma consulta, não uma por tarefa.
            void refreshNoteCounts();
          }
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
  // Uma consulta para o quadro inteiro (`count_all_task_notes`), e não uma por
  // tarefa: com algumas centenas de chamados na fila, o laço anterior disparava
  // uma travessia de IPC e uma consulta SQLite por card a cada recarga — e o
  // quadro recarrega a cada sincronização.
  //
  // Depende do TAMANHO das listas, não das listas em si: um sync que só mexeu
  // no título de um chamado não muda contador de anotação nenhum, e reconsultar
  // ali seria trabalho jogado fora. Escrever ou apagar anotação já chama
  // `refreshNoteCounts` pelo caminho que fez a mudança.
  const taskCount = pending.length + completed.length;

  const refreshNoteCounts = async () => {
    try {
      setNoteCounts(await api.countAllTaskNotes());
    } catch {
      // Contador é enfeite do card: sem ele o quadro funciona igual.
    }
  };

  useEffect(() => {
    if (taskCount === 0) {
      setNoteCounts({});
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const counts = await api.countAllTaskNotes();
        if (!cancelled) setNoteCounts(counts);
      } catch {
        // Idem: silencioso de propósito.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [taskCount]);

  const externalCount = useMemo(
    () => [...pending, ...completed].filter((t) => t.external !== null).length,
    [pending, completed],
  );

  /**
   * Vocabulário de status que o recorte de status pode decidir.
   *
   * Espelho com status fora daqui não é filtrado por status — senão ele não
   * teria aba nenhuma onde aparecer. Ver `isStatusFilterable` em `filter.ts`.
   *
   * Vazio enquanto o catálogo não chega: `undefined` mantém o recorte valendo
   * para tudo, e nesse instante `filters.statuses` também está vazio (o default
   * sem catálogo mostra tudo), então nada é escondido no meio do caminho.
   */
  const knownStatuses = useMemo(
    () => (catalog.length > 0 ? catalog.map((s) => s.value) : undefined),
    [catalog],
  );

  const clients = useMemo(
    () => clientsInTasks([...pending, ...completed]),
    [pending, completed],
  );
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
      await refresh({ silent: true });
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

  // Depois de uma ação do usuário a recarga é silenciosa: o erro da AÇÃO já
  // aparece pelo `catch` de cada handler, e o esqueleto no lugar da lista
  // faria o quadro piscar e perder a rolagem a cada clique.
  const handleComplete = async (id: string) => {
    try {
      await api.completeTask(id);
      await refresh({ silent: true });
    } catch (e) {
      setError(String(e));
    }
  };

  const handleReopen = async (id: string) => {
    try {
      await api.reopenTask(id);
      await refresh({ silent: true });
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
      await refresh({ silent: true });
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

  /**
   * Leva um card para outra coluna. Concluir e reabrir passam pelos comandos
   * próprios (cancelam/reagendam lembretes); entre colunas abertas é só a
   * coluna. Nada disso toca o Mastersys.
   */
  const handleMove = async (t: Task, target: ColumnId) => {
    const plan = planMove(t, target);
    if (plan.kind === "none") return;
    setError(null);
    try {
      if (plan.kind === "complete") {
        await api.completeTask(t.id);
      } else if (plan.kind === "reopen") {
        await api.reopenTask(t.id);
        await api.updateTask(t.id, { board_column: plan.column });
      } else {
        await api.updateTask(t.id, { board_column: plan.column });
      }
      await refresh({ silent: true });
    } catch (e) {
      setError(String(e));
    }
  };

  const renderTask = (t: Task, { detail = false }: { detail?: boolean } = {}) => {
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
    // No painel de detalhe as anotações ficam sempre abertas: é para isso que
    // o usuário abriu o card.
    const isOpen = detail || expanded.has(t.id);
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
          <span
            className="md-badge"
            style={{ "--badge-ink": PRIORITY_VAR[t.priority] } as React.CSSProperties}
          >
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

          {!detail && <button
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
          </button>}

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

  const allTasks = useMemo(() => [...pending, ...completed], [pending, completed]);
  const tasksOfView = useMemo(
    () => allTasks.filter((t) => (view === "board" ? t.external === null : t.external !== null)),
    [allTasks, view],
  );
  const visible = useMemo(
    () => applyTaskFilters(tasksOfView, filters, search, knownStatuses),
    [tasksOfView, filters, search, knownStatuses],
  );
  const grouped = useMemo(() => groupByColumn(visible), [visible]);
  const hiddenByFilter = tasksOfView.length - visible.length;
  const selected = selectedId ? allTasks.find((t) => t.id === selectedId) ?? null : null;
  const now = Date.now();

  // Card selecionado que sumiu (retirado da fila por um sync): fecha o painel
  // em vez de mostrar um fantasma.
  useEffect(() => {
    if (selectedId && !loading && !allTasks.some((t) => t.id === selectedId)) setSelectedId(null);
  }, [selectedId, allTasks, loading]);

  const isEmptyAll = !loading && tasksOfView.length === 0;

  return (
    <div style={{ height: "100%", display: "flex", flexDirection: "column", minHeight: 0 }}>
      <header className="md-board-header">
        <div className="md-board-heading">
          <h1 className="md-board-title">{view === "board" ? "Tarefas" : "Chamados"}</h1>
          <span className="md-count">
            {view === "board" ? "Suas tarefas locais" : "Somente o que veio do Mastersys"}
            {hiddenByFilter > 0 && ` · ${hiddenByFilter} oculto(s) por filtro`}
          </span>
        </div>
        <span style={{ flex: 1 }} />
        {liveSync && externalCount > 0 && (
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
            {liveSync.realtime ? "tempo real" : `a cada ${Math.round(liveSync.pollSecs / 60)} min`}
          </span>
        )}
        <button className="md-btn" onClick={() => setShowMastersys(true)}>
          Mastersys
        </button>
        <button
          className="md-btn md-btn--primary"
          onClick={() => {
            setLinkSeed(null);
            setCreating("plain");
          }}
        >
          Nova tarefa
        </button>
      </header>

      <TaskFilters
        filters={filters}
        onChange={setFilters}
        catalog={catalog}
        clients={clients}
        searchInput={searchInput}
        onSearchInput={setSearchInput}
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
                        <StatusBadge statusLabel={item.reference.status_label} catalog={catalog} />
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
                    <button className="md-btn md-btn--ghost" onClick={() => void handleImportAsLocal(item)}>
                      Criar tarefa local
                    </button>
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>
      )}

      {error && (
        <div role="alert" className="md-alert">
          {error}
          <button onClick={() => setError(null)} className="md-alert-dismiss">
            dispensar
          </button>
        </div>
      )}

      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>
        {loading ? (
          <div className="md-kanban">
            {COLUMNS.map((c) => (
              <div key={c.id} className="md-kcol">
                <div className="md-skeleton" />
              </div>
            ))}
          </div>
        ) : isEmptyAll ? (
          <div className="md-empty" role="status" style={{ flex: 1 }}>
            {view === "board" ? (
              <>
                <h3>Nenhuma tarefa sua ainda</h3>
                <p>
                  Tarefas que você cria, com prioridade, prazo e lembretes —
                  inclusive as vinculadas a um chamado. Os chamados atribuídos a
                  você ficam em <strong>Chamados</strong>.
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
            ) : (
              <>
                <h3>Nenhum chamado na sua fila</h3>
                <p>
                  Aqui aparecem as tarefas e os chamados do Mastersys em que
                  você é analista responsável ou atendente.
                </p>
                <button className="md-empty-cta md-empty-cta--primary" onClick={() => setShowMastersys(true)}>
                  Conectar o Mastersys
                </button>
              </>
            )}
          </div>
        ) : (
          <div className="md-kanban">
            {COLUMNS.map((col) => {
              const all = grouped[col.id];
              const cards = col.id === "done" ? all.slice(0, DONE_COLUMN_LIMIT) : all;
              return (
                <section key={col.id} className="md-kcol" aria-label={`${col.title}, ${all.length}`}>
                  <h2 className="md-kcol-head">
                    <span className={`md-kcol-dot md-kcol-dot--${col.id}`} aria-hidden />
                    {col.title}
                    <span className="md-kcol-count">{all.length}</span>
                  </h2>
                  <div className="md-kcol-list">
                    {cards.map((t) => (
                      <TaskCard
                        key={t.id}
                        task={t}
                        selected={t.id === selectedId}
                        poppedOut={poppedOut.has(t.id)}
                        notes={noteCounts[t.id] ?? 0}
                        now={now}
                        onSelect={selectCard}
                      />
                    ))}
                    {all.length === 0 && <div className="md-kcol-empty">Nada aqui</div>}
                    {all.length > cards.length && (
                      <div className="md-kcol-empty">
                        + {all.length - cards.length} mais antigas — use a busca
                      </div>
                    )}
                  </div>
                </section>
              );
            })}
          </div>
        )}

        {selected && (
          <aside className="md-detail" aria-label="Detalhe da tarefa">
            <div className="md-detail-head">
              <span className="md-eyebrow">Mover para</span>
              <span style={{ flex: 1 }} />
              <button
                type="button"
                className="md-detail-close"
                aria-label="Fechar detalhe"
                onClick={() => setSelectedId(null)}
              >
                ✕
              </button>
            </div>
            <div className="md-detail-move" role="group" aria-label="Mover para">
              {COLUMNS.map((c) => (
                <button
                  key={c.id}
                  type="button"
                  aria-pressed={columnOf(selected) === c.id}
                  onClick={() => void handleMove(selected, c.id)}
                >
                  {c.title}
                </button>
              ))}
            </div>
            {renderTask(selected, { detail: true })}
          </aside>
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
          onCreated={() => void refresh({ silent: true })}
        />
      )}

      {ticketOf && (
        <TicketModal
          task={ticketOf}
          catalog={catalog}
          parked={isParked(ticketOf)}
          onClose={() => setTicketOf(null)}
          onLinkChanged={() => void refresh({ silent: true })}
        />
      )}

      {showMastersys && (
        <MastersysPanel
          onClose={() => setShowMastersys(false)}
          onTasksChanged={() => void refresh({ silent: true })}
        />
      )}
    </div>
  );
}
