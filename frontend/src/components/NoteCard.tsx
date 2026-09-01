import { useState, useRef } from "react";
import type { Note } from "../types";

const COLORS = ["#FFEB3B", "#FF9800", "#8BC34A", "#03A9F4", "#E91E63", "#9C27B0", "#FFFFFF", "#263238"];

interface Props {
  note: Note;
  onUpdate: (id: string, patch: Partial<Note> & { title?: string; content?: string; color?: string; opacity?: number; position?: [number, number]; size?: [number, number] }) => void;
  onArchive: (id: string) => void;
  onDelete: (id: string) => void;
  onTogglePin: (id: string) => void;
  onToggleAot: (id: string) => void;
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
      // visual feedback via direct style? we emit position continuously
      // debounce final persist on mouseup
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

  return (
    <div
      id={`note-${note.id}`}
      style={{
        position: "absolute",
        left: note.position[0],
        top: note.position[1],
        width: note.size[0],
        height: note.size[1],
        background: note.color,
        opacity: note.opacity,
        borderRadius: 10,
        boxShadow: note.pinned ? "0 6px 20px rgba(0,0,0,0.25)" : "0 2px 10px rgba(0,0,0,0.15)",
        border: note.pinned ? "2px solid #111" : "1px solid rgba(0,0,0,0.1)",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        minWidth: 180,
        minHeight: 140,
        resize: "both",
      }}
      onMouseUp={(e) => {
        // detect resize via manual check: if size changed, persist
        const rect = (e.currentTarget as HTMLDivElement).getBoundingClientRect();
        if (Math.abs(rect.width - note.size[0]) > 2 || Math.abs(rect.height - note.size[1]) > 2) {
          handleResizeEnd(rect.width, rect.height);
        }
      }}
    >
      <div
        onMouseDown={handleMouseDown}
        style={{
          cursor: "grab",
          padding: "6px 8px",
          display: "flex",
          alignItems: "center",
          gap: 6,
          background: "rgba(0,0,0,0.06)",
          userSelect: "none",
          fontWeight: 600,
          fontSize: 13,
        }}
      >
        <span style={{ flex: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
          {note.title} {note.pinned ? "📌" : ""} {note.always_on_top ? "⬆" : ""}
        </span>
        <button onClick={() => onTogglePin(note.id)} title="Fixar" style={iconBtnStyle}>
          {note.pinned ? "Unpin" : "Pin"}
        </button>
        <button onClick={() => onToggleAot(note.id)} title="Always on top" style={iconBtnStyle}>
          {note.always_on_top ? "AOT off" : "AOT on"}
        </button>
      </div>

      <div style={{ padding: 8, flex: 1, display: "flex", flexDirection: "column", gap: 8, overflow: "auto" }}>
        {editing ? (
          <>
            <input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Título"
              maxLength={200}
              style={inputStyle}
            />
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="Conteúdo"
              rows={4}
              style={{ ...inputStyle, resize: "vertical" }}
            />
            <div style={{ display: "flex", gap: 6 }}>
              <button onClick={save} style={primaryBtnStyle}>
                Salvar
              </button>
              <button
                onClick={() => {
                  setTitle(note.title);
                  setContent(note.content);
                  setEditing(false);
                }}
                style={iconBtnStyle}
              >
                Cancelar
              </button>
            </div>
          </>
        ) : (
          <>
            <div style={{ whiteSpace: "pre-wrap", fontSize: 13, lineHeight: 1.4, flex: 1 }}>{note.content || <i style={{ opacity: 0.6 }}>sem conteúdo</i>}</div>
            <button onClick={() => setEditing(true)} style={iconBtnStyle}>
              Editar
            </button>
          </>
        )}

        <div style={{ display: "flex", flexWrap: "wrap", gap: 4, alignItems: "center" }}>
          {COLORS.map((c) => (
            <button
              key={c}
              onClick={() => onUpdate(note.id, { color: c })}
              title={c}
              style={{
                width: 18,
                height: 18,
                borderRadius: "50%",
                background: c,
                border: c === note.color ? "2px solid #111" : "1px solid rgba(0,0,0,0.2)",
                cursor: "pointer",
              }}
            />
          ))}
        </div>

        <label style={{ fontSize: 12, display: "flex", alignItems: "center", gap: 6 }}>
          Opacidade
          <input
            type="range"
            min={0.1}
            max={1}
            step={0.05}
            value={note.opacity}
            onChange={(e) => onUpdate(note.id, { opacity: parseFloat(e.target.value) })}
          />
          <span>{Math.round(note.opacity * 100)}%</span>
        </label>

        <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
          <button onClick={() => onArchive(note.id)} style={iconBtnStyle}>
            Arquivar
          </button>
          <button onClick={() => onDelete(note.id)} style={{ ...iconBtnStyle, color: "#b00020" }}>
            Deletar
          </button>
        </div>

        {note.tags.length > 0 && (
          <div style={{ display: "flex", gap: 4, flexWrap: "wrap" }}>
            {note.tags.map((t) => (
              <span
                key={t}
                style={{
                  fontSize: 11,
                  background: "rgba(0,0,0,0.08)",
                  padding: "2px 6px",
                  borderRadius: 999,
                }}
              >
                #{t}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

const iconBtnStyle: React.CSSProperties = {
  fontSize: 11,
  padding: "4px 6px",
  borderRadius: 6,
  border: "1px solid rgba(0,0,0,0.15)",
  background: "white",
  cursor: "pointer",
};

const primaryBtnStyle: React.CSSProperties = {
  ...iconBtnStyle,
  background: "#111",
  color: "white",
  borderColor: "#111",
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "6px 8px",
  borderRadius: 6,
  border: "1px solid rgba(0,0,0,0.2)",
  fontSize: 13,
  fontFamily: "inherit",
};
