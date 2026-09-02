/**
 * Tone-mapping das cores de nota entre os temas.
 *
 * O usuário escolhe uma cor por nota (post-it amarelo, azul, roxo...). No tema
 * claro essa cor é usada como está. No tema escuro, usá-la crua transformaria
 * cada nota numa ilha de luz — desconfortável à noite, que é justamente quando
 * o tema escuro serve para algo.
 *
 * A solução: preservar o **matiz** (é ele que identifica a nota) e recalcular
 * saturação e luminosidade para uma versão profunda da mesma família. Amarelo
 * vira âmbar escuro, azul vira azul-noite; a nota continua reconhecível de
 * relance sem brilhar.
 *
 * O texto nunca é escolhido de uma tabela fixa: é derivado por contraste real
 * (WCAG 2.1) contra o fundo resultante, então cor personalizada via seletor de
 * cor também fica legível.
 */

export interface NoteSurface {
  /** Fundo do corpo da nota. */
  background: string;
  /** Fundo da barra de título — um degrau acima do corpo. */
  headBackground: string;
  /** Cor de texto legível sobre `background`. */
  text: string;
  /** Borda da nota. */
  border: string;
  /** Divisor entre cabeçalho e corpo, e bordas de controles internos. */
  hairline: string;
  /** Fundo de chips/botões dentro da nota. */
  chip: string;
  /** True quando `text` é claro — componentes usam para escolher ícones. */
  onDark: boolean;
}

interface Hsl {
  h: number; // 0..360
  s: number; // 0..1
  l: number; // 0..1
}

const FALLBACK_HEX = "#FFEB3B";

/** Contraste mínimo texto/fundo — WCAG 2.1 AA para texto normal. */
export const MIN_CONTRAST_AA = 4.5;

/** Off-white morno: ecoa o `paper` do tema claro em vez de branco puro. */
const DARK_TEXT_ON_COLOR = "#F4F2EC";
const LIGHT_TEXT_ON_COLOR = "#12141A";

export function parseHex(hex: string): { r: number; g: number; b: number } | null {
  const value = hex.trim().replace(/^#/, "");
  if (!/^[0-9a-fA-F]{3}$|^[0-9a-fA-F]{6}$/.test(value)) return null;
  const full =
    value.length === 3
      ? value
          .split("")
          .map((c) => c + c)
          .join("")
      : value;
  return {
    r: parseInt(full.slice(0, 2), 16),
    g: parseInt(full.slice(2, 4), 16),
    b: parseInt(full.slice(4, 6), 16),
  };
}

function rgbToHsl(r: number, g: number, b: number): Hsl {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  const delta = max - min;

  if (delta === 0) return { h: 0, s: 0, l };

  const s = l > 0.5 ? delta / (2 - max - min) : delta / (max + min);
  let h: number;
  if (max === rn) h = ((gn - bn) / delta) % 6;
  else if (max === gn) h = (bn - rn) / delta + 2;
  else h = (rn - gn) / delta + 4;
  h *= 60;
  if (h < 0) h += 360;
  return { h, s, l };
}

function hslToHex({ h, s, l }: Hsl): string {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const hp = (((h % 360) + 360) % 360) / 60;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  const [r1, g1, b1] =
    hp < 1
      ? [c, x, 0]
      : hp < 2
        ? [x, c, 0]
        : hp < 3
          ? [0, c, x]
          : hp < 4
            ? [0, x, c]
            : hp < 5
              ? [x, 0, c]
              : [c, 0, x];
  const m = l - c / 2;
  const toHex = (v: number) =>
    Math.round(Math.min(1, Math.max(0, v + m)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${toHex(r1)}${toHex(g1)}${toHex(b1)}`;
}

/** Luminância relativa da WCAG 2.1. */
export function relativeLuminance(hex: string): number {
  const rgb = parseHex(hex) ?? parseHex(FALLBACK_HEX)!;
  const channel = (v: number) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : Math.pow((s + 0.055) / 1.055, 2.4);
  };
  return (
    0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
  );
}

/** Razão de contraste WCAG entre duas cores (1..21). */
export function contrastRatio(a: string, b: string): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const lighter = Math.max(la, lb);
  const darker = Math.min(la, lb);
  return (lighter + 0.05) / (darker + 0.05);
}

/**
 * Escolhe entre texto claro e escuro pelo contraste real — não por uma lista
 * de cores conhecidas. Empate vai para o escuro, que é a identidade do app.
 */
export function readableTextOn(background: string): string {
  const onLight = contrastRatio(background, LIGHT_TEXT_ON_COLOR);
  const onDark = contrastRatio(background, DARK_TEXT_ON_COLOR);
  return onLight >= onDark ? LIGHT_TEXT_ON_COLOR : DARK_TEXT_ON_COLOR;
}

/**
 * Garante contraste AA ajustando a **luminosidade** do fundo, nunca o matiz.
 *
 * Existem cores de meio-tom — o rosa #E91E63 do preset é o caso — em que nem
 * texto escuro nem texto claro chegam a 4,5:1. Escolher "o menos pior" deixaria
 * o texto na faixa de 4,2:1, que é exatamente o tipo de quase-legível que passa
 * no olho do desenvolvedor e cansa quem usa o dia inteiro.
 *
 * Então o fundo cede: caminha em passos de 2% para mais claro e para mais
 * escuro, e vence o primeiro que atinge AA — o desvio menor, portanto a menor
 * mudança visível em relação à cor que o usuário escolheu. O matiz e a
 * saturação ficam intactos, então a nota continua "a rosa".
 */
function ensureReadable(hsl: Hsl): { background: string; text: string } {
  const direct = hslToHex(hsl);
  const directText = readableTextOn(direct);
  if (contrastRatio(direct, directText) >= MIN_CONTRAST_AA) {
    return { background: direct, text: directText };
  }

  const STEP = 0.02;
  for (let delta = STEP; delta <= 1; delta += STEP) {
    // Testa clarear e escurecer no mesmo delta; o mais claro vem primeiro
    // porque post-it é um objeto claro — escurecer muda mais a identidade.
    for (const l of [hsl.l + delta, hsl.l - delta]) {
      if (l < 0 || l > 1) continue;
      const candidate = hslToHex({ ...hsl, l });
      const text = readableTextOn(candidate);
      if (contrastRatio(candidate, text) >= MIN_CONTRAST_AA) {
        return { background: candidate, text };
      }
    }
  }

  // Inalcançável na prática: preto e branco puros já passam com folga.
  return { background: direct, text: directText };
}

/**
 * Traduz a cor escolhida pelo usuário para a superfície do tema escuro.
 *
 * - matiz preservado (identidade da nota)
 * - saturação limitada a 0,18–0,42: o suficiente para o matiz ser reconhecível
 *   sem virar neon sobre fundo escuro
 * - luminosidade fixada perto de 16%, um degrau acima do canvas (#101113)
 * - cor acromática (branco, preto, cinza) cai num grafite morno, porque
 *   "branco" no escuro não pode ser branco
 */
function toDarkSurface(hsl: Hsl): { background: string; head: string; border: string } {
  const achromatic = hsl.s < 0.06;
  const s = achromatic ? 0.04 : Math.min(0.42, Math.max(0.18, hsl.s * 0.55));
  const h = achromatic ? 40 : hsl.h; // 40° = mesmo calor do `paper`
  return {
    background: hslToHex({ h, s, l: 0.16 }),
    head: hslToHex({ h, s, l: 0.22 }),
    border: hslToHex({ h, s, l: 0.3 }),
  };
}

/**
 * Superfície completa de uma nota no tema ativo.
 *
 * Cor inválida ou vazia cai no amarelo padrão em vez de quebrar o card — a cor
 * vem do banco e de um `<input type="color">`, então valor inesperado é uma
 * possibilidade real, não hipótese.
 */
export function noteSurface(color: string, isDark: boolean): NoteSurface {
  const rgb = parseHex(color) ?? parseHex(FALLBACK_HEX)!;
  const hsl = rgbToHsl(rgb.r, rgb.g, rgb.b);

  if (isDark) {
    const { background, head, border } = toDarkSurface(hsl);
    return {
      background,
      headBackground: head,
      text: DARK_TEXT_ON_COLOR,
      border,
      hairline: "rgba(255,255,255,.12)",
      chip: "rgba(255,255,255,.10)",
      onDark: true,
    };
  }

  const { background, text } = ensureReadable(hsl);
  const onDark = text === DARK_TEXT_ON_COLOR;
  return {
    background,
    // Cabeçalho é a mesma cor com um véu — mantém o efeito de papel dobrado
    // do tema claro atual em vez de introduzir uma segunda cor.
    headBackground: onDark ? "rgba(255,255,255,.10)" : "rgba(15,17,21,.06)",
    text,
    border: onDark ? "rgba(255,255,255,.16)" : "rgba(15,17,21,.10)",
    hairline: onDark ? "rgba(255,255,255,.14)" : "rgba(15,17,21,.08)",
    chip: onDark ? "rgba(255,255,255,.14)" : "rgba(15,17,21,.07)",
    onDark,
  };
}

/**
 * Amostra do seletor de cores: sempre a cor **de origem**, vívida, nos dois
 * temas.
 *
 * A primeira versão mostrava a cor já mapeada, para a amostra ser fiel ao
 * resultado. Na tela isso falhou: no tema escuro as seis cromáticas viram
 * círculos de 20px a ~16% de luminosidade, e ninguém consegue achar "azul"
 * entre eles — a paleta deixa de funcionar como paleta.
 *
 * A amostra é um **rótulo de matiz** ("amarelo", "azul"), não uma prévia. Quem
 * mostra o resultado é a própria nota, que muda na hora do clique.
 */
export function noteSwatch(color: string): string {
  return parseHex(color) ? color : FALLBACK_HEX;
}
