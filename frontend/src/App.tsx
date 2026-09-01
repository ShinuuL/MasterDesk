import { useState } from "react";
import { NotesBoard } from "./components/NotesBoard";
import { TasksBoard } from "./components/TasksBoard";

type Tab = "notes" | "tasks";

export default function App() {
  const [tab, setTab] = useState<Tab>("notes");

  return (
    <div style={{ height: "100vh", display: "flex", flexDirection: "column" }}>
      <nav
        style={{
          display: "flex",
          gap: 4,
          padding: "6px 12px",
          background: "#111827",
          color: "white",
        }}
      >
        <button
          onClick={() => setTab("notes")}
          style={{
            padding: "6px 16px",
            borderRadius: 6,
            border: "none",
            background: tab === "notes" ? "#374151" : "transparent",
            color: "white",
            cursor: "pointer",
            fontWeight: 600,
          }}
        >
          Notas
        </button>
        <button
          onClick={() => setTab("tasks")}
          style={{
            padding: "6px 16px",
            borderRadius: 6,
            border: "none",
            background: tab === "tasks" ? "#374151" : "transparent",
            color: "white",
            cursor: "pointer",
            fontWeight: 600,
          }}
        >
          Tarefas
        </button>
      </nav>
      {tab === "notes" ? <NotesBoard /> : <TasksBoard />}
    </div>
  );
}
