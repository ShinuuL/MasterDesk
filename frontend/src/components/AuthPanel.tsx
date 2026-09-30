import { useRef, useState } from "react";
import type { AuthPayload } from "../types";
import * as api from "../api";
import appIcon from "../assets/app-icon.png";

type Mode = "login" | "register";

interface AuthPanelProps {
  /** `rememberFailed`: pediu "manter conectado" e o cofre do SO recusou. */
  onAuthenticated: (user: AuthPayload, rememberFailed: boolean) => void;
}

export function AuthPanel({ onAuthenticated }: AuthPanelProps) {
  const [mode, setMode] = useState<Mode>("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [password2, setPassword2] = useState("");
  // Ligado por padrão: o cofre é do usuário do Windows, e quem divide a
  // máquina com outra pessoa desmarca — é o padrão dos apps de desktop.
  const [remember, setRemember] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const firstField = useRef<HTMLInputElement>(null);

  const switchMode = (m: Mode) => {
    setMode(m);
    setError(null);
    setPassword("");
    setPassword2("");
    firstField.current?.focus();
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    const u = username.trim();
    if (!u) {
      setError("Informe um usuário.");
      return;
    }
    if (!password) {
      setError("Informe uma senha.");
      return;
    }
    if (mode === "register" && password !== password2) {
      setError("As senhas não coincidem.");
      return;
    }

    setBusy(true);
    try {
      const res =
        mode === "login"
          ? await api.authLogin({ username: u, password, remember })
          : await api.authRegister({ username: u, password, remember });
      onAuthenticated(res, remember && !res.remembered);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div style={{ height: "100vh", display: "grid", placeItems: "center", background: "var(--canvas)" }}>
      <div style={{ width: 360, background: "var(--surface-plain)", borderRadius: 18, boxShadow: "var(--shadow-lg)", overflow: "hidden" }}>
        <div style={{ padding: "32px 28px 24px", textAlign: "center" }}>
          {/* O ícone real do app, o mesmo da janela e do instalador. */}
          <img
            src={appIcon}
            alt=""
            width={72}
            height={72}
            style={{ borderRadius: 16, boxShadow: "var(--shadow-md)", marginBottom: 14 }}
          />
          <h1 style={{ margin: "0 0 4px", fontSize: 22, letterSpacing: "-.02em", fontWeight: 700 }}>
            {mode === "login" ? "Entrar no MasterNote" : "Criar conta"}
          </h1>
          <p style={{ margin: "0 0 20px", fontSize: 13, color: "var(--text-muted)" }}>
            {mode === "login"
              ? "Acesse sua mesa de trabalho local."
              : "Sua conta fica apenas neste dispositivo — sem nuvem."}
          </p>

          <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: 12, textAlign: "left" }}>
            <div className="md-field">
              <label htmlFor="auth-username" style={{ fontSize: 11, fontWeight: 700, letterSpacing: ".06em", textTransform: "uppercase", color: "var(--text-muted)" }}>
                Usuário
              </label>
              <input
                id="auth-username"
                ref={firstField}
                className="md-input"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="3 a 32 caracteres (letras, números, _)"
                maxLength={32}
                autoComplete="username"
              />
            </div>

            <div className="md-field">
              <label htmlFor="auth-password" style={{ fontSize: 11, fontWeight: 700, letterSpacing: ".06em", textTransform: "uppercase", color: "var(--text-muted)" }}>
                Senha
              </label>
              <input
                id="auth-password"
                className="md-input"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={mode === "register" ? "Mínimo 8 caracteres" : "Sua senha"}
                autoComplete={mode === "register" ? "new-password" : "current-password"}
              />
            </div>

            {mode === "register" && (
              <div className="md-field">
                <label htmlFor="auth-password2" style={{ fontSize: 11, fontWeight: 700, letterSpacing: ".06em", textTransform: "uppercase", color: "var(--text-muted)" }}>
                  Confirmar senha
                </label>
                <input
                  id="auth-password2"
                  className="md-input"
                  type="password"
                  value={password2}
                  onChange={(e) => setPassword2(e.target.value)}
                  placeholder="Repita a senha"
                  autoComplete="new-password"
                />
              </div>
            )}

            <label className="md-toggle" title="Guarda a sessão no Gerenciador de Credenciais do Windows por até 30 dias sem uso. Sair apaga.">
              <input type="checkbox" checked={remember} onChange={(e) => setRemember(e.target.checked)} />
              Manter conectado
            </label>

            {error && (
              <div role="alert" className="md-alert" style={{ margin: 0 }}>
                {error}
              </div>
            )}

            <button type="submit" className="md-primary" disabled={busy} style={{ marginTop: 4, width: "100%" }}>
              {busy ? "Aguarde…" : mode === "login" ? "Entrar" : "Criar conta"}
            </button>
          </form>

          <div style={{ marginTop: 18, fontSize: 13, color: "var(--text-muted)", textAlign: "center" }}>
            {mode === "login" ? (
              <>
                Ainda não tem conta?{" "}
                <button
                  type="button"
                  onClick={() => switchMode("register")}
                  style={{ background: "none", border: "none", padding: 0, color: "var(--text)", fontWeight: 700, textDecoration: "underline", cursor: "pointer" }}
                >
                  Criar conta
                </button>
              </>
            ) : (
              <>
                Já tem conta?{" "}
                <button
                  type="button"
                  onClick={() => switchMode("login")}
                  style={{ background: "none", border: "none", padding: 0, color: "var(--text)", fontWeight: 700, textDecoration: "underline", cursor: "pointer" }}
                >
                  Entrar
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
