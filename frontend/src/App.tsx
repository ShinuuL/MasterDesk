import { useEffect, useState } from "react";
import { NotesBoard } from "./components/NotesBoard";
import { TasksBoard } from "./components/TasksBoard";
import { AuthPanel } from "./components/AuthPanel";
import { NoteCard } from "./components/NoteCard";
import { TaskWindowApp } from "./components/TaskWindowApp";
import { Sidebar, type Section } from "./components/Sidebar";
import { UpdateToast } from "./components/UpdateToast";
import type { AuthPayload, Note } from "./types";
import * as api from "./api";

/**
 * As quatro seções: **Notas | Tarefas | Chamados | Concluídos**.
 *
 * `tasks` é só o que é do usuário (`external === null`) e `tickets` é a fila
 * espelhada do Mastersys. Antes os dois dividiam um quadro só, e a mistura
 * cobrava caro: o trabalho próprio ficava soterrado por dezenas de chamados, e
 * as duas metades têm regras opostas — o espelho é sobrescrito e retirado pela
 * sincronização, a tarefa local é sua e ninguém a toca.
 *
 * `done` reúne o que saiu do trabalho ativo das duas origens: concluídas e
 * chamados que a origem considera parados (pós-atendimento, finalizado).
 *
 * Notas arquivadas continuam na aba Notas, no interruptor "Arquivadas":
 * arquivar não é concluir, e juntar as duas coisas misturaria "terminei isto"
 * com "saiu da mesa".
 */

/**
 * Espera antes de gravar a geometria de um pop-out, em ms.
 *
 * Curto o bastante para sobreviver a um fechamento logo após arrastar, e longo
 * o bastante para um arrasto inteiro virar uma escrita só. O mesmo valor está
 * em `TaskWindowApp`.
 */
const GEOMETRY_DEBOUNCE_MS = 250;

/** Alvo de uma janela destacada, quando esta janela for uma. */
type PopoutTarget = { kind: "note"; id: string } | { kind: "task"; id: string };

/**
 * Descobre se esta janela é um pop-out e de que item.
 *
 * Três canais redundantes, em ordem de confiabilidade:
 *
 * 1. O global injetado pela `initialization_script` do Rust — o único que não
 *    depende de como a URL sobreviveu.
 * 2. Query string (`?note=`), para quando a janela é aberta por link.
 * 3. Hash (`#note=`), que é o que o `WebviewUrl::App` realmente produz, com
 *    variações (`#note=id`, `#/index.html#note=id`, `#?note=id`) dependendo de
 *    dev vs. empacotado.
 *
 * O regex no href inteiro fecha a lista como último recurso. É redundância
 * deliberada: pop-out que não sabe o que renderizar é uma janela inútil que o
 * usuário não consegue fechar.
 */
function detectPopoutTarget(): PopoutTarget | null {
  const href = window.location.href;
  const w = window as unknown as { __NOTE_ID__?: string; __TASK_ID__?: string };

  const injected: Array<[PopoutTarget["kind"], string | undefined]> = [
    ["note", w.__NOTE_ID__],
    ["task", w.__TASK_ID__],
  ];
  for (const [kind, value] of injected) {
    if (value && value.trim()) return { kind, id: value.trim() };
  }

  for (const kind of ["note", "task"] as const) {
    const id = idFromUrl(href, kind);
    if (id) return { kind, id };
  }
  return null;
}

/** Procura `<param>=<uuid>` na query, no hash e, por fim, no href inteiro. */
function idFromUrl(href: string, param: "note" | "task"): string | null {
  const uuid = new RegExp(`${param}=([a-f0-9-]{36})`, "i");

  try {
    const url = new URL(href);

    const fromQuery = url.searchParams.get(param);
    if (fromQuery?.trim()) return fromQuery.trim();

    if (url.hash.includes(`${param}=`)) {
      // Pega o trecho após o último "#" e tenta lê-lo como query string.
      const lastHash = url.hash.split("#").pop() ?? "";
      const fromHash = new URLSearchParams(lastHash.replace(/^\/?/, "")).get(param);
      if (fromHash?.trim()) return fromHash.trim();
    }
  } catch {
    // href malformado: o regex abaixo ainda pode salvar.
  }

  const m = href.match(uuid);
  return m ? m[1] : null;
}

export default function App() {
  const [popout, setPopout] = useState<PopoutTarget | null>(null);
  const [resolved, setResolved] = useState(false);

  useEffect(() => {
    setPopout(detectPopoutTarget());
    // Sem este flag, o primeiro render (antes do efeito) mostraria o app
    // principal por um instante dentro da janela de pop-out.
    setResolved(true);
  }, []);

  if (!resolved) return null;
  if (popout?.kind === "note") return <NoteWindowApp noteId={popout.id} />;
  if (popout?.kind === "task") return <TaskWindowApp taskId={popout.id} />;
  return <MainApp />;
}

/**
 * Renderizado dentro de uma WebviewWindow dedicada (via `?note=<id>`).
 * Sem tabs, sem scroll, sem nav — apenas a nota. Drag/resize nativo do SO é
 * sincronizado de volta para a nota via listeners de janela.
 */
function NoteWindowApp({ noteId }: { noteId: string }) {
  const [note, setNote] = useState<Note | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  // Janela de nota: fundo transparente e sem scroll no html/body
  useEffect(() => {
    document.body.classList.add("note-window-body");
    document.documentElement.classList.add("note-window-body");
    return () => {
      document.body.classList.remove("note-window-body");
      document.documentElement.classList.remove("note-window-body");
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    const timeout = setTimeout(() => {
      if (!cancelled && loading) {
        setError("timeout ao carregar nota (backend não respondeu em 5s)");
        setLoading(false);
      }
    }, 5000);
    (async () => {
      try {
        console.log("NoteWindow: fetching", noteId);
        const n = await api.getNote(noteId);
        if (!cancelled) setNote(n);
      } catch (e) {
        console.error("getNote falhou:", e);
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) {
          clearTimeout(timeout);
          setLoading(false);
        }
      }
    })();
    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, [noteId]);

  // Sincroniza posição/tamanho da janela de nota para persistência.
  useEffect(() => {
    if (!note) return;
    let disposed = false;
    let unlistenFns: Array<() => void> = [];

    const syncPosition = async (physical: { x: number; y: number }) => {
      if (disposed) return;
      try {
        const win = (await import("@tauri-apps/api/window")).getCurrentWindow();
        const scale = await win.scaleFactor();
        const logical = { x: physical.x / scale, y: physical.y / scale };
        await api.updateNote(note.id, { position: [logical.x, logical.y] });
      } catch {
        // best effort — sincronização não bloqueia a UI
      }
    };

    const syncSize = async (physical: { width: number; height: number }) => {
      if (disposed) return;
      try {
        const win = (await import("@tauri-apps/api/window")).getCurrentWindow();
        const scale = await win.scaleFactor();
        const logical = { width: physical.width / scale, height: physical.height / scale };
        await api.updateNote(note.id, { size: [logical.width, logical.height] });
      } catch {
        // best effort
      }
    };

    // Debounce: `onMoved` dispara a cada pixel do arrasto, então arrastar a
    // nota pela tela emitia centenas de escritas no SQLite. O que interessa é
    // onde ela parou.
    //
    // Este caminho só passou a ser exercitado em 2026-09-03: até a correção da
    // capability (`core:window:allow-start-dragging`), arrastar a janela de
    // nota simplesmente não funcionava, e o listener nunca era chamado de
    // verdade.
    let moveTimer: ReturnType<typeof setTimeout> | undefined;
    let sizeTimer: ReturnType<typeof setTimeout> | undefined;

    (async () => {
      const win = (await import("@tauri-apps/api/window")).getCurrentWindow();
      const unMove = await win.onMoved((e) => {
        clearTimeout(moveTimer);
        moveTimer = setTimeout(() => syncPosition(e.payload), GEOMETRY_DEBOUNCE_MS);
      });
      const unResize = await win.onResized((e) => {
        clearTimeout(sizeTimer);
        sizeTimer = setTimeout(() => syncSize(e.payload), GEOMETRY_DEBOUNCE_MS);
      });
      if (!disposed) unlistenFns = [unMove, unResize];
    })();

    return () => {
      disposed = true;
      clearTimeout(moveTimer);
      clearTimeout(sizeTimer);
      unlistenFns.forEach((fn) => fn());
    };
  }, [note]);

  const handleUpdate = async (id: string, patch: Record<string, unknown>) => {
    if (!note) return;
    try {
      const payload: Record<string, unknown> = {};
      if ("title" in patch) payload.title = patch.title;
      if ("content" in patch) payload.content = patch.content;
      if ("color" in patch) payload.color = patch.color;
      if ("opacity" in patch) payload.opacity = patch.opacity;
      if ("position" in patch) payload.position = patch.position;
      if ("size" in patch) payload.size = patch.size;
      const updated = await api.updateNote(id, payload as never);
      setNote(updated);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleCloseWindow = async () => {
    try {
      await api.closeNoteWindow(noteId);
    } catch (e) {
      console.error("closeNoteWindow falhou:", e);
    }
    try {
      const w = (await import("@tauri-apps/api/window")).getCurrentWindow();
      await w.close();
    } catch {
      window.close();
    }
  };

  const handleTogglePin = async (id: string) => {
    try {
      const updated = await api.togglePin(id);
      setNote(updated);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleToggleAot = async (id: string) => {
    if (!note) return;
    try {
      const updated = await api.updateNote(id, { always_on_top: !note.always_on_top });
      await api.setNoteWindowAlwaysOnTop(id, updated.always_on_top);
      setNote(updated);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleArchive = async (id: string) => {
    try {
      await api.archiveNote(id);
      // Arquiva e fecha a janela — a nota volta ao placar apenas em "Arquivadas"
      await api.closeNoteWindow(id);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Deletar esta nota?")) return;
    try {
      await api.deleteNote(id);
      await api.closeNoteWindow(id);
    } catch (e) {
      setError(String(e));
    }
  };

  if (loading) {
    return (
      <div className="note-window-state note-window-state--loading">
        <div>
          <div className="note-window-hint">Carregando nota {noteId.slice(0, 8)}…</div>
          <div className="md-skeleton" style={{ width: 260, marginTop: 12, marginLeft: 0, marginRight: 0 }} />
        </div>
      </div>
    );
  }

  if (!note || error) {
    return (
      <div className="note-window-state" role="alert">
        <div className="md-alert" style={{ margin: 0 }}>
          <strong style={{ fontWeight: 700 }}>Não foi possível carregar a nota.</strong>{" "}
          {error ?? "A nota não foi encontrada — ela pode ter sido deletada."}
          <div className="note-window-id">ID: {noteId}</div>
        </div>
        <button
          className="md-btn"
          style={{ alignSelf: "flex-start" }}
          onClick={async () => {
            try { await api.closeNoteWindow(noteId); } catch {}
            try { const w = (await import("@tauri-apps/api/window")).getCurrentWindow(); await w.close(); } catch { window.close(); }
          }}
        >
          Fechar janela
        </button>
      </div>
    );
  }

  return (
    <div className="note-window-root">
      <NoteCard
        note={note}
        noteWindowMode
        onUpdate={handleUpdate}
        onArchive={handleArchive}
        onDelete={handleDelete}
        onTogglePin={handleTogglePin}
        onToggleAot={handleToggleAot}
        onCloseWindow={handleCloseWindow}
      />
    </div>
  );
}

function MainApp() {
  // Notas abre primeiro (pedido do DEV em 2026-09-30).
  const [tab, setTab] = useState<Section>("notes");
  const [authUser, setAuthUser] = useState<AuthPayload | null>(null);
  const [authLoading, setAuthLoading] = useState(true);
  const [authNotice, setAuthNotice] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    // Rede de segurança: se o backend não responder, cai no login em vez de
    // ficar no esqueleto para sempre. A restauração verifica um hash Argon2
    // (dezenas de ms), então 3 s é folga larga.
    const timeout = setTimeout(() => {
      if (!cancelled) {
        console.warn("authRestoreSession timeout — fallback para AuthPanel");
        setAuthLoading(false);
      }
    }, 3000);
    (async () => {
      try {
        // "Manter conectado": reabre a sessão lembrada, se houver.
        const restored = await api.authRestoreSession();
        if (!cancelled) setAuthUser(restored);
      } catch (e) {
        console.error("authRestoreSession falhou:", e);
        if (!cancelled) setAuthUser(null);
      } finally {
        if (!cancelled) {
          clearTimeout(timeout);
          setAuthLoading(false);
        }
      }
    })();
    return () => {
      cancelled = true;
      clearTimeout(timeout);
    };
  }, []);

  const handleLogout = async () => {
    try {
      await api.authLogout();
      setAuthUser(null);
      setAuthNotice(null);
      setTab("notes");
    } catch (e) {
      // Não finge que saiu: com "manter conectado", um logout que falhou pode
      // deixar a sessão lembrada valendo, e o app reabriria logado.
      setAuthNotice(`Não foi possível sair completamente: ${String(e)}. Tente de novo.`);
    }
  };

  if (authLoading) {
    return (
      <div style={{ height: "100vh", display: "grid", placeItems: "center", background: "var(--canvas)" }}>
        <div className="md-skeleton" style={{ width: 320 }} />
      </div>
    );
  }

  // O toast aparece também na tela de login: quem está com uma versão velha
  // costuma descobrir justamente ali, quando o login falha contra um Mastersys
  // que já mudou. Não aparece nos pop-outs — as saídas antecipadas acima já
  // trataram deles, e duas janelas instalando a mesma atualização seria pior
  // que nenhuma.
  if (!authUser) {
    return (
      <>
        <AuthPanel
          onAuthenticated={(u, rememberFailed) => {
            setAuthUser(u);
            setAuthNotice(
              rememberFailed
                ? "Não foi possível guardar a sessão no Gerenciador de Credenciais — você precisará entrar de novo na próxima abertura."
                : null,
            );
          }}
        />
        <UpdateToast />
      </>
    );
  }

  return (
    <div className="md-shell">
      <Sidebar section={tab} onSection={setTab} username={authUser.username} onLogout={handleLogout} />

      <main className="md-main">
        {authNotice && (
          <div role="status" className="md-alert">
            {authNotice}
            <button type="button" className="md-alert-dismiss" onClick={() => setAuthNotice(null)}>
              Fechar
            </button>
          </div>
        )}

        {/* `key` por seção: Quadro e Chamados são o mesmo componente com
            recortes diferentes, e cada um guarda filtros e seleção próprios. */}
        {tab === "notes" ? (
          <NotesBoard />
        ) : (
          <TasksBoard key={tab} view={tab === "board" ? "board" : "mastersys"} />
        )}
      </main>

      <UpdateToast />
    </div>
  );
}