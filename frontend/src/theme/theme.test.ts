import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  THEME_MODE_LABEL,
  mediaQueryTheme,
  readStoredMode,
  resolveTheme,
  writeStoredMode,
} from "./theme";

const STORAGE_KEY = "masterdesk.theme-mode";

/** Preserva o que já foi stubado, para os helpers poderem se compor. */
function currentWindow(): Record<string, unknown> {
  const w = (globalThis as Record<string, unknown>).window;
  return typeof w === "object" && w !== null ? (w as Record<string, unknown>) : {};
}

/** `localStorage` de mentira — os testes rodam em Node, sem DOM. */
function installStorage(store: Record<string, string> = {}, throwOnAccess = false) {
  const fake = {
    getItem: (k: string) => {
      if (throwOnAccess) throw new Error("bloqueado");
      return store[k] ?? null;
    },
    setItem: (k: string, v: string) => {
      if (throwOnAccess) throw new Error("bloqueado");
      store[k] = v;
    },
    removeItem: (k: string) => {
      delete store[k];
    },
  };
  vi.stubGlobal("window", { ...currentWindow(), localStorage: fake });
  return store;
}

function installMatchMedia(prefersDark: boolean | "throws") {
  vi.stubGlobal("window", {
    ...currentWindow(),
    matchMedia: (query: string) => {
      if (prefersDark === "throws") throw new Error("sem matchMedia");
      return { matches: prefersDark && query.includes("dark"), media: query };
    },
  });
}

beforeEach(() => {
  vi.unstubAllGlobals();
});

describe("resolveTheme", () => {
  it("uma escolha explícita ignora o sistema", () => {
    expect(resolveTheme("light", "dark")).toBe("light");
    expect(resolveTheme("dark", "light")).toBe("dark");
  });

  it("o modo automático segue o sistema", () => {
    expect(resolveTheme("system", "dark")).toBe("dark");
    expect(resolveTheme("system", "light")).toBe("light");
  });
});

describe("readStoredMode", () => {
  it("o padrão é seguir o sistema", () => {
    installStorage({});
    expect(readStoredMode()).toBe("system");
  });

  it("lê os três modos válidos", () => {
    for (const mode of ["light", "dark", "system"] as const) {
      installStorage({ [STORAGE_KEY]: mode });
      expect(readStoredMode()).toBe(mode);
    }
  });

  it("valor corrompido cai no padrão em vez de aplicar lixo", () => {
    for (const junk of ["", "DARK", "azul", "null", '{"a":1}']) {
      installStorage({ [STORAGE_KEY]: junk });
      expect(readStoredMode(), junk).toBe("system");
    }
  });

  it("storage bloqueado (janela privada) não lança", () => {
    installStorage({}, true);
    expect(() => readStoredMode()).not.toThrow();
    expect(readStoredMode()).toBe("system");
    expect(() => writeStoredMode("dark")).not.toThrow();
  });
});

describe("writeStoredMode", () => {
  it("persiste o modo escolhido", () => {
    const store = installStorage({});
    writeStoredMode("dark");
    expect(store[STORAGE_KEY]).toBe("dark");
    writeStoredMode("system");
    expect(store[STORAGE_KEY]).toBe("system");
  });
});

describe("mediaQueryTheme", () => {
  it("reflete a preferência do sistema", () => {
    installMatchMedia(true);
    expect(mediaQueryTheme()).toBe("dark");
    installMatchMedia(false);
    expect(mediaQueryTheme()).toBe("light");
  });

  it("sem matchMedia assume claro em vez de quebrar", () => {
    installMatchMedia("throws");
    expect(mediaQueryTheme()).toBe("light");
  });
});

describe("rótulos", () => {
  it("os três modos têm rótulo em português", () => {
    expect(THEME_MODE_LABEL.light).toBe("Claro");
    expect(THEME_MODE_LABEL.dark).toBe("Escuro");
    expect(THEME_MODE_LABEL.system).toBe("Sistema");
  });
});
