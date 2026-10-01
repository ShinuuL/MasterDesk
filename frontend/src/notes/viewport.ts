/**
 * Câmera do quadro de notas (pan + zoom, no estilo Figma).
 *
 * Tela = mundo * zoom + pan. As posições das notas ficam sempre em
 * coordenadas do mundo; só a câmera muda com o zoom.
 */

export interface Viewport {
  panX: number;
  panY: number;
  zoom: number;
}

export const MIN_ZOOM = 0.2;
export const MAX_ZOOM = 2;
export const DEFAULT_VIEWPORT: Viewport = { panX: 0, panY: 0, zoom: 1 };

export function clampZoom(z: number): number {
  if (!Number.isFinite(z)) return 1;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));
}

/** Zoom mantendo fixo o ponto de tela (sx, sy) — o cursor, na roda do mouse. */
export function zoomAt(vp: Viewport, nextZoom: number, sx: number, sy: number): Viewport {
  const zoom = clampZoom(nextZoom);
  const wx = (sx - vp.panX) / vp.zoom;
  const wy = (sy - vp.panY) / vp.zoom;
  return { zoom, panX: sx - wx * zoom, panY: sy - wy * zoom };
}

export function screenToWorld(vp: Viewport, sx: number, sy: number): [number, number] {
  return [(sx - vp.panX) / vp.zoom, (sy - vp.panY) / vp.zoom];
}

/** Câmera que enquadra todos os retângulos numa área de w×h px. */
export function fitRects(
  rects: readonly { x: number; y: number; w: number; h: number }[],
  w: number,
  h: number,
  padding = 40,
): Viewport {
  if (rects.length === 0 || w <= 0 || h <= 0) return DEFAULT_VIEWPORT;
  const minX = Math.min(...rects.map((r) => r.x));
  const minY = Math.min(...rects.map((r) => r.y));
  const maxX = Math.max(...rects.map((r) => r.x + r.w));
  const maxY = Math.max(...rects.map((r) => r.y + r.h));
  const zoom = clampZoom(
    Math.min(1, (w - padding * 2) / (maxX - minX), (h - padding * 2) / (maxY - minY)),
  );
  return {
    zoom,
    panX: (w - (maxX - minX) * zoom) / 2 - minX * zoom,
    panY: (h - (maxY - minY) * zoom) / 2 - minY * zoom,
  };
}

/** Lê a câmera salva; qualquer valor inválido volta ao padrão. */
export function parseViewport(raw: string | null): Viewport {
  if (!raw) return DEFAULT_VIEWPORT;
  try {
    const v = JSON.parse(raw) as Partial<Viewport>;
    if (
      typeof v.panX === "number" && Number.isFinite(v.panX) &&
      typeof v.panY === "number" && Number.isFinite(v.panY) &&
      typeof v.zoom === "number"
    ) {
      return { panX: v.panX, panY: v.panY, zoom: clampZoom(v.zoom) };
    }
  } catch {
    // JSON corrompido: usa o padrão.
  }
  return DEFAULT_VIEWPORT;
}
