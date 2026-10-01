import { describe, expect, it } from "vitest";
import { findFreeSpot, NOTE_GAP } from "./placement";
import { clampZoom, fitRects, MAX_ZOOM, MIN_ZOOM, parseViewport, screenToWorld, zoomAt } from "./viewport";

const SIZE = { w: 300, h: 250 };
const VISIBLE = { x: 0, y: 0, w: 1200, h: 800 };

describe("findFreeSpot", () => {
  it("quadro vazio: canto superior esquerdo da área visível", () => {
    expect(findFreeSpot([], SIZE, VISIBLE)).toEqual([NOTE_GAP, NOTE_GAP]);
  });

  it("não sobrepõe a nota existente", () => {
    const first = { x: NOTE_GAP, y: NOTE_GAP, ...SIZE };
    const [x, y] = findFreeSpot([first], SIZE, VISIBLE);
    const clear = x >= first.x + first.w + NOTE_GAP || y >= first.y + first.h + NOTE_GAP;
    expect(clear).toBe(true);
    expect(y).toBe(NOTE_GAP); // ao lado, não embaixo
  });

  it("segue a área visível depois de um pan", () => {
    expect(findFreeSpot([], SIZE, { x: -500, y: 300, w: 800, h: 600 })).toEqual([-500 + NOTE_GAP, 300 + NOTE_GAP]);
  });

  it("linha cheia: desce para a próxima", () => {
    const row = [0, 1, 2, 3].map((i) => ({ x: NOTE_GAP + i * 320, y: NOTE_GAP, ...SIZE }));
    const [, y] = findFreeSpot(row, SIZE, VISIBLE);
    expect(y).toBeGreaterThanOrEqual(NOTE_GAP + SIZE.h + NOTE_GAP);
  });

  it("nunca devolve posição que colide com várias notas", () => {
    const notes = Array.from({ length: 12 }, (_, i) => ({ x: (i % 4) * 330, y: Math.floor(i / 4) * 280, ...SIZE }));
    const [x, y] = findFreeSpot(notes, SIZE, VISIBLE);
    const hit = notes.some((r) => x < r.x + r.w && x + SIZE.w > r.x && y < r.y + r.h && y + SIZE.h > r.y);
    expect(hit).toBe(false);
  });
});

describe("viewport", () => {
  it("limita o zoom", () => {
    expect(clampZoom(10)).toBe(MAX_ZOOM);
    expect(clampZoom(0)).toBe(MIN_ZOOM);
    expect(clampZoom(NaN)).toBe(1);
  });

  it("zoomAt mantém o ponto sob o cursor", () => {
    const vp = { panX: 30, panY: -40, zoom: 1 };
    const before = screenToWorld(vp, 200, 150);
    const after = screenToWorld(zoomAt(vp, 0.5, 200, 150), 200, 150);
    expect(after[0]).toBeCloseTo(before[0]);
    expect(after[1]).toBeCloseTo(before[1]);
  });

  it("fitRects enquadra tudo sem passar de 100%", () => {
    const vp = fitRects([{ x: 0, y: 0, w: 3000, h: 300 }], 1000, 600);
    expect(vp.zoom).toBeLessThan(1);
    expect(fitRects([{ x: 0, y: 0, w: 100, h: 100 }], 1000, 600).zoom).toBe(1);
  });

  it("parseViewport rejeita lixo", () => {
    expect(parseViewport("{oops").zoom).toBe(1);
    expect(parseViewport('{"panX":"a","panY":0,"zoom":1}').panX).toBe(0);
    expect(parseViewport('{"panX":5,"panY":6,"zoom":9}')).toEqual({ panX: 5, panY: 6, zoom: MAX_ZOOM });
  });
});
