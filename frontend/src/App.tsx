import { useEffect, useState } from "react";
import { NotesBoard } from "./components/NotesBoard";
import { TasksBoard } from "./components/TasksBoard";
import { AuthPanel } from "./components/AuthPanel";
import { NoteCard } from "./components/NoteCard";
import type { AuthPayload, Note } from "./types";
import * as api from "./api";

type Tab = "notes" | "tasks";

export default function App() {
  const [noteParam, setNoteParam] = useState<string | null>(null);

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const n = params.get("note");
    setNoteParam(n && n.trim() ? n.trim() : null);
  }, []);

  // Modo janela de nota: URL ?note=<id> → renderiza apenas a nota isolada.
  if (noteParam !== null) {
    return <NoteWindowApp noteId={noteParam} />;
  }

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
    (async () => {
      try {
        const n = await api.getNote(noteId);
        if (!cancelled) setNote(n);
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
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

    (async () => {
      const win = (await import("@tauri-apps/api/window")).getCurrentWindow();
      const unMove = await win.onMoved((e) => syncPosition(e.payload));
      const unResize = await win.onResized((e) => syncSize(e.payload));
      if (!disposed) unlistenFns = [unMove, unResize];
    })();

    return () => {
      disposed = true;
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
      setError(String(e));
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
      <div className="note-window-root" style={{ display: "grid", placeItems: "center" }}>
        <div className="md-skeleton" style={{ width: 260 }} />
      </div>
    );
  }

  if (!note || error) {
    return (
      <div className="note-window-root" role="alert">
        <div style={{ padding: "14px", fontSize: 13 }}>
          <strong style={{ fontWeight: 700 }}>Não foi possível carregar a nota:</strong>{" "}
          {error ?? "nota não encontrada"}
        </div>
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
  const [tab, setTab] = useState<Tab>("notes");
  const [authUser, setAuthUser] = useState<AuthPayload | null>(null);
  const [authLoading, setAuthLoading] = useState(true);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const authed = await api.authIsAuthenticated();
        if (!cancelled) {
          // Se o backend tem sessão ativa mas ainda não sabemos quem é,
          // deixamos o AuthPanel decidir (o payload de login/register traz o usuário).
          if (!authed) setAuthUser(null);
        }
      } catch {
        if (!cancelled) setAuthUser(null);
      } finally {
        if (!cancelled) setAuthLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleLogout = async () => {
    try {
      await api.authLogout();
      setAuthUser(null);
      setTab("notes");
    } catch {
      // falha de logout não bloqueia UI
      setAuthUser(null);
    }
  };

  if (authLoading) {
    return (
      <div style={{ height: "100vh", display: "grid", placeItems: "center", background: "var(--canvas-dot)" }}>
        <div className="md-skeleton" style={{ width: 320 }} />
      </div>
    );
  }

  if (!authUser) {
    return <AuthPanel onAuthenticated={(u) => setAuthUser(u)} />;
  }

  return (
    <div style={{ height: "100vh", display: "flex", flexDirection: "column", background:"var(--paper)" }}>
      <nav className="md-nav" role="tablist" aria-label="Seções do MasterDesk">
        <div className="md-brand" aria-label="MasterDesk">
          <div className="md-brand-mark" aria-hidden>MD</div>
          <div style={{ display:"flex", flexDirection:"column", lineHeight:1 }}>
            <span style={{ fontSize:14, letterSpacing:"-.02em" }}>MasterDesk</span>
            <small>notas • tarefas • foco</small>
          </div>
        </div>

        <div className="md-tabs">
          <button
            role="tab"
            aria-selected={tab === "notes"}
            aria-controls="panel-notes"
            id="tab-notes"
            onClick={() => setTab("notes")}
            className="md-tab"
          >
            Notas
          </button>
          <button
            role="tab"
            aria-selected={tab === "tasks"}
            aria-controls="panel-tasks"
            id="tab-tasks"
            onClick={() => setTab("tasks")}
            className="md-tab"
          >
            Tarefas
          </button>
        </div>

        <div className="md-nav-right">
          <span style={{ display:"flex", alignItems:"center", gap:6 }}>
            <span style={{ width:7, height:7, borderRadius:"50%", background:"var(--accent)", display:"inline-block", boxShadow:"0 0 0 4px rgba(255,235,59,.18)" }} />
            @{authUser.username}
          </span>
          <span style={{ opacity:.45 }}>•</span>
          <button
            onClick={handleLogout}
            className="md-tab"
            style={{ padding:"6px 12px", fontSize:12, minHeight:30 }}
            title="Sair"
          >
            Sair
          </button>
        </div>
      </nav>

      <div style={{ flex:1, minHeight:0, display:"flex", flexDirection:"column" }}>
        {tab === "notes" ? (
          <div role="tabpanel" id="panel-notes" aria-labelledby="tab-notes" style={{ display:"flex", flexDirection:"column", flex:1, minHeight:0 }}>
            <NotesBoard />
          </div>
        ) : (
          <div role="tabpanel" id="panel-tasks" aria-labelledby="tab-tasks" style={{ display:"flex", flexDirection:"column", flex:1, minHeight:0 }}>
            <TasksBoard />
          </div>
        )}
      </div>
    </div>
  );
}