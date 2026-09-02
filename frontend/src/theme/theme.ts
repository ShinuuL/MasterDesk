/**
 * Tema claro / escuro / automático.
 *
 * ## Por que não confiar só em `prefers-color-scheme`
 *
 * A propagação do tema da janela do Tauri para o `prefers-color-scheme` do
 * webview é inconsistente entre plataformas — em especial no Linux, onde é um
 * bug conhecido e aberto (tauri-apps/tauri#9255, tauri-apps/wry#806). Usar só
 * a media query faria o modo "automático" simplesmente não funcionar em parte
 * dos sistemas suportados.
 *
 * Então a fonte da verdade do modo automático é a API nativa
 * (`getCurrentWindow().theme()` + `onThemeChanged`), que consulta o SO pelo
 * lado Rust. `prefers-color-scheme` fica como fallback para quando não há
 * runtime do Tauri — o caso de rodar `npm run dev` no navegador.
 *
 * ## Escolha do usuário: `localStorage`, não SQLite
 *
 * A preferência é per-máquina e precisa ser lida de forma **síncrona** no
 * primeiro paint; um ida-e-volta assíncrono ao banco causaria um flash de tema
 * claro a cada abertura, inclusive nas janelas de nota destacadas. As janelas
 * de nota compartilham a origem, então compartilham o valor gravado — e uma
 * troca em qualquer janela é propagada em tempo real por evento do Tauri.
 */

export type ThemeMode = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

const STORAGE_KEY = "masterdesk.theme-mode";

/** Evento global do Tauri: mantém todas as janelas no mesmo tema. */
const THEME_EVENT = "masterdesk://theme-changed";

export const THEME_MODE_LABEL: Record<ThemeMode, string> = {
  light: "Claro",
  dark: "Escuro",
  system: "Sistema",
};

function isThemeMode(value: unknown): value is ThemeMode {
  return value === "light" || value === "dark" || value === "system";
}

/**
 * Modo salvo. `system` é o padrão: seguir o SO é o comportamento que o usuário
 * já configurou em outro lugar, então é a escolha menos surpreendente.
 */
export function readStoredMode(): ThemeMode {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    return isThemeMode(raw) ? raw : "system";
  } catch {
    // Modo privado / storage bloqueado — não é motivo para falhar.
    return "system";
  }
}

export function writeStoredMode(mode: ThemeMode): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // Preferência não persiste, mas a sessão atual continua correta.
  }
}

/** Leitura síncrona da media query, para o primeiro paint. */
export function mediaQueryTheme(): ResolvedTheme {
  try {
    return window.matchMedia?.("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light";
  } catch {
    return "light";
  }
}

/** Tema do SO pela API nativa, com a media query como fallback. */
export async function detectSystemTheme(): Promise<ResolvedTheme> {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const theme = await getCurrentWindow().theme();
    if (theme === "light" || theme === "dark") return theme;
  } catch {
    // Sem runtime do Tauri (dev no navegador) ou API indisponível.
  }
  return mediaQueryTheme();
}

export function resolveTheme(mode: ThemeMode, system: ResolvedTheme): ResolvedTheme {
  return mode === "system" ? system : mode;
}

/**
 * Aplica o tema no documento. `data-theme` no `<html>` é o que o CSS observa;
 * `color-scheme` faz o SO desenhar scrollbars, seletores de data e o
 * `<input type="color">` na variante certa.
 */
export function applyTheme(resolved: ResolvedTheme): void {
  const root = document.documentElement;
  root.setAttribute("data-theme", resolved);
  root.style.colorScheme = resolved;
}

/**
 * Aplica o tema o quanto antes, antes do React montar, para não haver flash.
 * Chamado de `main.tsx`. Usa só APIs síncronas de propósito.
 */
export function applyInitialTheme(): ResolvedTheme {
  const resolved = resolveTheme(readStoredMode(), mediaQueryTheme());
  applyTheme(resolved);
  return resolved;
}

/**
 * Observa mudanças de tema do SO. Registra os dois canais (evento nativo e
 * media query) porque cada um cobre uma plataforma onde o outro pode falhar.
 * Retorna a função de limpeza.
 */
export function watchSystemTheme(
  onChange: (theme: ResolvedTheme) => void,
): () => void {
  const cleanups: Array<() => void> = [];

  let media: MediaQueryList | null = null;
  const onMediaChange = (e: MediaQueryListEvent) =>
    onChange(e.matches ? "dark" : "light");
  try {
    media = window.matchMedia("(prefers-color-scheme: dark)");
    media.addEventListener("change", onMediaChange);
    cleanups.push(() => media?.removeEventListener("change", onMediaChange));
  } catch {
    // Sem matchMedia: resta o canal nativo.
  }

  let disposed = false;
  void (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const unlisten = await getCurrentWindow().onThemeChanged(({ payload }) => {
        if (payload === "light" || payload === "dark") onChange(payload);
      });
      if (disposed) unlisten();
      else cleanups.push(unlisten);
    } catch {
      // Sem runtime do Tauri.
    }
  })();

  return () => {
    disposed = true;
    cleanups.forEach((fn) => fn());
  };
}

/** Avisa as outras janelas (notas destacadas) que o modo mudou. */
export async function broadcastMode(mode: ThemeMode): Promise<void> {
  try {
    const { emit } = await import("@tauri-apps/api/event");
    await emit(THEME_EVENT, mode);
  } catch {
    // Janela única / sem Tauri: nada a propagar.
  }
}

/** Ouve trocas de modo feitas em outra janela. Retorna a limpeza. */
export function watchModeBroadcast(
  onMode: (mode: ThemeMode) => void,
): () => void {
  let disposed = false;
  let unlisten: (() => void) | null = null;

  void (async () => {
    try {
      const { listen } = await import("@tauri-apps/api/event");
      const stop = await listen<ThemeMode>(THEME_EVENT, ({ payload }) => {
        if (isThemeMode(payload)) onMode(payload);
      });
      if (disposed) stop();
      else unlisten = stop;
    } catch {
      // Sem Tauri.
    }
  })();

  return () => {
    disposed = true;
    unlisten?.();
  };
}
