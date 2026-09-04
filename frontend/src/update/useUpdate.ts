import { useCallback, useEffect, useRef, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  DownloadProgress,
  summarizeNotes,
  updateErrorMessage,
  type UpdateState,
} from "./state";

/** Intervalo entre checagens enquanto o app fica aberto. */
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000; // 6 h

/** Espera antes da primeira checagem, para não disputar rede com o login e a sincronização inicial. */
const FIRST_CHECK_DELAY_MS = 20_000;

/**
 * Checa atualizações e expõe o estado do toast.
 *
 * Erros de checagem são engolidos de propósito: uma máquina sem internet, atrás
 * de proxy ou com o GitHub bloqueado não deve ver um toast vermelho a cada seis
 * horas por algo que não é problema dela e que ela não pode resolver. Erro só
 * aparece depois que o usuário clicou "Atualizar" — aí ele pediu, e o silêncio
 * é que seria confuso.
 */
export function useUpdate() {
  const [state, setState] = useState<UpdateState>({ kind: "idle" });

  /**
   * O objeto `Update` do plugin, guardado fora do estado.
   *
   * Ele carrega um handle nativo e não é serializável; pô-lo no `useState`
   * funcionaria, mas convida a compará-lo ou logá-lo. E `check()` não pode ser
   * chamado de novo só para reobter o mesmo objeto — cada chamada é uma
   * requisição de rede.
   */
  const pending = useRef<Update | null>(null);

  /** Evita duas checagens concorrentes (intervalo + foco da janela, por exemplo). */
  const busy = useRef(false);

  /**
   * Espelho do estado para o `runCheck` ler.
   *
   * Ler `state` direto tornaria `runCheck` dependente dele, e o `useEffect` do
   * intervalo recriaria o timer a cada mudança — inclusive a cada tique de
   * progresso do download, que nunca chegaria ao fim das seis horas.
   */
  const stateRef = useRef(state);
  stateRef.current = state;

  const runCheck = useCallback(async () => {
    if (busy.current) return;
    // Não interrompe um download em curso nem sobrescreve um "pronto para reiniciar".
    if (stateRef.current.kind !== "idle") return;
    busy.current = true;
    try {
      const update = await check();
      if (update) {
        pending.current = update;
        setState({
          kind: "available",
          version: update.version,
          notes: summarizeNotes(update.body),
        });
      }
    } catch {
      // Silêncio deliberado — ver o doc-comment do hook.
    } finally {
      busy.current = false;
    }
  }, []);

  useEffect(() => {
    const first = window.setTimeout(runCheck, FIRST_CHECK_DELAY_MS);
    const timer = window.setInterval(runCheck, CHECK_INTERVAL_MS);
    return () => {
      window.clearTimeout(first);
      window.clearInterval(timer);
    };
  }, [runCheck]);

  /** Baixa e instala. O app só reinicia quando o usuário mandar. */
  const install = useCallback(async () => {
    const update = pending.current;
    if (!update) return;
    const version = update.version;
    const progress = new DownloadProgress();
    setState({ kind: "downloading", version, progress: null });
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          progress.start(event.data.contentLength);
        } else if (event.event === "Progress") {
          progress.advance(event.data.chunkLength);
          setState({ kind: "downloading", version, progress: progress.fraction });
        }
      });
      setState({ kind: "ready", version });
    } catch (e) {
      setState({ kind: "error", version, message: updateErrorMessage(e) });
    }
  }, []);

  /** "Depois": some com o toast até a próxima checagem (ou a próxima abertura). */
  const dismiss = useCallback(() => {
    // O `Update` fica guardado: se o usuário mudar de ideia na próxima checagem,
    // não é preciso outra volta de rede para reencontrar a mesma versão.
    setState({ kind: "idle" });
  }, []);

  const restart = useCallback(async () => {
    await relaunch();
  }, []);

  return { state, install, dismiss, restart, retry: install };
}
