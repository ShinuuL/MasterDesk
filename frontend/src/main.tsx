import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import { applyInitialTheme } from "./theme/theme";
import { ThemeProvider } from "./theme/useTheme";

// Antes de o React montar: evita um frame de tema claro em quem usa escuro.
// Só APIs síncronas aqui — a correção pela API nativa do Tauri vem depois,
// dentro do ThemeProvider.
applyInitialTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ThemeProvider>
      <App />
    </ThemeProvider>
  </React.StrictMode>,
);
