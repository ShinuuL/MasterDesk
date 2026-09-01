import { useEffect, useState } from "react";
import type { Task, Priority } from "../types";
import * as api from "../api";

// Presets de thresholds em minutos
const PRESET_THRESHOLDS: { label: string; minutes: number }[] = [
  { label: "5m", minutes: 5 },
  { label: "10m", minutes: 10 },
  { label: "15m", minutes: 15 },
  { label: "30m", minutes: 30 },
  { label: "1h", minutes: 60 },
  { label: "2h", minutes: 120 },
];

const PRIORITIES: Priority[] = ["Low", "Medium", "High", "Urgent"];

// Helper para extrair minutos de um ReminderThreshold (formato serde enum)
function thresholdMinutes(t: unknown): number {
  if (t && typeof t === "object") {
    const obj = t as Record<string, unknown>;
    if ("Minutes" in obj && typeof obj.Minutes === "number") return obj.Minutes;
    if ("Hours" in obj && typeof obj.Hours === "number") return (obj.Hours as number) * 60;
    if ("Custom" in obj) {
      const c = obj.Custom as Record<string, unknown>;
      if (c && typeof c.minutes_before === "number") return c.minutes_before;
    }
  }
  return 0;
}

function formatDeadline(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleString();
}

export function TasksBoard() {
  const [pending, setPending] = useState<Task[]>([]);
  const [completed, setCompleted] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Form state
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [priority, setPriority] = useState<Priority>("Medium");
  const [deadlineLocal, setDeadlineLocal] = useState("");
  const [thresholds, setThresholds] = useState<Set<number>>(new Set());
  const [customMinutes, setCustomMinutes] = useState("");

  const refresh = async () => {
    try {
      setLoading(true);
      setError(null);
      const [p, c] = await Promise.all([api.listPendingTasks(), api.listCompletedTasks()]);
      setPending(p);
      setCompleted(c);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const toggleThreshold = (minutes: number) => {
    setThresholds((prev) => {
      const next = new Set(prev);
      if (next.has(minutes)) next.delete(minutes);
      else next.add(minutes);
      return next;
    });
  };

  const collectThresholdMinutes = (): number[] => {
    const mins = new Set<number>(thresholds);
    const custom = parseInt(customMinutes, 10);
    if (!isNaN(custom) && custom > 0) mins.add(custom);
    return Array.from(mins);
  };

  const handleCreate = async () => {
    if (!title.trim()) return;
    try {
      const deadline =
        deadlineLocal && !isNaN(new Date(deadlineLocal).getTime())
          ? new Date(deadlineLocal).toISOString()
          : undefined;
      const thresholdsArr = collectThresholdMinutes();
      await api.createTask({
        title: title.trim(),
        description: description.trim() || undefined,
        priority,
        deadline,
        reminder_thresholds: thresholdsArr.length > 0 ? thresholdsArr : undefined,
      });
      setTitle("");
      setDescription("");
      setDeadlineLocal("");
      setThresholds(new Set());
      setCustomMinutes("");
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleComplete = async (id: string) => {
    try {
      await api.completeTask(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleReopen = async (id: string) => {
    try {
      await api.reopenTask(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDelete = async (id: string) => {
    if (!confirm("Deletar esta tarefa?")) return;
    try {
      await api.deleteTask(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSnooze = async (id: string) => {
    try {
      await api.snoozeTask(id);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const renderTask = (t: Task) => {
    const overdue = !t.completed && t.deadline !== null && new Date(t.deadline) <= new Date();
    const dueSoon =
      !t.completed &&
      t.deadline !== null &&
      new Date(t.deadline) <= new Date(Date.now() + 30 * 60 * 1000) &&
      new Date(t.deadline) > new Date();
    const priorityColor =
      t.priority === "Urgent"
        ? "#dc2626"
        : t.priority === "High"
        ? "#ea580c"
        : t.priority === "Medium"
        ? "#2563eb"
        : "#6b7280";
    const borderColor = overdue ? "#dc2626" : dueSoon ? "#f59e0b" : "#e5e7eb";

    return (
      <div
        key={t.id}
        style={{
          border: `1px solid ${borderColor}`,
          borderLeft: `4px solid ${priorityColor}`,
          borderRadius: 8,
          padding: "10px 12px",
          marginBottom: 8,
          background: overdue ? "#fef2f2" : dueSoon ? "#fffbeb" : "white",
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <strong>{t.title}</strong>
          <span
            style={{
              fontSize: 11,
              padding: "1px 6px",
              borderRadius: 10,
              background: priorityColor,
              color: "white",
              fontWeight: 600,
            }}
          >
            {t.priority}
          </span>
          {overdue && (
            <span style={{ fontSize: 11, color: "#991b1b", fontWeight: 700 }}>ATRASADA</span>
          )}
          {dueSoon && (
            <span style={{ fontSize: 11, color: "#92400e", fontWeight: 700 }}>VENCE EM BREVE</span>
          )}
        </div>
        {t.description && (
          <div style={{ fontSize: 13, color: "#4b5563", marginTop: 4 }}>{t.description}</div>
        )}
        <div style={{ fontSize: 12, color: "#6b7280", marginTop: 6 }}>
          Deadline: {formatDeadline(t.deadline)}
          {t.reminder_thresholds && t.reminder_thresholds.length > 0 && (
            <span style={{ marginLeft: 8 }}>
              Lembretes:{" "}
              {t.reminder_thresholds
                .map((r) => {
                  const mins = thresholdMinutes(r);
                  return mins >= 60 ? `${mins / 60}h` : `${mins}m`;
                })
                .join(", ")}
            </span>
          )}
        </div>
        <div style={{ marginTop: 8, display: "flex", gap: 6 }}>
          {!t.completed ? (
            <>
              <button onClick={() => handleComplete(t.id)} style={btnStyle("#16a34a")}>
                Concluir
              </button>
              <button onClick={() => handleSnooze(t.id)} style={btnStyle("#6b7280")}>
                Snooze 15m
              </button>
            </>
          ) : (
            <button onClick={() => handleReopen(t.id)} style={btnStyle("#2563eb")}>
              Reabrir
            </button>
          )}
          <button onClick={() => handleDelete(t.id)} style={btnStyle("#dc2626")}>
            Deletar
          </button>
        </div>
      </div>
    );
  };

  return (
    <div
      style={{
        fontFamily: "system-ui, sans-serif",
        height: "100vh",
        display: "flex",
        flexDirection: "column",
      }}
    >
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
        <strong>MasterDesk — Tarefas</strong>
        <span style={{ marginLeft: "auto", fontSize: 12, opacity: 0.6 }}>
          {pending.length} pendentes · {completed.length} concluídas
        </span>
      </header>

      {/* Form novo task */}
      <div
        style={{
          display: "flex",
          gap: 8,
          padding: 12,
          alignItems: "flex-end",
          background: "#f9fafb",
          borderBottom: "1px solid #e5e7eb",
          flexWrap: "wrap",
        }}
      >
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <label style={{ fontSize: 12, fontWeight: 600 }}>Título</label>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Título da tarefa"
            maxLength={200}
            style={inputStyle}
          />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <label style={{ fontSize: 12, fontWeight: 600 }}>Descrição</label>
          <input
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder="Descrição opcional"
            style={{ ...inputStyle, width: 200 }}
          />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <label style={{ fontSize: 12, fontWeight: 600 }}>Prioridade</label>
          <select
            value={priority}
            onChange={(e) => setPriority(e.target.value as Priority)}
            style={inputStyle}
          >
            {PRIORITIES.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
          <label style={{ fontSize: 12, fontWeight: 600 }}>Deadline</label>
          <input
            type="datetime-local"
            value={deadlineLocal}
            onChange={(e) => setDeadlineLocal(e.target.value)}
            style={inputStyle}
          />
        </div>
      </div>

      {/* Thresholds config */}
      <div
        style={{
          padding: "8px 12px",
          background: "#f3f4f6",
          borderBottom: "1px solid #e5e7eb",
          display: "flex",
          gap: 16,
          alignItems: "center",
          flexWrap: "wrap",
        }}
      >
        <span style={{ fontSize: 12, fontWeight: 600 }}>Lembretes antes do deadline:</span>
        <div style={{ display: "flex", gap: 8, alignItems: "center", flexWrap: "wrap" }}>
          {PRESET_THRESHOLDS.map((p) => (
            <label key={p.minutes} style={{ fontSize: 12, display: "flex", alignItems: "center", gap: 3 }}>
              <input
                type="checkbox"
                checked={thresholds.has(p.minutes)}
                onChange={() => toggleThreshold(p.minutes)}
              />
              {p.label}
            </label>
          ))}
          <label style={{ fontSize: 12, display: "flex", alignItems: "center", gap: 3 }}>
            Custom:
            <input
              type="number"
              min={1}
              value={customMinutes}
              onChange={(e) => setCustomMinutes(e.target.value)}
              placeholder="min"
              style={{ width: 60, padding: "4px 6px", borderRadius: 6, border: "1px solid #d1d5db" }}
            />
          </label>
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
            marginLeft: "auto",
          }}
        >
          Nova tarefa
        </button>
      </div>

      {error && (
        <div
          style={{
            margin: 12,
            padding: 8,
            background: "#fef2f2",
            border: "1px solid #fecaca",
            borderRadius: 6,
            color: "#991b1b",
            fontSize: 13,
          }}
        >
          {error}
        </div>
      )}

      <div style={{ flex: 1, overflow: "auto", padding: 12, background: "#f3f4f6" }}>
        {loading ? (
          <div style={{ padding: 20, opacity: 0.6 }}>Carregando...</div>
        ) : (
          <>
            <h3 style={{ fontSize: 14, margin: "4px 0 8px" }}>Pendentes</h3>
            {pending.length === 0 ? (
              <div style={{ opacity: 0.6, fontSize: 13, padding: "8px 0" }}>
                Nenhuma tarefa pendente.
              </div>
            ) : (
              pending.map(renderTask)
            )}
            <h3 style={{ fontSize: 14, margin: "20px 0 8px" }}>Concluídas</h3>
            {completed.length === 0 ? (
              <div style={{ opacity: 0.6, fontSize: 13, padding: "8px 0" }}>
                Nenhuma tarefa concluída.
              </div>
            ) : (
              completed.map(renderTask)
            )}
          </>
        )}
      </div>
    </div>
  );
}

const inputStyle: React.CSSProperties = {
  padding: "6px 8px",
  borderRadius: 6,
  border: "1px solid #d1d5db",
};

const btnStyle = (color: string): React.CSSProperties => ({
  padding: "5px 10px",
  borderRadius: 6,
  border: "none",
  background: color,
  color: "white",
  fontSize: 12,
  cursor: "pointer",
  fontWeight: 600,
});
