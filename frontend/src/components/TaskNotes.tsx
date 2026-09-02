import { useEffect, useRef, useState } from "react";
import type { TaskNote } from "../types";
import * as api from "../api";

interface Props {
  taskId: string;
  /** Contador vindo do board, para não buscar tudo só para mostrar "3". */
  initialCount?: number;
  onCountChange?: (count: number) => void;
}

/**
 * `02/09 14:07` — exatamente 11 caracteres, sempre.
 *
 * Montado à mão em vez de `toLocaleString`: a calha do log tem largura fixa, e
 * o locale muda o formato (o pt-BR insere uma vírgula, outros invertem dia e
 * mês). Largura previsível é requisito de layout aqui, não preferência.
 */
function formatEntryTime(iso: string): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(d.getDate())}/${pad(d.getMonth() + 1)} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/**
 * Log de atendimento de uma tarefa.
 *
 * As anotações são uma sequência — o que foi feito, em ordem — então a leitura
 * é a de um caderno de atendimento: horário na calha à esquerda, marcador na
 * régua vertical, texto no corpo. Marcar o ponto risca a linha, o que deixa o
 * log servir como checklist sem precisar de subtarefas.
 */
export function TaskNotes({ taskId, initialCount, onCountChange }: Props) {
  const [notes, setNotes] = useState<TaskNote[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingText, setEditingText] = useState("");
  const composeRef = useRef<HTMLTextAreaElement>(null);

  const publish = (next: TaskNote[]) => {
    setNotes(next);
    onCountChange?.(next.length);
  };

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const list = await api.listTaskNotes(taskId);
        if (!cancelled) {
          setNotes(list);
          onCountChange?.(list.length);
        }
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
    // `onCountChange` fica fora das deps de propósito: o board passa uma
    // closure nova a cada render e isso recarregaria o log sem parar.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId]);

  const handleAdd = async () => {
    const content = draft.trim();
    if (!content || saving) return;
    setSaving(true);
    setError(null);
    try {
      const created = await api.addTaskNote(taskId, content);
      publish([...notes, created]);
      setDraft("");
      composeRef.current?.focus();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleToggleDone = async (note: TaskNote) => {
    try {
      const updated = await api.setTaskNoteDone(note.id, !note.done);
      publish(notes.map((n) => (n.id === updated.id ? updated : n)));
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSaveEdit = async () => {
    if (!editingId) return;
    const content = editingText.trim();
    if (!content) {
      setError("A anotação não pode ficar vazia. Apague-a se não precisa mais dela.");
      return;
    }
    try {
      const updated = await api.updateTaskNote(editingId, content);
      publish(notes.map((n) => (n.id === updated.id ? updated : n)));
      setEditingId(null);
      setEditingText("");
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async (note: TaskNote) => {
    try {
      await api.deleteTaskNote(note.id);
      publish(notes.filter((n) => n.id !== note.id));
    } catch (e) {
      setError(String(e));
    }
  };

  // Ctrl/Cmd+Enter salva: quem registra atendimento digita muito e não quer
  // ir ao mouse a cada entrada. Enter puro insere linha, porque uma anotação
  // frequentemente tem mais de uma.
  const composeKeys = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void handleAdd();
    }
  };

  const editKeys = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      void handleSaveEdit();
    }
    if (e.key === "Escape") {
      e.preventDefault();
      setEditingId(null);
      setEditingText("");
    }
  };

  return (
    <div className="md-tasklog">
      <div className="md-eyebrow" style={{ marginBottom: 8 }}>
        Anotações {initialCount !== undefined && !loading ? `· ${notes.length}` : ""}
      </div>

      {error && (
        <div role="alert" className="md-alert" style={{ margin: "0 0 8px" }}>
          {error}
          <button className="md-alert-dismiss" onClick={() => setError(null)}>
            dispensar
          </button>
        </div>
      )}

      {loading ? (
        <div className="md-skeleton" style={{ height: 34, margin: "0 0 8px" }} />
      ) : notes.length === 0 ? (
        <div className="md-quiet" style={{ padding: "10px 12px" }}>
          Nenhuma anotação. Registre o que você fez nessa tarefa — cada entrada
          fica com a data e hora.
        </div>
      ) : (
        <ul className="md-tasklog-list">
          {notes.map((note) => {
            const isEditing = editingId === note.id;
            return (
              <li
                key={note.id}
                className={`md-tasklog-entry ${note.done ? "md-tasklog-entry--done" : ""}`}
              >
                <time className="md-tasklog-time" dateTime={note.created_at}>
                  {formatEntryTime(note.created_at)}
                </time>

                <span className="md-tasklog-mark">
                  <input
                    type="checkbox"
                    checked={note.done}
                    onChange={() => void handleToggleDone(note)}
                    aria-label={
                      note.done
                        ? `Reabrir anotação: ${note.content.slice(0, 40)}`
                        : `Concluir anotação: ${note.content.slice(0, 40)}`
                    }
                  />
                </span>

                <div className="md-tasklog-body">
                  {isEditing ? (
                    <>
                      <textarea
                        className="md-textarea"
                        value={editingText}
                        onChange={(e) => setEditingText(e.target.value)}
                        onKeyDown={editKeys}
                        rows={2}
                        autoFocus
                        aria-label="Editar anotação"
                      />
                      <div className="md-btn-row" style={{ marginTop: 6 }}>
                        <button className="md-btn md-btn--primary" onClick={() => void handleSaveEdit()}>
                          Salvar
                        </button>
                        <button
                          className="md-btn md-btn--ghost"
                          onClick={() => {
                            setEditingId(null);
                            setEditingText("");
                          }}
                        >
                          Cancelar
                        </button>
                      </div>
                    </>
                  ) : (
                    <>
                      <div
                        className="md-tasklog-text"
                        onDoubleClick={() => {
                          setEditingId(note.id);
                          setEditingText(note.content);
                        }}
                        title="Clique duas vezes para editar"
                      >
                        {note.content}
                      </div>
                      <div className="md-tasklog-actions">
                        <button
                          className="md-tasklog-mini"
                          onClick={() => {
                            setEditingId(note.id);
                            setEditingText(note.content);
                          }}
                        >
                          Editar
                        </button>
                        <button
                          className="md-tasklog-mini md-tasklog-mini--danger"
                          onClick={() => void handleDelete(note)}
                        >
                          Apagar
                        </button>
                      </div>
                    </>
                  )}
                </div>
              </li>
            );
          })}
        </ul>
      )}

      <div className="md-tasklog-compose">
        <textarea
          ref={composeRef}
          className="md-textarea"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={composeKeys}
          placeholder="Registrar uma anotação…"
          rows={1}
          aria-label="Nova anotação"
        />
        <button
          className="md-btn md-btn--primary"
          onClick={() => void handleAdd()}
          disabled={!draft.trim() || saving}
          title="Adicionar anotação (Ctrl+Enter)"
        >
          {saving ? "Salvando…" : "Adicionar"}
        </button>
      </div>
    </div>
  );
}
