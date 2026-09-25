import { useEffect, useState, useRef } from "react";
import type { Note } from "../types";
import { noteSurface, noteSwatch, parseHex } from "../theme/noteSurface";
import { useTheme } from "../theme/useTheme";

const COLORS = ["#FFEB3B", "#FF9800", "#8BC34A", "#03A9F4", "#E91E63", "#9C27B0", "#FFFFFF", "#263238"] as const;
/** Mesmo valor do debounce de geometria do pop-out (`App.tsx`): o seletor de
 *  cor e o slider disparam um evento por passo, e cada um virava um UPDATE. */
const APPEARANCE_DEBOUNCE_MS = 250;

type Appearance = { color?: string; opacity?: number };

const COLOR_LABEL: Record<string,string> = {
  "#FFEB3B":"Amarelo", "#FF9800":"Laranja", "#8BC34A":"Verde", "#03A9F4":"Azul",
  "#E91E63":"Rosa", "#9C27B0":"Roxo", "#FFFFFF":"Branco", "#263238":"Grafite"
};

interface Props {
  note: Note;
  onUpdate: (id: string, patch: Partial<Note> & { title?: string; content?: string; color?: string; opacity?: number; position?: [number, number]; size?: [number, number] }) => void;
  onArchive: (id: string) => void;
  onDelete: (id: string) => void;
  onTogglePin: (id: string) => void;
  onToggleAot: (id: string) => void;
  onPopOut?: (id: string) => void;
  onCloseWindow?: (id: string) => void;
  /** True when rendering in an isolated note-window (via ?note= URL) */
  noteWindowMode?: boolean;
}

export function NoteCard({ note, onUpdate, onArchive, onDelete, onTogglePin, onToggleAot, onPopOut, onCloseWindow, noteWindowMode }: Props) {
  const { theme } = useTheme();
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(note.title);
  const [content, setContent] = useState(note.content);
  const dragRef = useRef<{ x: number; y: number; orig: [number, number] } | null>(null);

  // Cor/opacidade em arrasto: a tela acompanha na hora (`draft`), o banco só
  // recebe o valor quando o arrasto para. Gravar a cada passo gerava dezenas
  // de IPC + UPDATE, e respostas fora de ordem podiam deixar um valor
  // intermediário como final.
  const [draft, setDraft] = useState<Appearance>({});
  const pendingRef = useRef<Appearance>({});
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onUpdateRef = useRef(onUpdate);
  onUpdateRef.current = onUpdate;
  const noteIdRef = useRef(note.id);
  noteIdRef.current = note.id;

  const flushAppearance = () => {
    if (timerRef.current) {
      clearTimeout(timerRef.current);
      timerRef.current = null;
    }
    const patch = pendingRef.current;
    if (patch.color === undefined && patch.opacity === undefined) return;
    pendingRef.current = {};
    onUpdateRef.current(noteIdRef.current, patch);
  };

  const previewAppearance = (patch: Appearance) => {
    pendingRef.current = { ...pendingRef.current, ...patch };
    setDraft((d) => ({ ...d, ...patch }));
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(flushAppearance, APPEARANCE_DEBOUNCE_MS);
  };

  // Fechar o card (ou a janela) no meio do arrasto não perde o último valor.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  useEffect(() => () => flushAppearance(), []);

  // Chegou o valor salvo e não há nada pendente: o rascunho sai de cena.
  useEffect(() => {
    if (pendingRef.current.color === undefined && pendingRef.current.opacity === undefined) {
      setDraft({});
    }
  }, [note.color, note.opacity]);

  const color = draft.color ?? note.color;
  const opacity = draft.opacity ?? note.opacity;

  const save = () => {
    setEditing(false);
    if (title !== note.title || content !== note.content) {
      onUpdate(note.id, { title, content });
    }
  };

  /**
   * Faz do cabeçalho a alça de arrastar da janela destacada.
   *
   * O Tauri só olha a **presença** de `data-tauri-drag-region`; o valor é
   * ignorado. Antes estava `"deep"`, que não significa nada aqui e sugeria um
   * comportamento inexistente.
   *
   * Arrastar exige ainda `core:window:allow-start-dragging` na capability.
   * `core:window:default` concede apenas as 28 permissões de leitura, então a
   * chamada era negada pela ACL **em silêncio** — era essa a razão de a janela
   * de nota não arrastar, e não o código daqui.
   */
  const dragRegionProps = noteWindowMode
    ? ({ "data-tauri-drag-region": "" } as React.HTMLAttributes<HTMLDivElement>)
    : {};

  const handleMouseDown = (e: React.MouseEvent) => {
    // In note-window mode, don't prevent default — let the OS handle window dragging
    if (noteWindowMode) return;

    const startX = e.clientX;
    const startY = e.clientY;
    dragRef.current = { x: startX, y: startY, orig: [...note.position] as [number, number] };
    const onMove = (ev: MouseEvent) => {
      if (!dragRef.current) return;
      const dx = ev.clientX - dragRef.current.x;
      const dy = ev.clientY - dragRef.current.y;
      const nx = dragRef.current.orig[0] + dx;
      const ny = dragRef.current.orig[1] + dy;
      const el = document.getElementById(`note-${note.id}`);
      if (el) {
        el.style.left = `${nx}px`;
        el.style.top = `${ny}px`;
      }
    };
    const onUp = (ev: MouseEvent) => {
      if (!dragRef.current) return;
      const dx = ev.clientX - dragRef.current.x;
      const dy = ev.clientY - dragRef.current.y;
      const nx = dragRef.current.orig[0] + dx;
      const ny = dragRef.current.orig[1] + dy;
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      dragRef.current = null;
      onUpdate(note.id, { position: [nx, ny] });
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  };

  const handleResizeEnd = (newW: number, newH: number) => {
    onUpdate(note.id, { size: [newW, newH] });
  };

  // A cor da nota é do usuário, então não pode virar token de CSS: o tema
  // escuro a remapeia para uma variante profunda da mesma família, e o texto
  // sai do contraste calculado (WCAG), não de uma lista de cores conhecidas.
  const surface = noteSurface(color, theme === "dark");
  const ink = surface.text;
  const headBtnStyle: React.CSSProperties = {
    color: ink,
    borderColor: surface.hairline,
    background: surface.chip,
  };

  return (
    <div
      id={`note-${note.id}`}
      className={`md-note ${note.pinned ? "md-note--pinned" : ""} ${noteWindowMode ? "md-note--window" : ""}`}
      style={noteWindowMode ? {
        position: "relative",
        left: "auto",
        top: "auto",
        width: "100%",
        height: "100%",
        borderRadius: 0,
        border: "none",
        background: surface.background,
        opacity,
        color: ink,
        resize: "none",
      } : {
        left: note.position[0],
        top: note.position[1],
        width: note.size[0],
        height: note.size[1],
        background: surface.background,
        borderColor: surface.border,
        opacity,
        color: ink,
      }}
      onMouseUp={(e) => {
        if (noteWindowMode) return;
        const rect = (e.currentTarget as HTMLDivElement).getBoundingClientRect();
        if (Math.abs(rect.width - note.size[0]) > 2 || Math.abs(rect.height - note.size[1]) > 2) {
          handleResizeEnd(Math.round(rect.width), Math.round(rect.height));
        }
      }}
      role="article"
      aria-label={note.title}
    >
      <div
        onMouseDown={handleMouseDown}
        {...dragRegionProps}
        className="md-note-head"
        style={{
          color: ink,
          background: surface.headBackground,
          borderBottomColor: surface.hairline,
        }}
      >
        <span className="md-grip" aria-hidden>⋮⋮</span>
        <span className="md-note-title" title={note.title}>
          {note.title} {note.pinned ? "📌" : ""} {note.always_on_top ? "⬆" : ""}
        </span>
        <button onClick={() => onTogglePin(note.id)} title={note.pinned ? "Desafixar" : "Fixar"} aria-pressed={note.pinned} className="md-icon-btn" style={headBtnStyle}>
          {note.pinned ? "Unpin" : "Pin"}
        </button>
        <button onClick={() => onToggleAot(note.id)} title="Always on top" aria-pressed={note.always_on_top} className="md-icon-btn" style={headBtnStyle}>
          {note.always_on_top ? "AOT off" : "AOT on"}
        </button>
        {noteWindowMode ? (
          <button
            onClick={() => onCloseWindow?.(note.id)}
            title="Fechar janela"
            className="md-icon-btn"
            style={headBtnStyle}
          >
            ✕
          </button>
        ) : (
          <button
            onClick={() => onPopOut?.(note.id)}
            title="Destacar em janela separada"
            className="md-icon-btn"
            style={headBtnStyle}
          >
            Pop-out
          </button>
        )}
      </div>

      <div className="md-note-body">
        {editing ? (
          <>
            <input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Título"
              maxLength={200}
              className="md-input-sm"
              autoFocus
            />
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="Conteúdo"
              rows={4}
              className="md-input-sm"
              style={{ resize:"vertical", minHeight:72 }}
            />
            <div style={{ display:"flex", gap:6 }}>
              <button onClick={save} className="md-icon-btn" style={{ background: ink, color: surface.background, borderColor: ink, fontWeight:700, opacity:1 }}>
                Salvar
              </button>
              <button
                onClick={() => {
                  setTitle(note.title);
                  setContent(note.content);
                  setEditing(false);
                }}
                className="md-icon-btn"
              >
                Cancelar
              </button>
            </div>
          </>
        ) : (
          <>
            <div className="md-note-content">
              {note.content || <i className="md-note-empty">sem conteúdo</i>}
            </div>
            <button onClick={() => setEditing(true)} className="md-icon-btn" style={{ alignSelf:"flex-start" }}>
              Editar
            </button>
          </>
        )}

        <div className="md-swatches" role="group" aria-label="Escolher cor">
          {COLORS.map((c) => {
            const selected = c === color;
            return (
              <button
                key={c}
                onClick={() => {
                  // Pelo mesmo caminho do arrasto, para um flush atrasado de
                  // cor personalizada não sobrescrever este clique. Grava já.
                  previewAppearance({ color: c });
                  flushAppearance();
                }}
                title={COLOR_LABEL[c] ?? c}
                aria-label={`Cor ${COLOR_LABEL[c] ?? c}`}
                aria-pressed={selected}
                className="md-swatch"
                // A amostra mostra a cor como ela vai ficar NESTE tema.
                style={{ background: noteSwatch(c), color: ink }}
              >
                {selected && (
                  <span aria-hidden style={{ fontSize:10, lineHeight:1, fontWeight:800, color: noteSurface(c, theme === "dark").text }}>✓</span>
                )}
              </button>
            );
          })}
          <label
            title="Cor personalizada"
            aria-label="Cor personalizada"
            aria-pressed={!COLORS.includes(color as typeof COLORS[number])}
            className="md-swatch"
            style={{
              background:"conic-gradient(from 0deg, #f44336, #ffeb3b, #8bc34a, #03a9f4, #9c27b0, #f44336)",
              color: ink,
              overflow:"hidden",
            }}
          >
            <span aria-hidden style={{ fontSize:10, lineHeight:1, fontWeight:800, background:"#fff", color:"#0F1115", borderRadius:"50%", width:10, height:10, display:"grid", placeItems:"center" }}>+</span>
            <input
              type="color"
              value={parseHex(color) ? color : "#FFEB3B"}
              onInput={(e) => previewAppearance({ color: (e.target as HTMLInputElement).value })}
              style={{ position:"absolute", inset:0, opacity:0, cursor:"pointer", width:"100%", height:"100%" }}
            />
          </label>
        </div>

        <label style={{ fontSize:11, fontWeight:600, letterSpacing:".04em", textTransform:"uppercase", opacity:.75, display:"flex", alignItems:"center", gap:7, flexWrap:"wrap" }}>
          Opacidade
          <input type="range" min={0.1} max={1} step={0.05} value={opacity} onChange={(e) => previewAppearance({ opacity: parseFloat(e.target.value) })} aria-label="Opacidade" style={{ flex:1 }} />
          <span style={{ fontVariantNumeric:"tabular-nums", fontSize:12, opacity:1, textTransform:"none", letterSpacing:0 }}>{Math.round(opacity * 100)}%</span>
        </label>

        <div style={{ display:"flex", gap:6, flexWrap:"wrap" }}>
          <button onClick={() => onArchive(note.id)} className="md-icon-btn" style={headBtnStyle}>Arquivar</button>
          <button onClick={() => onDelete(note.id)} className="md-icon-btn" style={{ ...headBtnStyle, color: surface.onDark ? "#FCA5A5" : "#B91C1C" }}>
            Deletar
          </button>
        </div>

        {note.tags.length > 0 && (
          <div style={{ display:"flex", gap:4, flexWrap:"wrap" }}>
            {note.tags.map((t) => (
              <span key={t} style={{ fontSize:11, background: surface.chip, color: ink, padding:"2px 7px", borderRadius:999, fontWeight:500 }}>
                #{t}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
