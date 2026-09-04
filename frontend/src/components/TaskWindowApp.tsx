import { useEffect, useState } from "react";
import type { MastersysTicketStatus, Task } from "../types";
import * as api from "../api";
import { TaskNotes } from "./TaskNotes";
import { TaskOriginStamp } from "./TaskOriginStamp";
import { isOverdue, isParked } from "../tasks/filter";

interface Props {
  taskId: string;
}

/**
 * Espera antes de gravar a geometria, em ms.
 *
 * Curto o bastante para sobreviver a um fechamento logo após arrastar, e longo
 * o bastante para um arrasto inteiro virar uma escrita só.
 */
const GEOMETRY_DEBOUNCE_MS = 250;

function formatDeadline(iso: string | null): string {
  if (!iso) return "sem prazo";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

/**
 * Uma tarefa destacada em janela própria.
 *
 * Serve o caso de acompanhar um atendimento sem manter o MasterNote inteiro na
 * frente: janela pequena, sem moldura, por cima das outras, com o log de
 * anotações à mão — que é onde o trabalho de verdade é registrado.
 *
 * Espelha o pop-out de nota no comportamento (arrastar pelo cabeçalho, fechar
 * pelo ✕, geometria persistida), mas **não** herda os bugs dele: a posição é
 * validada contra os monitores reais no lado Rust, e o quadro descobre quais
 * janelas estão abertas perguntando ao gerenciador de janelas.
 */
export function TaskWindowApp({ taskId }: Props) {
  const [task, setTask] = useState<Task | null>(null);
  const [catalog, setCatalog] = useState<MastersysTicketStatus[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  // Janela de pop-out é transparente e sem moldura; sem estas classes o corpo
  // herdaria o fundo e o scroll do app principal.
  useEffect(() => {
    document.body.classList.add("note-window-body");
    document.documentElement.classList.add("note-window-body");
    return () => {
      document.body.classList.remove("note-window-body");
      document.documentElement.classList.remove("note-window-body");
    };
  }, []);

  const load = async () => {
    try {
      const [t, cat] = await Promise.all([
        api.getTask(taskId),
        api.mastersysStatusCatalog().catch(() => [] as MastersysTicketStatus[]),
      ]);
      setTask(t);
      setCatalog(cat);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId]);

  // Persiste geometria quando o usuário para de mexer.
  //
  // ## Duas armadilhas aqui, ambas já pagas
  //
  // 1. **Não chamar nada que mova a janela.** As funções são `save*`, que só
  //    gravam. A versão anterior aplicava a posição também, e como quem chama é
  //    este listener, virava laço: gravar → mover → `onMoved` → gravar. A
  //    janela agitava freneticamente (bug filmado pelo DEV em 2026-09-03).
  //
  // 2. **Converter físico → lógico.** `onMoved`/`onResized` entregam pixels
  //    FÍSICOS; a janela é criada em lógicos. Sem dividir pela escala, a janela
  //    "anda" a cada reabertura em tela com escala != 100% — o padrão em
  //    notebook Windows.
  //
  // O debounce existe porque `onMoved` dispara a cada pixel do arrasto: sem
  // ele, arrastar a janela pela tela emite centenas de escritas no SQLite. O
  // que interessa é onde ela parou.
  useEffect(() => {
    let disposed = false;
    let unlisten: Array<() => void> = [];
    let moveTimer: ReturnType<typeof setTimeout> | undefined;
    let sizeTimer: ReturnType<typeof setTimeout> | undefined;

    (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        const scale = await win.scaleFactor();

        const unMove = await win.onMoved(({ payload }) => {
          if (disposed) return;
          clearTimeout(moveTimer);
          moveTimer = setTimeout(() => {
            void api
              .saveTaskWindowPosition(taskId, payload.x / scale, payload.y / scale)
              .catch(() => {
                // Best effort: não gravar a posição não pode travar a janela.
              });
          }, GEOMETRY_DEBOUNCE_MS);
        });

        const unResize = await win.onResized(({ payload }) => {
          if (disposed) return;
          clearTimeout(sizeTimer);
          sizeTimer = setTimeout(() => {
            void api
              .saveTaskWindowSize(taskId, payload.width / scale, payload.height / scale)
              .catch(() => {});
          }, GEOMETRY_DEBOUNCE_MS);
        });

        if (!disposed) unlisten = [unMove, unResize];
        else {
          unMove();
          unResize();
        }
      } catch {
        // Fora do runtime do Tauri (navegador em dev) — nada a sincronizar.
      }
    })();

    return () => {
      disposed = true;
      clearTimeout(moveTimer);
      clearTimeout(sizeTimer);
      unlisten.forEach((fn) => fn());
    };
  }, [taskId]);

  const handleClose = async () => {
    // Comando Rust primeiro: `getCurrentWindow().close()` depende de
    // `core:window:allow-close` na capability, e a ACL nega em silêncio quando
    // falta. O comando não passa pela ACL.
    try {
      await api.closeTaskWindow(taskId);
    } catch {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        await getCurrentWindow().close();
      } catch {
        setError("Não foi possível fechar esta janela.");
      }
    }
  };

  if (loading) {
    return (
      <div className="note-window-root md-taskwindow">
        <div className="md-skeleton" style={{ height: 60 }} />
      </div>
    );
  }

  if (error !== null || task === null) {
    return (
      <div className="note-window-root md-taskwindow">
        <div className="md-taskwindow-head" data-tauri-drag-region="">
          <span className="md-taskwindow-title">Tarefa indisponível</span>
          <button className="md-taskwindow-close" onClick={() => void handleClose()}>
            ✕
          </button>
        </div>
        <div role="alert" className="md-alert" style={{ margin: 10 }}>
          {/* Acontece de verdade: um espelho do Mastersys apagado pelo
              `retire_mirror` deixa esta janela apontando para nada. O ✕ tem de
              continuar funcionando. */}
          {error ?? "Esta tarefa não existe mais — ela pode ter saído da sua fila no Mastersys."}
        </div>
      </div>
    );
  }

  const overdue = isOverdue(task);
  const parked = isParked(task);

  return (
    <div className="note-window-root md-taskwindow">
      {/* Cabeçalho é a alça de arrastar. Exige
          `core:window:allow-start-dragging` na capability — sem ela a ACL nega
          em silêncio, que era o bug do pop-out de nota. */}
      <div className="md-taskwindow-head" data-tauri-drag-region="">
        <span className="md-taskwindow-title" title={task.title}>
          {task.title}
        </span>
        <button
          className="md-taskwindow-close"
          onClick={() => void handleClose()}
          title="Fechar esta janela (a tarefa continua no quadro)"
          aria-label="Fechar janela"
        >
          ✕
        </button>
      </div>

      <div className="md-taskwindow-body">
        {/* Mesmo selo do quadro — a janela destacada mostra a mesma tarefa e
            não pode contar uma história diferente sobre a origem dela. Sem
            `onOpenTicket`: aqui não há espaço para um diálogo, e a leitura do
            chamado fica no quadro. */}
        <TaskOriginStamp task={task} catalog={catalog} parked={parked} />

        <div className="md-taskwindow-meta">
          <span>
            Prazo: <strong>{formatDeadline(task.deadline)}</strong>
          </span>
          {overdue && <span className="md-due md-due--overdue">• atrasada</span>}
          {parked && <span className="md-due md-due--parked">• aguardando</span>}
        </div>

        {task.description && (
          <div className="md-taskwindow-desc">{task.description}</div>
        )}

        <TaskNotes taskId={task.id} />
      </div>
    </div>
  );
}
