import { THEME_MODE_LABEL, type ThemeMode } from "../theme/theme";
import { useTheme } from "../theme/useTheme";

const MODES: ThemeMode[] = ["light", "dark", "system"];

/** 14px, traço de 1.5 — mesma densidade do ícone de busca do board. */
function ModeIcon({ mode }: { mode: ThemeMode }) {
  const common = {
    width: 14,
    height: 14,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: 1.8,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
  };
  if (mode === "light") {
    return (
      <svg {...common}>
        <circle cx="12" cy="12" r="4" />
        <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M18.4 5.6L17 7M7 17l-1.4 1.4" />
      </svg>
    );
  }
  if (mode === "dark") {
    return (
      <svg {...common}>
        <path d="M20 14.5A8.5 8.5 0 1 1 9.5 4a7 7 0 0 0 10.5 10.5z" />
      </svg>
    );
  }
  // Sistema: um monitor — a decisão vem de fora do app.
  return (
    <svg {...common}>
      <rect x="3" y="4" width="18" height="12" rx="2" />
      <path d="M9 20h6M12 16v4" />
    </svg>
  );
}

/**
 * Claro / escuro / sistema como controle segmentado de três estados.
 *
 * Um botão de alternância de dois estados não conseguiria expressar
 * "automático", que é o padrão — e "automático" é justamente o que a maioria
 * quer, então precisa ser visível, não escondido num submenu.
 */
export function ThemeToggle() {
  const { mode, theme, followsSystem, setMode } = useTheme();

  return (
    <div
      className="md-theme-toggle"
      role="group"
      aria-label={`Tema: ${THEME_MODE_LABEL[mode]}${followsSystem ? ` (sistema está ${theme === "dark" ? "escuro" : "claro"})` : ""}`}
    >
      {MODES.map((m) => (
        <button
          key={m}
          type="button"
          className="md-theme-btn"
          aria-pressed={mode === m}
          onClick={() => setMode(m)}
          title={
            m === "system"
              ? `Seguir o sistema (agora: ${theme === "dark" ? "escuro" : "claro"})`
              : `Tema ${THEME_MODE_LABEL[m].toLowerCase()}`
          }
        >
          <ModeIcon mode={m} />
          <span
            style={{
              position: "absolute",
              width: 1,
              height: 1,
              overflow: "hidden",
              clip: "rect(0 0 0 0)",
              whiteSpace: "nowrap",
            }}
          >
            {THEME_MODE_LABEL[m]}
          </span>
        </button>
      ))}
    </div>
  );
}
