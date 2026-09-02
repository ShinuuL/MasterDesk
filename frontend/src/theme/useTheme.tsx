import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  applyTheme,
  broadcastMode,
  detectSystemTheme,
  mediaQueryTheme,
  readStoredMode,
  resolveTheme,
  watchModeBroadcast,
  watchSystemTheme,
  writeStoredMode,
  type ResolvedTheme,
  type ThemeMode,
} from "./theme";

interface ThemeContextValue {
  /** Escolha do usuário: claro, escuro ou seguir o sistema. */
  mode: ThemeMode;
  /** Tema efetivamente aplicado. */
  theme: ResolvedTheme;
  /** True quando `mode === "system"` — a UI mostra qual tema o SO pediu. */
  followsSystem: boolean;
  setMode: (mode: ThemeMode) => void;
}

const ThemeContext = createContext<ThemeContextValue | null>(null);

/**
 * Fonte única do tema para toda a janela.
 *
 * Montado tanto na janela principal quanto nas janelas de nota destacadas —
 * cada webview é um documento independente e precisa do seu próprio provider.
 */
export function ThemeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<ThemeMode>(readStoredMode);
  // Começa pela media query (síncrona, já usada no primeiro paint) e é
  // corrigida pela API nativa logo depois, se ela discordar.
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(mediaQueryTheme);

  const theme = resolveTheme(mode, systemTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  useEffect(() => {
    let cancelled = false;
    void detectSystemTheme().then((detected) => {
      if (!cancelled) setSystemTheme(detected);
    });
    const stopSystem = watchSystemTheme(setSystemTheme);
    // Troca feita em outra janela: reflete aqui sem regravar nem reemitir,
    // senão duas janelas ficariam ecoando o evento uma para a outra.
    const stopBroadcast = watchModeBroadcast(setModeState);
    return () => {
      cancelled = true;
      stopSystem();
      stopBroadcast();
    };
  }, []);

  const setMode = useCallback((next: ThemeMode) => {
    setModeState(next);
    writeStoredMode(next);
    void broadcastMode(next);
  }, []);

  const value = useMemo<ThemeContextValue>(
    () => ({ mode, theme, followsSystem: mode === "system", setMode }),
    [mode, theme, setMode],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    throw new Error("useTheme precisa estar dentro de <ThemeProvider>");
  }
  return ctx;
}
