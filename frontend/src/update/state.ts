/**
 * Estado do toast de atualização, separado do React e do Tauri para poder ser
 * testado sem nenhum dos dois.
 *
 * O ciclo é deliberadamente curto: quem usa o app não quer um assistente de
 * atualização, quer saber que existe uma versão nova e decidir se para agora ou
 * depois. Por isso não há "verificando" visível — a checagem acontece calada e
 * só aparece quando tem o que dizer.
 */
export type UpdateState =
  /** Nada a mostrar: ainda checando, sem versão nova, ou checagem falhou. */
  | { kind: "idle" }
  /** Versão nova encontrada; esperando o usuário decidir. */
  | { kind: "available"; version: string; notes: string | null }
  /** Baixando. `progress` é 0..1, ou `null` quando o servidor não manda tamanho. */
  | { kind: "downloading"; version: string; progress: number | null }
  /** Baixado e instalado; falta reiniciar. */
  | { kind: "ready"; version: string }
  /** Falhou ao baixar/instalar — o toast mostra o motivo e deixa tentar de novo. */
  | { kind: "error"; version: string; message: string };

/**
 * Acumulador de progresso do download.
 *
 * O `onEvent` do plugin manda `contentLength` uma vez (no `Started`) e depois
 * só o tamanho de cada pedaço; somar é responsabilidade de quem escuta. Quando
 * o servidor não manda `Content-Length` — GitHub manda, mas um proxy corporativo
 * pode remover — `total` fica 0 e o progresso vira `null`, que a barra desenha
 * como indeterminada em vez de fingir uma porcentagem.
 */
export class DownloadProgress {
  private total = 0;
  private downloaded = 0;

  start(contentLength: number | undefined): void {
    this.total = contentLength ?? 0;
    this.downloaded = 0;
  }

  advance(chunkLength: number): void {
    this.downloaded += chunkLength;
  }

  /** Fração 0..1, ou `null` se o tamanho total é desconhecido. */
  get fraction(): number | null {
    if (this.total <= 0) return null;
    return Math.min(1, this.downloaded / this.total);
  }
}

/**
 * Mensagem de erro legível a partir do que o plugin lançou.
 *
 * O updater rejeita com `Error`, com string, e — quando o erro vem da ponte IPC
 * — com objetos sem `message`. Um `String(e)` cru produziria "[object Object]"
 * no lugar onde o usuário precisa entender se é rede ou permissão.
 */
export function updateErrorMessage(e: unknown): string {
  if (e instanceof Error && e.message) return e.message;
  if (typeof e === "string" && e.trim()) return e.trim();
  if (e && typeof e === "object") {
    const msg = (e as { message?: unknown }).message;
    if (typeof msg === "string" && msg.trim()) return msg.trim();
  }
  return "falha desconhecida ao atualizar";
}

/**
 * Corta as notas de versão para caber no toast.
 *
 * O corpo da release no GitHub pode ter páginas de changelog; o toast tem três
 * linhas. Corta em limite de palavra para não terminar no meio de uma.
 */
export function summarizeNotes(notes: string | undefined, max = 180): string | null {
  const clean = (notes ?? "").replace(/\s+/g, " ").trim();
  if (!clean) return null;
  if (clean.length <= max) return clean;
  const cut = clean.slice(0, max);
  const lastSpace = cut.lastIndexOf(" ");
  return `${(lastSpace > max * 0.6 ? cut.slice(0, lastSpace) : cut).trimEnd()}…`;
}
