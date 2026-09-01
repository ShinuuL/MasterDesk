import { useEffect, useState } from "react";
import type { Task, Priority } from "../types";
import * as api from "../api";

const PRESET_THRESHOLDS: { label: string; minutes: number }[] = [
  { label: "5m", minutes: 5 },
  { label: "10m", minutes: 10 },
  { label: "15m", minutes: 15 },
  { label: "30m", minutes: 30 },
  { label: "1h", minutes: 60 },
  { label: "2h", minutes: 120 },
];

const PRIORITIES: Priority[] = ["Low", "Medium", "High", "Urgent"];

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
  return d.toLocaleString("pt-BR", { dateStyle:"short", timeStyle:"short" } as never);
}

export function TasksBoard() {
  const [pending, setPending] = useState<Task[]>([]);
  const [completed, setCompleted] = useState<Task[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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
        ? "#DC2626"
        : t.priority === "High"
        ? "#EA580C"
        : t.priority === "Medium"
        ? "#3B5BFF"
        : "#6B7280";
    const toneClass = overdue ? "md-task--overdue" : dueSoon ? "md-task--soon" : "";

    return (
      <div
        key={t.id}
        className={`md-task ${toneClass}`}
        style={{ borderLeftColor: priorityColor }}
      >
        <div className="md-task-head">
          <span className="md-task-title">{t.title}</span>
          <span className="md-badge" style={{ background: priorityColor }}>
            {t.priority}
          </span>
          {overdue && <span className="md-due md-due--overdue">• atrasada</span>}
          {dueSoon && <span className="md-due md-due--soon">• vence em breve</span>}
        </div>
        {t.description && <div className="md-task-desc">{t.description}</div>}
        <div className="md-task-meta">
          <span>Deadline: <strong style={{color:"var(--ink)", fontWeight:600}}>{formatDeadline(t.deadline)}</strong></span>
          {t.reminder_thresholds && t.reminder_thresholds.length > 0 && (
            <span>
              Lembretes:{" "}
              {t.reminder_thresholds
                .map((r) => {
                  const mins = thresholdMinutes(r);
                  return mins >= 60 ? `${mins / 60}h` : `${mins}m`;
                })
                .join(" · ")}
            </span>
          )}
        </div>
        <div className="md-btn-row">
          {!t.completed ? (
            <>
              <button onClick={() => handleComplete(t.id)} className="md-btn md-btn--primary">
                Concluir
              </button>
              <button onClick={() => handleSnooze(t.id)} className="md-btn">
                Snooze 15m
              </button>
            </>
          ) : (
            <button onClick={() => handleReopen(t.id)} className="md-btn">
              Reabrir
            </button>
          )}
          <button onClick={() => handleDelete(t.id)} className="md-btn md-btn--danger">
            Deletar
          </button>
        </div>
      </div>
    );
  };

  const isEmptyAll = !loading && pending.length===0 && completed.length===0;

  return (
    <div style={{ fontFamily: "inherit", height:"100%", display:"flex", flexDirection:"column", minHeight:0 }}>
      <header className="md-board-header">
        <strong style={{ fontSize:14, letterSpacing:"-.02em" }}>Tarefas</strong>
        <span className="md-count">
          {pending.length} pendentes · {completed.length} concluídas
        </span>
      </header>

      <div className="md-create-bar" style={{ gap:12 }}>
        <div className="md-field" style={{ flex:"0 0 200px" }}>
          <label htmlFor="task-title">Título</label>
          <input id="task-title" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Título da tarefa" maxLength={200} className="md-input" />
        </div>
        <div className="md-field" style={{ flex:"1 1 160px" }}>
          <label htmlFor="task-desc">Descrição</label>
          <input id="task-desc" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Descrição opcional" className="md-input" style={{ width:"100%" }} />
        </div>
        <div className="md-field">
          <label htmlFor="task-prio">Prioridade</label>
          <select id="task-prio" value={priority} onChange={(e) => setPriority(e.target.value as Priority)} className="md-select">
            {PRIORITIES.map((p) => <option key={p} value={p}>{p}</option>)}
          </select>
        </div>
        <div className="md-field">
          <label htmlFor="task-deadline">Deadline</label>
          <input id="task-deadline" type="datetime-local" value={deadlineLocal} onChange={(e) => setDeadlineLocal(e.target.value)} className="md-input" />
        </div>
      </div>

      <div style={{ padding:"8px 14px", background:"var(--canvas)", borderBottom:"1px solid var(--line)", display:"flex", gap:16, alignItems:"center", flexWrap:"wrap" }}>
        <span style={{ fontSize:11, fontWeight:700, letterSpacing:".06em", textTransform:"uppercase", color:"var(--muted)" }}>Lembretes antes do deadline:</span>
        <div style={{ display:"flex", gap:6, alignItems:"center", flexWrap:"wrap" }}>
          {PRESET_THRESHOLDS.map((p) => (
            <label key={p.minutes} className="md-btn" style={{ padding:"5px 10px", cursor:"pointer", borderColor: thresholds.has(p.minutes) ? "var(--ink)" : undefined, background: thresholds.has(p.minutes) ? "var(--ink)" : "#fff", color: thresholds.has(p.minutes) ? "#fff" : "var(--ink)" }}>
              <input type="checkbox" checked={thresholds.has(p.minutes)} onChange={() => toggleThreshold(p.minutes)} style={{ display:"none" }} />
              {p.label}
            </label>
          ))}
          <label style={{ fontSize:12, display:"flex", alignItems:"center", gap:6 }}>
            Custom:
            <input type="number" min={1} value={customMinutes} onChange={(e) => setCustomMinutes(e.target.value)} placeholder="min" className="md-input" style={{ width:72, padding:"6px 8px", minHeight:32 }} />
          </label>
        </div>
        <button onClick={handleCreate} disabled={!title.trim()} className="md-primary md-primary-accent" style={{ marginLeft:"auto" }}>
          Nova tarefa
        </button>
      </div>

      {error && <div role="alert" className="md-alert">{error} <button onClick={()=>setError(null)} style={{ marginLeft:8, background:"transparent", border:"none", textDecoration:"underline", cursor:"pointer", color:"inherit", fontSize:12 }}>dispensar</button></div>}

      <div className="scroll-hidden" style={{ flex:1, minHeight:0, padding:14, background:"var(--canvas)", overflow: isEmptyAll ? "hidden" : undefined, display: isEmptyAll ? "flex" : "block", flexDirection: isEmptyAll ? "column" as const : undefined }}>
        {loading ? (
          <>
            <div className="md-skeleton" />
            <div className="md-skeleton" style={{ width:"92%" }} />
          </>
        ) : isEmptyAll ? (
          <div className="md-empty" role="status">
            <div className="md-empty-illus" aria-hidden>
              <svg width="26" height="26" viewBox="0 0 24 24" fill="none" aria-hidden style={{ position:"relative", zIndex:1 }}>
                <rect x="5" y="5" width="14" height="14" rx="3" fill="white" stroke="#0F1115" strokeWidth="1.4"/>
                <path d="M8 10h8M8 13h5" stroke="#0F1115" strokeWidth="1.3" strokeLinecap="round"/>
                <circle cx="17" cy="7" r="3" fill="#FFEB3B" stroke="#0F1115" strokeWidth="1.2"/>
                <path d="M15.6 7.2 16.6 8.2 18.4 6.2" stroke="#0F1115" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round"/>
              </svg>
            </div>
            <h3>Nenhuma tarefa ainda</h3>
            <p>Crie sua primeira tarefa acima. Defina prioridade, deadline e lembretes — atrasadas sobem com borda vermelha.</p>
            <div style={{ fontSize:12, color:"var(--muted)", marginTop:2 }}>Dica: use 1h / 30m para lembretes antes do prazo.</div>
          </div>
        ) : (
          <>
            <h3 style={{ fontSize:12, fontWeight:700, letterSpacing:".06em", textTransform:"uppercase", color:"var(--muted)", margin:"2px 0 10px" }}>Pendentes</h3>
            {pending.length === 0 ? (
              <div style={{ background:"#fff", border:"1px dashed var(--line-strong)", borderRadius:12, padding:"14px", fontSize:13, color:"var(--muted)", marginBottom:12 }}>Nenhuma tarefa pendente — bom trabalho!</div>
            ) : (
              pending.map(renderTask)
            )}
            <h3 style={{ fontSize:12, fontWeight:700, letterSpacing:".06em", textTransform:"uppercase", color:"var(--muted)", margin:"18px 0 10px" }}>
              Concluídas {completed.length>0 && <span style={{ fontWeight:500, textTransform:"none", letterSpacing:0 }}>· {completed.length}</span>}
            </h3>
            {completed.length === 0 ? (
              <div style={{ fontSize:13, color:"var(--muted)", padding:"6px 0" }}>Nenhuma tarefa concluída ainda.</div>
            ) : (
              completed.map(renderTask)
            )}
          </>
        )}
      </div>
    </div>
  );
}
