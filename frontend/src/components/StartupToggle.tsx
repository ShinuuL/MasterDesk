import { useEffect, useState } from "react";
import * as api from "../api";

/** Nome que a pessoa reconhece; fora do Windows, genérico. */
const SYSTEM_NAME = navigator.userAgent.includes("Windows") ? "Windows" : "sistema";

/**
 * "Iniciar com o Windows" como botão de dois estados, no mesmo formato do
 * seletor de tema ao lado.
 *
 * O estado mostrado é sempre o que o SO respondeu (`autostartSetEnabled`
 * devolve a leitura pós-operação), não o que foi clicado — se o Windows
 * recusar, o botão não mente.
 */
export function StartupToggle() {
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api
      .autostartIsEnabled()
      .then((v) => { if (!cancelled) setEnabled(v); })
      .catch((e) => { if (!cancelled) setError(String(e)); });
    return () => { cancelled = true; };
  }, []);

  const toggle = async () => {
    if (enabled === null || busy) return;
    setBusy(true);
    setError(null);
    try {
      setEnabled(await api.autostartSetEnabled(!enabled));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const label = `Iniciar com o ${SYSTEM_NAME}`;
  const title = error
    ? `${label}: ${error}`
    : enabled === null
      ? label
      : `${label}: ${enabled ? "ligado" : "desligado"}`;

  return (
    <div className="md-theme-toggle" role="group" aria-label={label}>
      <button
        type="button"
        className="md-theme-btn"
        aria-pressed={enabled === true}
        aria-busy={busy}
        disabled={enabled === null || busy}
        onClick={toggle}
        title={title}
        data-error={error ? "true" : undefined}
      >
        {/* Símbolo de energia: liga junto com o computador. */}
        <svg
          width={14}
          height={14}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.8}
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden
        >
          <path d="M12 3v9" />
          <path d="M6.3 6.8a8 8 0 1 0 11.4 0" />
        </svg>
        <span className="md-sr-only">{title}</span>
      </button>
    </div>
  );
}
