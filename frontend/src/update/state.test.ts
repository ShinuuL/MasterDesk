import { describe, expect, it } from "vitest";
import { DownloadProgress, summarizeNotes, updateErrorMessage } from "./state";

describe("DownloadProgress", () => {
  it("soma os pedaços em fração do total", () => {
    const p = new DownloadProgress();
    p.start(1000);
    p.advance(250);
    expect(p.fraction).toBe(0.25);
    p.advance(250);
    expect(p.fraction).toBe(0.5);
  });

  it("sem Content-Length, não inventa porcentagem", () => {
    const p = new DownloadProgress();
    p.start(undefined);
    p.advance(500);
    expect(p.fraction).toBeNull();
  });

  it("não passa de 100% se a soma dos pedaços exceder o total anunciado", () => {
    const p = new DownloadProgress();
    p.start(100);
    p.advance(150);
    expect(p.fraction).toBe(1);
  });

  it("zera ao começar um novo download depois de um que falhou", () => {
    const p = new DownloadProgress();
    p.start(100);
    p.advance(100);
    p.start(200);
    expect(p.fraction).toBe(0);
  });
});

describe("updateErrorMessage", () => {
  it("usa a mensagem do Error", () => {
    expect(updateErrorMessage(new Error("rede indisponível"))).toBe("rede indisponível");
  });

  it("aceita erro em string", () => {
    expect(updateErrorMessage("  assinatura inválida ")).toBe("assinatura inválida");
  });

  it("lê `message` de objeto solto vindo da ponte IPC", () => {
    expect(updateErrorMessage({ message: "forbidden" })).toBe("forbidden");
  });

  it("nunca devolve [object Object]", () => {
    expect(updateErrorMessage({})).toBe("falha desconhecida ao atualizar");
    expect(updateErrorMessage(null)).toBe("falha desconhecida ao atualizar");
  });
});

describe("summarizeNotes", () => {
  it("devolve null quando não há notas", () => {
    expect(summarizeNotes(undefined)).toBeNull();
    expect(summarizeNotes("   \n ")).toBeNull();
  });

  it("colapsa espaços e quebras de linha do corpo da release", () => {
    expect(summarizeNotes("Corrige\n\n  o filtro")).toBe("Corrige o filtro");
  });

  it("corta em limite de palavra e marca a elisão", () => {
    const out = summarizeNotes("palavra ".repeat(40), 30);
    expect(out).not.toBeNull();
    expect(out!.endsWith("…")).toBe(true);
    expect(out!.length).toBeLessThanOrEqual(31);
    expect(out).not.toMatch(/pala…$/);
  });

  it("deixa passar inteiro o que já cabe", () => {
    expect(summarizeNotes("Ajuste pequeno", 180)).toBe("Ajuste pequeno");
  });
});
