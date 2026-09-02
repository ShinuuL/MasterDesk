import { useEffect, useRef, useState } from "react";
import type { MastersysStatus, SyncReport } from "../types";
import * as api from "../api";

interface Props {
  onClose: () => void;
  /** Chamado após qualquer operação que possa ter mexido nas tarefas. */
  onTasksChanged: () => void;
}

const PRESET_REMINDERS: { label: string; minutes: number }[] = [
  { label: "15m", minutes: 15 },
  { label: "30m", minutes: 30 },
  { label: "1h", minutes: 60 },
  { label: "2h", minutes: 120 },
];

const REMINDERS_KEY = "masterdesk.mastersys.import-reminders";

/**
 * Lembretes para itens importados. Vive no `localStorage` porque é preferência
 * de máquina e a UI precisa dela antes de qualquer chamada — o backend recebe
 * o valor como parâmetro da sincronização em vez de ter um default embutido.
 */
function readReminders(): number[] {
  try {
    const raw = window.localStorage.getItem(REMINDERS_KEY);
    if (!raw) return [30];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [30];
    return parsed.filter((n): n is number => typeof n === "number" && n > 0);
  } catch {
    return [30];
  }
}

function writeReminders(minutes: number[]): void {
  try {
    window.localStorage.setItem(REMINDERS_KEY, JSON.stringify(minutes));
  } catch {
    // Sem persistência: a sessão atual continua funcionando.
  }
}

function describeReport(report: SyncReport): { label: string; value: number }[] {
  return [
    { label: "importados", value: report.imported },
    { label: "atualizados", value: report.updated },
    { label: "removidos", value: report.removed },
    { label: "mantidos com anotações", value: report.kept_with_notes },
  ].filter((row) => row.value > 0);
}

export function MastersysPanel({ onClose, onTasksChanged }: Props) {
  const [status, setStatus] = useState<MastersysStatus | null>(null);
  const [endpoint, setEndpoint] = useState("");
  const [identifier, setIdentifier] = useState("");
  const [password, setPassword] = useState("");
  const [reminders, setReminders] = useState<number[]>(readReminders);

  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<null | "endpoint" | "connect" | "sync" | "disconnect">(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [report, setReport] = useState<SyncReport | null>(null);
  const panelRef = useRef<HTMLElement>(null);

  // `role="dialog" aria-modal="true"` cria a expectativa de fechar no Escape e
  // de o foco entrar no painel. Sem isso, quem navega por teclado abre o painel
  // e continua tabulando pelo quadro atrás dele.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    panelRef.current?.focus();
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const refresh = async () => {
    const next = await api.mastersysStatus();
    setStatus(next);
    setEndpoint(next.endpoint ?? "");
    return next;
  };

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const next = await api.mastersysStatus();
        if (!cancelled) {
          setStatus(next);
          setEndpoint(next.endpoint ?? "");
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
  }, []);

  const run = async (
    kind: NonNullable<typeof busy>,
    action: () => Promise<string | null>,
  ) => {
    setBusy(kind);
    setError(null);
    setNotice(null);
    try {
      const message = await action();
      if (message) setNotice(message);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleSaveEndpoint = () =>
    run("endpoint", async () => {
      await api.mastersysSetEndpoint(endpoint);
      await refresh();
      return "Endereço salvo.";
    });

  const handleConnect = () =>
    run("connect", async () => {
      // Salva o endereço junto: quem está conectando quase sempre acabou de
      // digitá-lo, e pedir "salve depois conecte" é um passo desnecessário.
      if (endpoint.trim() && endpoint.trim() !== status?.endpoint) {
        await api.mastersysSetEndpoint(endpoint);
      }
      const identity = await api.mastersysConnect(identifier, password);
      setPassword("");
      setIdentifier("");
      await refresh();
      return `Conectado como ${identity.display_name}.`;
    });

  const handleSync = () =>
    run("sync", async () => {
      const result = await api.mastersysSync(reminders);
      setReport(result);
      onTasksChanged();
      const changes = describeReport(result);
      return changes.length === 0
        ? "Nada mudou desde a última sincronização."
        : "Sincronizado.";
    });

  const handleDisconnect = () =>
    run("disconnect", async () => {
      const result = await api.mastersysDisconnect();
      setReport(result);
      onTasksChanged();
      await refresh();
      return "Desconectado.";
    });

  const toggleReminder = (minutes: number) => {
    const next = reminders.includes(minutes)
      ? reminders.filter((m) => m !== minutes)
      : [...reminders, minutes].sort((a, b) => a - b);
    setReminders(next);
    writeReminders(next);
  };

  const connected = status?.connected ?? false;
  const reportRows = report ? describeReport(report) : [];

  return (
    <div
      className="md-panel-overlay"
      onClick={onClose}
      role="presentation"
    >
      <aside
        ref={panelRef}
        className="md-panel"
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label="Mastersys Suporte"
        tabIndex={-1}
      >
        <header className="md-panel-head">
          <h2>Mastersys Suporte</h2>
          <button className="md-panel-close" onClick={onClose} aria-label="Fechar painel">
            ✕
          </button>
        </header>

        <div className="md-panel-body scroll-hidden">
          <p className="md-panel-note">
            As tarefas e chamados atribuídos a você no Mastersys aparecem no seu
            quadro. A sincronização é de mão única: editar aqui não altera o
            Mastersys, e a próxima sincronização sobrescreve título, prazo e
            status. Suas anotações são só suas e nunca são apagadas.
          </p>

          {loading ? (
            <div className="md-skeleton" style={{ margin: 0 }} />
          ) : (
            <>
              <div className={`md-conn ${connected ? "md-conn--on" : "md-conn--off"}`}>
                <span className="md-conn-dot" aria-hidden />
                <span className="md-conn-text">
                  <span className="md-conn-title">
                    {connected ? "Conectado" : "Não conectado"}
                  </span>
                  <span className="md-conn-sub">
                    {status?.identity
                      ? `${status.identity.display_name}${status.identity.email ? ` · ${status.identity.email}` : ""}`
                      : status?.endpoint
                        ? "Entre com seu usuário do Mastersys."
                        : "Informe o endereço do Mastersys para começar."}
                  </span>
                </span>
              </div>

              {error && (
                <div role="alert" className="md-alert" style={{ margin: 0 }}>
                  {error}
                </div>
              )}
              {notice && !error && (
                <div role="status" className="md-alert md-alert--ok" style={{ margin: 0 }}>
                  {notice}
                </div>
              )}

              {reportRows.length > 0 && (
                <ul className="md-sync-report" aria-label="Resultado da sincronização">
                  {reportRows.map((row) => (
                    <li key={row.label}>
                      <strong>{row.value}</strong>
                      {row.label}
                    </li>
                  ))}
                </ul>
              )}

              <section className="md-panel-section">
                <div className="md-field">
                  <label htmlFor="ms-endpoint">Endereço</label>
                  <input
                    id="ms-endpoint"
                    className="md-input"
                    value={endpoint}
                    onChange={(e) => setEndpoint(e.target.value)}
                    placeholder="https://suporte.suaempresa.com"
                    autoComplete="off"
                    spellCheck={false}
                  />
                </div>
                <button
                  className="md-btn"
                  onClick={() => void handleSaveEndpoint()}
                  disabled={!endpoint.trim() || busy !== null}
                  style={{ alignSelf: "flex-start" }}
                >
                  {busy === "endpoint" ? "Salvando…" : "Salvar endereço"}
                </button>
              </section>

              {!connected && (
                <section className="md-panel-section">
                  <div className="md-eyebrow">Entrar</div>
                  <div className="md-field">
                    <label htmlFor="ms-user">Usuário ou e-mail</label>
                    <input
                      id="ms-user"
                      className="md-input"
                      value={identifier}
                      onChange={(e) => setIdentifier(e.target.value)}
                      autoComplete="username"
                      spellCheck={false}
                    />
                  </div>
                  <div className="md-field">
                    <label htmlFor="ms-pass">Senha</label>
                    <input
                      id="ms-pass"
                      className="md-input"
                      type="password"
                      value={password}
                      onChange={(e) => setPassword(e.target.value)}
                      autoComplete="current-password"
                    />
                  </div>
                  <p className="md-panel-note">
                    A senha é usada uma vez e descartada. O MasterDesk guarda
                    apenas o token de sessão, no cofre de credenciais do
                    Windows.
                  </p>
                  <button
                    className="md-primary"
                    onClick={() => void handleConnect()}
                    disabled={
                      !identifier.trim() || !password || !endpoint.trim() || busy !== null
                    }
                    style={{ alignSelf: "flex-start" }}
                  >
                    {busy === "connect" ? "Entrando…" : "Entrar"}
                  </button>
                </section>
              )}

              <section className="md-panel-section">
                <div className="md-eyebrow">Lembretes de itens importados</div>
                <p className="md-panel-note">
                  Aplicado quando um item entra no quadro pela primeira vez. Se
                  você ajustar o lembrete de uma tarefa depois, a sincronização
                  respeita a sua escolha.
                </p>
                <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                  {PRESET_REMINDERS.map((preset) => (
                    <button
                      key={preset.minutes}
                      type="button"
                      className="md-chip"
                      aria-pressed={reminders.includes(preset.minutes)}
                      onClick={() => toggleReminder(preset.minutes)}
                    >
                      {preset.label}
                    </button>
                  ))}
                </div>
              </section>
            </>
          )}
        </div>

        <footer className="md-panel-foot">
          {connected && (
            <button
              className="md-btn md-btn--danger"
              onClick={() => void handleDisconnect()}
              disabled={busy !== null}
            >
              {busy === "disconnect" ? "Desconectando…" : "Desconectar"}
            </button>
          )}
          <button
            className="md-primary"
            onClick={() => void handleSync()}
            disabled={!connected || busy !== null}
            title={connected ? undefined : "Entre no Mastersys para sincronizar"}
          >
            {busy === "sync" ? "Sincronizando…" : "Sincronizar agora"}
          </button>
        </footer>
      </aside>
    </div>
  );
}
