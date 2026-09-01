import { useEffect, useRef, useState, useCallback } from "react";
import type { Note } from "../types";
import * as api from "../api";
import { NoteCard } from "./NoteCard";

export function NotesBoard() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [showArchived, setShowArchived] = useState(false);
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const [content, setContent] = useState("");
  const titleRef = useRef<HTMLInputElement>(null);
  const [poppedOut, setPoppedOut] = useState<Set<string>>(new Set());
  const poppedOutRef = useRef<Set<string>>(new Set());

  const setPoppedOutBoth = (next: Set<string>) => {
    poppedOutRef.current = next;
    setPoppedOut(next);
  };

  const refresh = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = showArchived ? await api.listArchivedNotes() : await api.listActiveNotes();
      setNotes(data);
      // Reconcilia: remove do `poppedOut` janelas que já foram fechadas
      // (ex.: fechadas pelo botão ✕ na própria janela ou pelo OS).
      if (poppedOutRef.current.size > 0) {
        const stillOpen: string[] = [];
        for (const id of poppedOutRef.current) {
          try {
            if (await api.isNoteWindowOpen(id)) stillOpen.push(id);
          } catch {
            // se a checagem falhar, mantém o id (conservador)
            stillOpen.push(id);
          }
        }
        const next = new Set(stillOpen);
        if (next.size !== poppedOutRef.current.size) {
          setPoppedOutBoth(next);
        }
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [showArchived]);

  // Quando a janela principal volta ao foco (ex.: restaurada da bandeja),
  // reconcilia estado das janelas de nota abertas/fechadas.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    (async () => {
      try {
        const win = (await import("@tauri-apps/api/window")).getCurrentWindow();
        const un = await win.onFocusChanged(({ payload }) => {
          if (payload && !cancelled) refresh();
        });
        if (!cancelled) unlisten = un;
      } catch {
        // fora do Tauri (browser/dev puro) — ignora
      }
    })();
    return () => {
      cancelled = true;
      unlisten?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleCreate = async () => {
    if (!title.trim()) return;
    try {
      const created = await api.createNote({ title: title.trim(), content });
      setNotes((prev) => [created, ...prev]);
      setTitle("");
      setContent("");
      titleRef.current?.focus();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleUpdate = async (id: string, patch: Record<string, unknown>) => {
    try {
      const payload: Record<string, unknown> = {};
      if ("title" in patch) payload.title = patch.title;
      if ("content" in patch) payload.content = patch.content;
      if ("color" in patch) payload.color = patch.color;
      if ("opacity" in patch) payload.opacity = patch.opacity;
      if ("position" in patch) payload.position = patch.position;
      if ("size" in patch) payload.size = patch.size;
      const updated = await api.updateNote(id, payload as never);
      setNotes((prev) => prev.map((n) => (n.id === id ? updated : n)));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleArchive = async (id: string) => {
    try {
      if (showArchived) {
        await api.unarchiveNote(id);
        await refresh();
      } else {
        await api.archiveNote(id);
        setNotes((prev) => prev.filter((n) => n.id !== id));
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Deletar esta nota?")) return;
    try {
      await api.deleteNote(id);
      setNotes((prev) => prev.filter((n) => n.id !== id));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleTogglePin = async (id: string) => {
    try {
      const updated = await api.togglePin(id);
      setNotes((prev) => prev.map((n) => (n.id === id ? updated : n)));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleToggleAot = async (id: string) => {
    const target = notes.find((n) => n.id === id);
    if (!target) return;
    try {
      const updated = await api.setAlwaysOnTop(id, !target.always_on_top);
      await api.setWindowAlwaysOnTop(updated.always_on_top);
      setNotes((prev) => prev.map((n) => (n.id === id ? updated : n)));
    } catch (e) {
      setError(String(e));
    }
  };

  const handlePopOut = useCallback(async (id: string) => {
    const target = notes.find((n) => n.id === id);
    if (!target) return;
    try {
      await api.openNoteWindow(
        target.id,
        target.title,
        target.color,
        target.position[0],
        target.position[1],
        target.size[0],
        target.size[1],
      );
      setPoppedOutBoth(new Set(poppedOutRef.current).add(id));
    } catch (e) {
      setError(String(e));
    }
  }, [notes]);

  const handleCloseWindow = useCallback(async (id: string) => {
    try {
      await api.closeNoteWindow(id);
      const next = new Set(poppedOutRef.current);
      next.delete(id);
      setPoppedOutBoth(next);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const filtered = notes.filter((n) => {
    // Hide notes that are popped out into their own windows
    if (poppedOut.has(n.id)) return false;
    if (!filter.trim()) return true;
    const q = filter.toLowerCase();
    return n.title.toLowerCase().includes(q) || n.content.toLowerCase().includes(q) || n.tags.some((t) => t.includes(q));
  });

  const isEmpty = !loading && filtered.length === 0;

  return (
    <div style={{ fontFamily: "inherit", height: "100%", display: "flex", flexDirection: "column", minHeight:0 }}>
      <header className="md-board-header">
        <div className="md-search" role="search">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden>
            <path d="M10.5 18a7.5 7.5 0 1 1 0-15 7.5 7.5 0 0 1 0 15Zm0-13.5a6 6 0 1 0 0 12 6 6 0 0 0 0-12ZM16.2 16.2 21 21" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round"/>
          </svg>
          <input
            placeholder="Buscar por título, conteúdo ou #tag"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            aria-label="Buscar notas"
          />
        </div>

        <label className="md-toggle">
          <input type="checkbox" checked={showArchived} onChange={(e) => setShowArchived(e.target.checked)} />
          Arquivadas
        </label>

        <span className="md-count" aria-live="polite">
          {filtered.length} {filtered.length === 1 ? "nota" : "notas"}
          {filter.trim() ? " • filtradas" : ""}
        </span>
      </header>

      <div className="md-create-bar">
        <div className="md-field" style={{ flex:"0 0 240px" }}>
          <label htmlFor="note-title">Título</label>
          <input
            id="note-title"
            ref={titleRef}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Título da nota"
            maxLength={200}
            className="md-input"
            onKeyDown={(e)=>{ if(e.key==="Enter") handleCreate(); }}
          />
        </div>
        <div className="md-field" style={{ flex:1, minWidth:180 }}>
          <label htmlFor="note-content">Conteúdo</label>
          <input
            id="note-content"
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="Conteúdo opcional — pressione Enter para criar"
            className="md-input"
            onKeyDown={(e)=>{ if(e.key==="Enter") handleCreate(); }}
          />
        </div>
        <button
          onClick={handleCreate}
          disabled={!title.trim()}
          className="md-primary md-primary-accent"
          aria-disabled={!title.trim()}
        >
          Nova nota
        </button>
      </div>

      {error && (
        <div role="alert" className="md-alert">
          <strong style={{fontWeight:700}}>Algo deu errado:</strong> {error}{" "}
          <button onClick={()=>setError(null)} style={{marginLeft:8, fontSize:12, textDecoration:"underline", background:"transparent", border:"none", cursor:"pointer", color:"inherit"}}>dispensar</button>
        </div>
      )}

      {/* Canvas: scroll invisível mas funcional */}
      <div
        className={`scroll-hidden canvas-desk ${isEmpty ? "" : ""}`}
        style={{
          position:"relative",
          flex:1,
          minHeight:0,
          // quando vazio, sem overflow para não mostrar gutter; quando tem notas, mantém scroll mas hidden
          overflow: isEmpty ? "hidden" : undefined,
          display: isEmpty ? "flex" : "block",
          flexDirection: isEmpty ? "column" as const : undefined,
        }}
        aria-busy={loading}
      >
        {loading ? (
          <div style={{ padding:"14px" }}>
            <div className="md-skeleton" />
            <div className="md-skeleton" style={{ width:"88%" }} />
            <div className="md-skeleton" style={{ width:"76%" }} />
          </div>
        ) : isEmpty ? (
          <EmptyState
            showArchived={showArchived}
            hasFilter={Boolean(filter.trim())}
            onClearFilter={()=>setFilter("")}
            onFocusCreate={()=>titleRef.current?.focus()}
          />
        ) : (
          <>
            {/* área virtual para absolute cards — garante altura mínima para scroll */}
            <div style={{ position:"relative", minHeight:"100%", minWidth:"100%", height: filtered.length>0 ? "720px" : "100%" }}>
              {filtered.map((n) => (
                <NoteCard
                  key={n.id}
                  note={n}
                  onUpdate={handleUpdate}
                  onArchive={handleArchive}
                  onDelete={handleDelete}
                  onTogglePin={handleTogglePin}
                  onToggleAot={handleToggleAot}
                  onPopOut={handlePopOut}
                  onCloseWindow={handleCloseWindow}
                />
              ))}
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function EmptyState({ showArchived, hasFilter, onClearFilter, onFocusCreate }: {
  showArchived:boolean; hasFilter:boolean; onClearFilter:()=>void; onFocusCreate:()=>void
}){
  if (hasFilter) {
    return (
      <div className="md-empty" role="status" aria-live="polite">
        <div className="md-empty-illus" aria-hidden>
          <span style={{ fontSize:22, position:"relative", zIndex:1 }}>🔎</span>
        </div>
        <h3>Nenhum resultado</h3>
        <p>Nenhuma nota corresponde à sua busca. Tente outros termos ou limpe o filtro.</p>
        <button className="md-empty-cta" onClick={onClearFilter}>Limpar busca</button>
      </div>
    );
  }
  if (showArchived) {
    return (
      <div className="md-empty" role="status">
        <div className="md-empty-illus" aria-hidden>
          <span style={{ fontSize:22, position:"relative", zIndex:1 }}>🗄️</span>
        </div>
        <h3>Nenhuma nota arquivada</h3>
        <p>Quando você arquivar uma nota, ela aparece aqui. Arquivar mantém a mesa limpa sem perder o conteúdo.</p>
      </div>
    );
  }
  return (
    <div className="md-empty" role="status">
      <div className="md-empty-illus" aria-hidden>
        {/* sticky note icon */}
        <svg width="28" height="28" viewBox="0 0 24 24" fill="none" aria-hidden style={{ position:"relative", zIndex:1 }}>
          <rect x="4" y="4" width="14" height="14" rx="2.5" fill="white" stroke="#0F1115" strokeWidth="1.4"/>
          <rect x="7.2" y="7.2" width="14" height="14" rx="2.5" fill="#FFEB3B" stroke="#0F1115" strokeWidth="1.4"/>
          <path d="M11 11h6M11 14.5h6" stroke="#0F1115" strokeWidth="1.2" strokeLinecap="round"/>
        </svg>
      </div>
      <h3>Sua mesa está limpa</h3>
      <p>Crie a primeira nota acima. Arraste para organizar, troque a cor e ajuste a opacidade — tudo fica sobre uma mesa pontilhada.</p>
      <div style={{ display:"flex", gap:8, flexWrap:"wrap", justifyContent:"center" }}>
        <button className="md-empty-cta md-empty-cta--primary" onClick={onFocusCreate}>Criar primeira nota</button>
        <span style={{ fontSize:12, color:"var(--muted)", alignSelf:"center" }}>dica: Enter cria rápido</span>
      </div>
    </div>
  );
}
