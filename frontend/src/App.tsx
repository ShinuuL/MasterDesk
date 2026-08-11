// Fase 1 — Foundation: sem componentes de nota reais ainda. Isso é
// deliberado: o item pendente do ADR-002 (protótipo React vs Svelte +
// mockups) precisa ser validado com o DEV antes de construir UI real de
// domínio (ver docs/ROADMAP.md, Fase 1 / Dev 2).

export default function App() {
  return (
    <main style={{ fontFamily: "sans-serif", padding: "1rem" }}>
      <h1>MasterDesk</h1>
      <p>Foundation scaffold — protótipo React (comparar com protótipo Svelte).</p>
    </main>
  );
}
