import { useState } from "react";
import { NotesBoard } from "./components/NotesBoard";
import { TasksBoard } from "./components/TasksBoard";

type Tab = "notes" | "tasks";

export default function App() {
  const [tab, setTab] = useState<Tab>("notes");

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

        <div className="md-nav-right" aria-hidden>
          <span style={{ width:7, height:7, borderRadius:"50%", background:"var(--accent)", display:"inline-block", boxShadow:"0 0 0 4px rgba(255,235,59,.18)" }} />
          <span style={{ fontWeight:600, letterSpacing:".02em" }}>1080×620</span>
          <span style={{ opacity:.45 }}>•</span>
          <span>desktop-first</span>
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
