import { useEffect, useRef, useState, useCallback } from "react";
import type { Note } from "../types";
import * as api from "../api";
import { NoteCard } from "./NoteCard";
import { NoteFormModal } from "./NoteFormModal";
import { findFreeSpot } from "../notes/placement";
import {
  clampZoom, DEFAULT_VIEWPORT, fitRects, parseViewport, screenToWorld, zoomAt, type Viewport,
} from "../notes/viewport";

/** Mesmo default do domínio (`Note::new`), usado só para achar espaço livre. */
const NEW_NOTE_SIZE = { w: 300, h: 250 };
/** A câmera do quadro é conveniência local por máquina, não estado do domínio. */
const VIEWPORT_STORAGE_KEY = "masterdesk.notes.viewport";
const ZOOM_STEP = 1.2;
const DOT_GRID = 22;

function loadViewport(): Viewport {
  try {
    return parseViewport(localStorage.getItem(VIEWPORT_STORAGE_KEY));
  } catch {
    return DEFAULT_VIEWPORT;
  }
}

const noteRects = (list: Note[]) =>
  list.map((n) => ({ x: n.position[0], y: n.position[1], w: n.size[0], h: n.size[1] }));

export function NotesBoard() {
  const [notes, setNotes] = useState<Note[]>([]);
  const [showArchived, setShowArchived] = useState(false);
  const [filter, setFilter] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  /** `null` = diálogo fechado; senão, onde a nota vai nascer. */
  const [creating, setCreating] = useState<[number, number] | null>(null);
  const [viewport, setViewport] = useState<Viewport>(loadViewport);
  const [panning, setPanning] = useState(false);
  const canvasRef = useRef<HTMLDivElement>(null);
  const panRef = useRef<{ x: number; y: number; panX: number; panY: number } | null>(null);
  /** Devolve o foco ao botão quando o diálogo fecha (WAI-ARIA dialog). */
  const createRef = useRef<HTMLButtonElement>(null);
  const [poppedOut, setPoppedOut] = useState<Set<string>>(new Set());
  const poppedOutRef = useRef<Set<string>>(new Set());

  const setPoppedOutBoth = (next: Set<string>) => {
    poppedOutRef.current = next;
    setPoppedOut(next);
  };

  /**
   * `silent`: recarga de fundo (volta do foco) não mostra esqueleto nem erro —
   * mesmo motivo documentado no `refresh` do `TasksBoard`: a lista sumia e
   * voltava a cada alt-tab.
   */
  const refresh = async ({ silent = false }: { silent?: boolean } = {}) => {
    try {
      if (!silent) {
        setLoading(true);
        setError(null);
      }
      const data = showArchived ? await api.listArchivedNotes() : await api.listActiveNotes();
      setNotes(data);

      // Pergunta ao gerenciador de janelas quais notas estão destacadas, em vez
      // de confiar no que este componente lembra.
      //
      // O estado local se perdia na troca de aba (o `App` desmonta o board
      // quando você vai para Tarefas), e a nota voltava ao quadro com a janela
      // dela ainda aberta — duas superfícies editando a mesma nota.
      //
      // Uma chamada em vez de uma por nota, e em falha o conjunto fica VAZIO,
      // não preservado: em dúvida, mostrar a nota. Nota duplicada no quadro é
      // recuperável; nota invisível nos dois lugares não era.
      try {
        setPoppedOutBoth(new Set(await api.openNoteWindowIds()));
      } catch {
        setPoppedOutBoth(new Set());
      }
    } catch (e) {
      if (!silent) setError(String(e));
    } finally {
      if (!silent) setLoading(false);
    }
  };
  // O listener de foco é registrado uma vez só; sem a ref ele chamaria o
  // `refresh` da primeira renderização, com `showArchived` congelado.
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;

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
          if (payload && !cancelled) void refreshRef.current({ silent: true });
        });
        // Desmontou durante o `await`: remove já, senão o listener vaza.
        if (!cancelled) unlisten = un;
        else un();
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

  const handleCreated = (created: Note) => {
    setNotes((prev) => [created, ...prev]);
    // Nota nova numa lista de arquivadas não seria vista: recarrega para o
    // quadro mostrar o que a aba atual realmente contém.
    if (showArchived) void refresh();
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

  useEffect(() => {
    try {
      localStorage.setItem(VIEWPORT_STORAGE_KEY, JSON.stringify(viewport));
    } catch {
      // armazenamento indisponível: a câmera só não é lembrada.
    }
  }, [viewport]);

  /**
   * Roda do mouse no estilo Figma: Ctrl/⌘ (e a pinça do touchpad, que chega
   * como Ctrl+wheel) dá zoom no cursor; sem modificador, desloca o quadro.
   * Listener nativo porque o `onWheel` do React é passivo e não deixa
   * cancelar o zoom da própria webview.
   */
  useEffect(() => {
    const el = canvasRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if ((e.target as HTMLElement).closest("textarea, .md-note-content")) return;
      e.preventDefault();
      const rect = el.getBoundingClientRect();
      if (e.ctrlKey || e.metaKey) {
        const factor = Math.exp(-e.deltaY * 0.002);
        setViewport((vp) => zoomAt(vp, vp.zoom * factor, e.clientX - rect.left, e.clientY - rect.top));
      } else {
        const dx = e.shiftKey ? e.deltaY : e.deltaX;
        const dy = e.shiftKey ? 0 : e.deltaY;
        setViewport((vp) => ({ ...vp, panX: vp.panX - dx, panY: vp.panY - dy }));
      }
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  /** Arrastar o fundo (ou o botão do meio em qualquer lugar) move o quadro. */
  const handleCanvasPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    const target = e.target as HTMLElement;
    if (target.closest(".md-zoom-controls")) return;
    const onNote = target.closest(".md-note");
    if (!(e.button === 1 || (e.button === 0 && !onNote))) return;
    e.preventDefault();
    panRef.current = { x: e.clientX, y: e.clientY, panX: viewport.panX, panY: viewport.panY };
    setPanning(true);
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const handleCanvasPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const start = panRef.current;
    if (!start) return;
    setViewport((vp) => ({
      ...vp,
      panX: start.panX + e.clientX - start.x,
      panY: start.panY + e.clientY - start.y,
    }));
  };
  const handleCanvasPointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!panRef.current) return;
    panRef.current = null;
    setPanning(false);
    e.currentTarget.releasePointerCapture(e.pointerId);
  };

  const zoomFromCenter = (next: number) => {
    const rect = canvasRef.current?.getBoundingClientRect();
    setViewport((vp) => zoomAt(vp, next, (rect?.width ?? 0) / 2, (rect?.height ?? 0) / 2));
  };

  const fitAll = () => {
    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    setViewport(fitRects(noteRects(filtered), rect.width, rect.height));
  };

  /** Abre o diálogo já sabendo onde há espaço livre na área visível. */
  const openCreate = () => {
    const rect = canvasRef.current?.getBoundingClientRect();
    const [vx, vy] = screenToWorld(viewport, 0, 0);
    const visible = {
      x: vx,
      y: vy,
      w: (rect?.width ?? 1200) / viewport.zoom,
      h: (rect?.height ?? 800) / viewport.zoom,
    };
    // Na aba de arquivadas o quadro ativo não está carregado; a nota nasce no
    // canto da área visível. Destacadas continuam ocupando o seu lugar.
    const occupied = showArchived ? [] : noteRects(notes);
    setCreating(findFreeSpot(occupied, NEW_NOTE_SIZE, visible));
  };

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

      {/* A criação virou diálogo: o conteúdo da nota não caber numa linha era
          o limite do formulário em barra (ver `NoteFormModal`). O espaço que a
          barra ocupava em todas as telas volta para as notas. */}
      <div className="md-create-bar">
        <button
          ref={createRef}
          onClick={openCreate}
          className="md-primary md-primary-accent"
        >
          Nova nota
        </button>
        <span className="md-panel-note" style={{ margin: 0 }}>
          Título, conteúdo e cor no diálogo.
        </span>
      </div>

      {error && (
        <div role="alert" className="md-alert">
          <strong style={{fontWeight:700}}>Algo deu errado:</strong> {error}{" "}
          <button onClick={()=>setError(null)} style={{marginLeft:8, fontSize:12, textDecoration:"underline", background:"transparent", border:"none", cursor:"pointer", color:"inherit"}}>dispensar</button>
        </div>
      )}

      {/* Canvas infinito: pan arrastando o fundo, zoom com Ctrl+roda. */}
      <div
        ref={canvasRef}
        className={`canvas-desk md-notes-canvas ${panning ? "is-panning" : ""}`}
        style={{
          position:"relative",
          flex:1,
          minHeight:0,
          overflow:"hidden",
          display: isEmpty ? "flex" : "block",
          flexDirection: isEmpty ? "column" as const : undefined,
          // A grade de pontos acompanha a câmera, senão o pan não "se sente".
          backgroundSize: `${DOT_GRID * viewport.zoom}px ${DOT_GRID * viewport.zoom}px`,
          backgroundPosition: `${viewport.panX}px ${viewport.panY}px`,
        }}
        onPointerDown={isEmpty || loading ? undefined : handleCanvasPointerDown}
        onPointerMove={handleCanvasPointerMove}
        onPointerUp={handleCanvasPointerUp}
        onPointerCancel={handleCanvasPointerUp}
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
            onFocusCreate={openCreate}
          />
        ) : (
          <>
            <div
              className="md-notes-world"
              style={{ transform: `translate(${viewport.panX}px, ${viewport.panY}px) scale(${viewport.zoom})` }}
            >
              {filtered.map((n) => (
                <NoteCard
                  key={n.id}
                  note={n}
                  zoom={viewport.zoom}
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
            <div className="md-zoom-controls" role="toolbar" aria-label="Zoom do quadro">
              <button onClick={() => zoomFromCenter(viewport.zoom / ZOOM_STEP)} aria-label="Diminuir zoom" title="Diminuir zoom (Ctrl+roda)">−</button>
              <button onClick={() => zoomFromCenter(1)} title="Voltar a 100%" className="md-zoom-value">
                {Math.round(clampZoom(viewport.zoom) * 100)}%
              </button>
              <button onClick={() => zoomFromCenter(viewport.zoom * ZOOM_STEP)} aria-label="Aumentar zoom" title="Aumentar zoom (Ctrl+roda)">+</button>
              <button onClick={fitAll} title="Enquadrar todas as notas">Ajustar</button>
            </div>
          </>
        )}
      </div>

      {creating && (
        <NoteFormModal
          position={creating}
          onClose={() => {
            setCreating(null);
            createRef.current?.focus();
          }}
          onCreated={handleCreated}
        />
      )}
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
          <rect x="4" y="4" width="14" height="14" rx="2.5" fill="var(--surface-plain)" stroke="var(--text)" strokeWidth="1.4"/>
          <rect x="7.2" y="7.2" width="14" height="14" rx="2.5" fill="var(--accent)" stroke="var(--text)" strokeWidth="1.4"/>
          <path d="M11 11h6M11 14.5h6" stroke="var(--accent-ink)" strokeWidth="1.2" strokeLinecap="round"/>
        </svg>
      </div>
      <h3>Sua mesa está limpa</h3>
      <p>Crie a primeira nota acima. Arraste para organizar, troque a cor e ajuste a opacidade — tudo fica sobre uma mesa pontilhada.</p>
      <div style={{ display:"flex", gap:8, flexWrap:"wrap", justifyContent:"center" }}>
        <button className="md-empty-cta md-empty-cta--primary" onClick={onFocusCreate}>Criar primeira nota</button>
        <span style={{ fontSize:12, color:"var(--text-muted)", alignSelf:"center" }}>dica: Ctrl+Enter salva no diálogo</span>
      </div>
    </div>
  );
}
