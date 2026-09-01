import { useState, useRef } from "react";
import type { Note } from "../types";

const COLORS = ["#FFEB3B", "#FF9800", "#8BC34A", "#03A9F4", "#E91E63", "#9C27B0", "#FFFFFF", "#263238"] as const;
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
}

function textColorFor(bg: string): string {
  // usa grafite/escuro contrastante; amarelo e branco precisam ink escuro, grafite precisa claro
  if (bg === "#263238") return "#FFFFFF";
  if (bg === "#9C27B0" || bg === "#E91E63") return "#FFFFFF";
  return "#0F1115";
}

export function NoteCard({ note, onUpdate, onArchive, onDelete, onTogglePin, onToggleAot }: Props) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(note.title);
  const [content, setContent] = useState(note.content);
  const dragRef = useRef<{ x: number; y: number; orig: [number, number] } | null>(null);

  const save = () => {
    setEditing(false);
    if (title !== note.title || content !== note.content) {
      onUpdate(note.id, { title, content });
    }
  };

  const handleMouseDown = (e: React.MouseEvent) => {
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

  const ink = textColorFor(note.color);
  const isDark = ink === "#FFFFFF";

  return (
    <div
      id={`note-${note.id}`}
      className={`md-note ${note.pinned ? "md-note--pinned" : ""}`}
      style={{
        left: note.position[0],
        top: note.position[1],
        width: note.size[0],
        height: note.size[1],
        background: note.color,
        opacity: note.opacity,
        color: ink,
      }}
      onMouseUp={(e) => {
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
        className="md-note-head"
        style={{ color: ink, borderColor: isDark ? "rgba(255,255,255,.14)" : "rgba(15,17,21,.08)" }}
      >
        <span className="md-grip" aria-hidden>⋮⋮</span>
        <span className="md-note-title" title={note.title}>
          {note.title} {note.pinned ? "📌" : ""} {note.always_on_top ? "⬆" : ""}
        </span>
        <button onClick={() => onTogglePin(note.id)} title={note.pinned ? "Desafixar" : "Fixar"} aria-pressed={note.pinned} className="md-icon-btn" style={{ color: ink, borderColor: isDark ? "rgba(255,255,255,.22)" : undefined, background: isDark ? "rgba(255,255,255,.12)" : undefined }}>
          {note.pinned ? "Unpin" : "Pin"}
        </button>
        <button onClick={() => onToggleAot(note.id)} title="Always on top" aria-pressed={note.always_on_top} className="md-icon-btn" style={{ color: ink, borderColor: isDark ? "rgba(255,255,255,.22)" : undefined, background: isDark ? "rgba(255,255,255,.12)" : undefined }}>
          {note.always_on_top ? "AOT off" : "AOT on"}
        </button>
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
              <button onClick={save} className="md-icon-btn" style={{ background: ink, color: note.color, borderColor: ink, fontWeight:700 }}>
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
            <div style={{ whiteSpace:"pre-wrap", fontSize:13, lineHeight:1.5, flex:1, wordBreak:"break-word" }}>
              {note.content || <i style={{ opacity:.6 }}>sem conteúdo</i>}
            </div>
            <button onClick={() => setEditing(true)} className="md-icon-btn" style={{ alignSelf:"flex-start" }}>
              Editar
            </button>
          </>
        )}

        <div style={{ display:"flex", flexWrap:"wrap", gap:5, alignItems:"center" }} role="group" aria-label="Escolher cor">
          {COLORS.map((c) => {
            const selected = c === note.color;
            return (
              <button
                key={c}
                onClick={() => onUpdate(note.id, { color: c })}
                title={COLOR_LABEL[c] ?? c}
                aria-label={`Cor ${COLOR_LABEL[c] ?? c}`}
                aria-pressed={selected}
                style={{
                  width:20, height:20, borderRadius:"50%",
                  background:c,
                  border: selected ? "2.5px solid #0F1115" : "1px solid rgba(15,17,21,.18)",
                  cursor:"pointer",
                  boxShadow: selected ? "0 0 0 2px rgba(15,17,21,.08)" : "0 1px 2px rgba(0,0,0,.08)",
                  position:"relative",
                  display:"grid", placeItems:"center",
                }}
              >
                {selected && <span aria-hidden style={{ fontSize:10, lineHeight:1, color: c==="#263238"||c==="#9C27B0" ? "#fff" : "#0F1115", fontWeight:800 }}>✓</span>}
              </button>
            );
          })}
        </div>

        <label style={{ fontSize:11, fontWeight:600, letterSpacing:".04em", textTransform:"uppercase", opacity:.75, display:"flex", alignItems:"center", gap:7, flexWrap:"wrap" }}>
          Opacidade
          <input type="range" min={0.1} max={1} step={0.05} value={note.opacity} onChange={(e) => onUpdate(note.id, { opacity: parseFloat(e.target.value) })} aria-label="Opacidade" style={{ flex:1 }} />
          <span style={{ fontVariantNumeric:"tabular-nums", fontSize:12, opacity:1, textTransform:"none", letterSpacing:0 }}>{Math.round(note.opacity * 100)}%</span>
        </label>

        <div style={{ display:"flex", gap:6, flexWrap:"wrap" }}>
          <button onClick={() => onArchive(note.id)} className="md-icon-btn">Arquivar</button>
          <button onClick={() => onDelete(note.id)} className="md-icon-btn" style={{ color:"#B91C1C", borderColor:"rgba(185,28,28,.22)" }}>
            Deletar
          </button>
        </div>

        {note.tags.length > 0 && (
          <div style={{ display:"flex", gap:4, flexWrap:"wrap" }}>
            {note.tags.map((t) => (
              <span key={t} style={{ fontSize:11, background: isDark ? "rgba(255,255,255,.14)" : "rgba(15,17,21,.08)", color: ink, padding:"2px 7px", borderRadius:999, fontWeight:500 }}>
                #{t}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
