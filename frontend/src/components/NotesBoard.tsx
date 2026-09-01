import { useEffect, useState } from "react";
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

  const refresh = async () => {
    try {
      setLoading(true);
      setError(null);
      const data = showArchived ? await api.listArchivedNotes() : await api.listActiveNotes();
      setNotes(data);
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

  const handleCreate = async () => {
    if (!title.trim()) return;
    try {
      const created = await api.createNote({ title: title.trim(), content });
      setNotes((prev) => [created, ...prev]);
      setTitle("");
      setContent("");
    } catch (e) {
      setError(String(e));
    }
  };

  const handleUpdate = async (id: string, patch: Record<string, unknown>) => {
    try {
      // map camelCase patch to UpdateNotePayload keys
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
        const n = await api.unarchiveNote(id);
        setNotes((prev) => prev.filter((x) => x.id !== id).concat([]).map((x) => (x.id === id ? n : x)).filter((x) => !x.archived || showArchived));
        // simpler: refresh
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
      // aplica no window também (global always-on-top — reflete estado da nota mais recente)
      await api.setWindowAlwaysOnTop(updated.always_on_top);
      setNotes((prev) => prev.map((n) => (n.id === id ? updated : n)));
    } catch (e) {
      setError(String(e));
    }
  };

  const filtered = notes.filter((n) => {
    if (!filter.trim()) return true;
    const q = filter.toLowerCase();
    return n.title.toLowerCase().includes(q) || n.content.toLowerCase().includes(q) || n.tags.some((t) => t.includes(q));
  });

  return (
    <div style={{ fontFamily: "system-ui, sans-serif", height: "100vh", display: "flex", flexDirection: "column" }}>
      <header
        style={{
          display: "flex",
          gap: 8,
          padding: "8px 12px",
          alignItems: "center",
          borderBottom: "1px solid #e5e7eb",
          background: "white",
          position: "sticky",
          top: 0,
          zIndex: 10,
        }}
      >
        <strong>MasterDesk — Notas</strong>
        <input
          placeholder="Buscar..."
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          style={{ flex: "0 1 220px", padding: "6px 8px", borderRadius: 6, border: "1px solid #d1d5db" }}
        />
        <label style={{ display: "flex", alignItems: "center", gap: 4, fontSize: 13 }}>
          <input type="checkbox" checked={showArchived} onChange={(e) => setShowArchived(e.target.checked)} />
          Arquivadas
        </label>
        <span style={{ marginLeft: "auto", fontSize: 12, opacity: 0.6 }}>{filtered.length} notas</span>
      </header>

      <div style={{ display: "flex", gap: 8, padding: 12, alignItems: "flex-end", background: "#f9fafb", borderBottom: "1px solid #e5e7eb" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <label style={{ fontSize: 12, fontWeight: 600 }}>Título</label>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Título da nota"
            maxLength={200}
            style={{ width: 220, padding: "6px 8px", borderRadius: 6, border: "1px solid #d1d5db" }}
          />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 4, flex: 1 }}>
          <label style={{ fontSize: 12, fontWeight: 600 }}>Conteúdo</label>
          <input
            value={content}
            onChange={(e) => setContent(e.target.value)}
            placeholder="Conteúdo opcional"
            style={{ padding: "6px 8px", borderRadius: 6, border: "1px solid #d1d5db" }}
          />
        </div>
        <button
          onClick={handleCreate}
          disabled={!title.trim()}
          style={{
            padding: "8px 14px",
            borderRadius: 8,
            background: title.trim() ? "#111" : "#9ca3af",
            color: "white",
            border: "none",
            cursor: title.trim() ? "pointer" : "not-allowed",
            fontWeight: 600,
          }}
        >
          Nova nota
        </button>
      </div>

      {error && (
        <div style={{ margin: 12, padding: 8, background: "#fef2f2", border: "1px solid #fecaca", borderRadius: 6, color: "#991b1b", fontSize: 13 }}>{error}</div>
      )}

      <div style={{ position: "relative", flex: 1, overflow: "auto", background: "#f3f4f6" }}>
        {loading ? (
          <div style={{ padding: 20, opacity: 0.6 }}>Carregando...</div>
        ) : filtered.length === 0 ? (
          <div style={{ padding: 24, textAlign: "center", opacity: 0.6 }}>
            {showArchived ? "Nenhuma nota arquivada." : "Nenhuma nota. Crie a primeira acima."}
          </div>
        ) : (
          filtered.map((n) => (
            <NoteCard
              key={n.id}
              note={n}
              onUpdate={handleUpdate}
              onArchive={handleArchive}
              onDelete={handleDelete}
              onTogglePin={handleTogglePin}
              onToggleAot={handleToggleAot}
            />
          ))
        )}
      </div>
    </div>
  );
}
