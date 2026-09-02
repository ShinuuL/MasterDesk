import { describe, expect, it } from "vitest";
import {
  MIN_CONTRAST_AA,
  contrastRatio,
  noteSurface,
  noteSwatch,
  parseHex,
  readableTextOn,
  relativeLuminance,
} from "./noteSurface";

/** Os presets oferecidos no NoteCard. */
const PRESETS = [
  "#FFEB3B",
  "#FF9800",
  "#8BC34A",
  "#03A9F4",
  "#E91E63",
  "#9C27B0",
  "#FFFFFF",
  "#263238",
] as const;

/** Casos que o seletor de cor personalizado pode produzir. */
const CUSTOM = [
  "#000000",
  "#FFFFFF",
  "#7F7F7F",
  "#888888",
  "#FF0000",
  "#00FF00",
  "#0000FF",
  "#123456",
  "#996699",
  "#C0392B",
  "#fff",
  "#abc",
] as const;

describe("parseHex", () => {
  it("aceita 3 e 6 dígitos, com e sem #", () => {
    expect(parseHex("#fff")).toEqual({ r: 255, g: 255, b: 255 });
    expect(parseHex("FFFFFF")).toEqual({ r: 255, g: 255, b: 255 });
    expect(parseHex("#123456")).toEqual({ r: 0x12, g: 0x34, b: 0x56 });
    expect(parseHex("  #abc  ")).toEqual({ r: 0xaa, g: 0xbb, b: 0xcc });
  });

  it("rejeita o que não é hex", () => {
    for (const bad of ["", "#", "red", "#12", "#12345", "#1234567", "#gggggg", "rgb(1,2,3)"]) {
      expect(parseHex(bad), bad).toBeNull();
    }
  });
});

describe("contraste (WCAG 2.1)", () => {
  it("luminância dos extremos", () => {
    expect(relativeLuminance("#000000")).toBeCloseTo(0, 5);
    expect(relativeLuminance("#FFFFFF")).toBeCloseTo(1, 5);
  });

  it("preto sobre branco é 21:1 e a razão é simétrica", () => {
    expect(contrastRatio("#000000", "#FFFFFF")).toBeCloseTo(21, 1);
    expect(contrastRatio("#FFFFFF", "#000000")).toBeCloseTo(21, 1);
  });

  it("mesma cor é 1:1", () => {
    expect(contrastRatio("#8BC34A", "#8BC34A")).toBeCloseTo(1, 5);
  });

  it("escolhe texto escuro em fundo claro e claro em fundo escuro", () => {
    expect(readableTextOn("#FFFFFF")).toBe(readableTextOn("#FFEB3B"));
    expect(readableTextOn("#000000")).not.toBe(readableTextOn("#FFFFFF"));
  });
});

describe("noteSurface — invariante de legibilidade", () => {
  // Esta é a razão de o módulo existir. `textColorFor`, que ele substituiu,
  // era uma tabela com as 8 cores predefinidas: qualquer cor do seletor
  // personalizado recebia texto escuro e podia sair ilegível.
  const ALL = [...PRESETS, ...CUSTOM];

  for (const color of ALL) {
    for (const isDark of [false, true]) {
      const label = isDark ? "escuro" : "claro";
      it(`${color} no tema ${label} atinge AA`, () => {
        const surface = noteSurface(color, isDark);
        expect(contrastRatio(surface.background, surface.text)).toBeGreaterThanOrEqual(
          MIN_CONTRAST_AA,
        );
      });
    }
  }

  it("o rosa do preset é clareado só o necessário para atingir AA", () => {
    // #E91E63 cru não passa AA com nenhuma cor de texto (fica em ~4,23:1).
    const raw = "#E91E63";
    const worstRaw = Math.max(
      contrastRatio(raw, "#12141A"),
      contrastRatio(raw, "#F4F2EC"),
    );
    expect(worstRaw).toBeLessThan(MIN_CONTRAST_AA);

    const surface = noteSurface(raw, false);
    expect(contrastRatio(surface.background, surface.text)).toBeGreaterThanOrEqual(
      MIN_CONTRAST_AA,
    );
    // E o desvio é pequeno: continua sendo "a nota rosa".
    const before = parseHex(raw)!;
    const after = parseHex(surface.background)!;
    const distance =
      Math.abs(before.r - after.r) + Math.abs(before.g - after.g) + Math.abs(before.b - after.b);
    expect(distance).toBeLessThan(60);
  });
});

describe("noteSurface — tema escuro", () => {
  it("escurece a cor em vez de usá-la crua", () => {
    const light = noteSurface("#FFEB3B", false);
    const dark = noteSurface("#FFEB3B", true);
    expect(dark.background).not.toBe(light.background);
    expect(relativeLuminance(dark.background)).toBeLessThan(
      relativeLuminance(light.background),
    );
    expect(relativeLuminance(dark.background)).toBeLessThan(0.1);
  });

  it("preserva o matiz para a nota continuar reconhecível", () => {
    // Amarelo → âmbar: o canal vermelho continua dominando o azul.
    const amber = parseHex(noteSurface("#FFEB3B", true).background)!;
    expect(amber.r).toBeGreaterThan(amber.b);

    // Azul → azul-noite: o canal azul continua dominando o vermelho.
    const navy = parseHex(noteSurface("#03A9F4", true).background)!;
    expect(navy.b).toBeGreaterThan(navy.r);
  });

  it("cores distintas continuam distintas no escuro", () => {
    const backgrounds = PRESETS.map((c) => noteSurface(c, true).background);
    // Branco e grafite colapsam no mesmo grafite morno de propósito (ambos são
    // acromáticos); as seis cromáticas precisam sobrar distintas.
    const chromatic = new Set(backgrounds.slice(0, 6));
    expect(chromatic.size).toBe(6);
  });

  it("acromáticas viram um grafite morno, não preto nem branco", () => {
    const white = noteSurface("#FFFFFF", true);
    const black = noteSurface("#000000", true);
    const gray = noteSurface("#7F7F7F", true);
    expect(white.background).toBe(black.background);
    expect(white.background).toBe(gray.background);
    expect(white.background).not.toBe("#000000");
    expect(white.background).not.toBe("#ffffff");
  });

  it("usa texto claro no escuro", () => {
    for (const color of PRESETS) {
      expect(noteSurface(color, true).onDark).toBe(true);
    }
  });

  it("cabeçalho e borda ficam mais claros que o corpo", () => {
    const s = noteSurface("#03A9F4", true);
    expect(relativeLuminance(s.headBackground)).toBeGreaterThan(
      relativeLuminance(s.background),
    );
    expect(relativeLuminance(s.border)).toBeGreaterThan(
      relativeLuminance(s.headBackground),
    );
  });
});

describe("noteSurface — entrada inválida", () => {
  it("cai no amarelo padrão sem lançar", () => {
    for (const bad of ["", "  ", "não-é-cor", "#gg0011", "undefined"]) {
      const light = noteSurface(bad, false);
      const dark = noteSurface(bad, true);
      expect(light.background.toLowerCase()).toBe("#ffeb3b");
      expect(dark.background).toBe(noteSurface("#FFEB3B", true).background);
      expect(contrastRatio(dark.background, dark.text)).toBeGreaterThanOrEqual(
        MIN_CONTRAST_AA,
      );
    }
  });
});

describe("noteSwatch", () => {
  // Regra: a amostra é rótulo de matiz, não prévia do resultado. Mostrar a cor
  // mapeada deixava a paleta ilegível no tema escuro (seis círculos de 20px
  // quase idênticos) — verificado na tela, não só no teste.
  it("mostra sempre a cor de origem, para a paleta continuar escaneável", () => {
    expect(noteSwatch("#03A9F4")).toBe("#03A9F4");
    expect(noteSwatch("#FFEB3B")).toBe("#FFEB3B");
  });

  it("as amostras são mutuamente distinguíveis", () => {
    const swatches = new Set(PRESETS.map((c) => noteSwatch(c)));
    expect(swatches.size).toBe(PRESETS.length);
  });

  it("cor inválida não vaza para o estilo inline", () => {
    expect(noteSwatch("lixo")).toBe("#FFEB3B");
  });
});
