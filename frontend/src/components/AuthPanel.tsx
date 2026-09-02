import { useRef, useState } from "react";
import type { AuthPayload } from "../types";
import * as api from "../api";

type Mode = "login" | "register";

interface AuthPanelProps {
  onAuthenticated: (user: AuthPayload) => void;
}

export function AuthPanel({ onAuthenticated }: AuthPanelProps) {
  const [mode, setMode] = useState<Mode>("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [password2, setPassword2] = useState("");
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
          ? await api.authLogin({ username: u, password })
          : await api.authRegister({ username: u, password });
      onAuthenticated(res);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div style={{ height: "100vh", display: "grid", placeItems: "center", background: "var(--canvas)" }}>
      <div style={{ width: 360, background: "var(--surface-plain)", border: "1px solid var(--line-strong)", borderRadius: "var(--radius-lg)", boxShadow: "var(--shadow-lg)", overflow: "hidden" }}>
        <div style={{ padding: "22px 24px", display: "flex", alignItems: "center", gap: 12, borderBottom: "1px solid var(--line)", background: "var(--chrome)", color: "var(--chrome-text)" }}>
          <div style={{ width: 40, height: 40, borderRadius: 10, background: "var(--accent)", color: "var(--text)", display: "grid", placeItems: "center", fontWeight: 800, fontSize: 16 }} aria-hidden>
            MD
          </div>
          <div>
            <div style={{ fontWeight: 700, letterSpacing: "-.02em", fontSize: 15 }}>MasterDesk</div>
            <div style={{ fontSize: 11, opacity: 0.7, textTransform: "uppercase", letterSpacing: ".06em" }}>notas • tarefas • foco</div>
          </div>
        </div>

        <div style={{ padding: "22px 24px" }}>
          <h1 style={{ margin: "0 0 4px", fontSize: 18, letterSpacing: "-.02em", fontWeight: 750 }}>
            {mode === "login" ? "Entrar" : "Criar conta"}
          </h1>
          <p style={{ margin: "0 0 18px", fontSize: 13, color: "var(--text-muted)" }}>
            {mode === "login"
              ? "Acesse sua mesa de trabalho local."
              : "Sua conta fica apenas neste dispositivo — sem nuvem."}
          </p>

          <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: 12 }}>
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
