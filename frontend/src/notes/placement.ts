/**
 * Onde uma nota nova nasce no quadro.
 *
 * O domínio cria toda nota em (100, 100); sem isto as notas se empilhavam
 * umas sobre as outras. A busca é feita aqui (UI) porque depende do que o
 * usuário está vendo — a área visível do canvas — e não é regra de negócio.
 */

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Espaço livre mínimo entre notas. */
export const NOTE_GAP = 16;
/** Passo da varredura: casa com a grade de pontos do canvas (22px). */
const SCAN_STEP = 22;
/** Limite de linhas varridas além da área visível antes de desistir. */
const MAX_EXTRA_ROWS = 60;

function overlaps(a: Rect, b: Rect, gap: number): boolean {
  return (
    a.x < b.x + b.w + gap &&
    a.x + a.w + gap > b.x &&
    a.y < b.y + b.h + gap &&
    a.y + a.h + gap > b.y
  );
}

/**
 * Primeiro ponto livre, em coordenadas do mundo, varrendo linha a linha a
 * partir do canto superior esquerdo da área visível. Se a área visível estiver
 * cheia, continua descendo (dentro da mesma faixa de largura); se ainda assim
 * não couber, posiciona à direita de todas as notas.
 */
export function findFreeSpot(
  existing: readonly Rect[],
  size: { w: number; h: number },
  visible: Rect,
  gap: number = NOTE_GAP,
): [number, number] {
  const startX = visible.x + gap;
  const startY = visible.y + gap;
  const maxX = Math.max(startX, visible.x + visible.w - size.w - gap);
  const maxY = startY + Math.max(0, visible.h - size.h - gap) + MAX_EXTRA_ROWS * SCAN_STEP;

  for (let y = startY; y <= maxY; y += SCAN_STEP) {
    for (let x = startX; x <= maxX; x += SCAN_STEP) {
      const candidate = { x, y, w: size.w, h: size.h };
      if (!existing.some((r) => overlaps(candidate, r, gap))) {
        return [Math.round(x), Math.round(y)];
      }
    }
  }

  const right = existing.reduce((m, r) => Math.max(m, r.x + r.w), startX);
  return [Math.round(right + gap), Math.round(startY)];
}
