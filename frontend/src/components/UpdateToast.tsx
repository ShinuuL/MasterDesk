import { useUpdate } from "../update/useUpdate";

/**
 * Toast de "atualização disponível", no canto inferior direito.
 *
 * Fica fora do fluxo das abas de propósito: a atualização não pertence a Notas,
 * Tarefas ou Chamados, e um aviso dentro de um quadro sumiria ao trocar de aba
 * bem no momento em que o usuário foi procurá-lo.
 *
 * Não é modal. Quem está no meio de um chamado não deve ser interrompido por
 * uma versão nova — o toast espera, e "Depois" o tira da frente até a próxima
 * checagem.
 */
export function UpdateToast() {
  const { state, install, dismiss, restart, retry } = useUpdate();

  if (state.kind === "idle") return null;

  const busy = state.kind === "downloading";

  return (
    <div
      className="md-update-toast"
      role="status"
      /* `polite`: leitores de tela terminam a frase em curso antes de anunciar.
         `assertive` cortaria a leitura de um chamado por causa de um aviso que
         não tem urgência nenhuma. */
      aria-live="polite"
    >
      <div className="md-update-head">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
             strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden>
          <path d="M12 3v12" />
          <path d="M7 10l5 5 5-5" />
          <path d="M4 20h16" />
        </svg>
        <strong>
          {state.kind === "ready"
            ? "Atualização instalada"
            : state.kind === "downloading"
              ? "Baixando atualização"
              : state.kind === "error"
                ? "Não foi possível atualizar"
                : "Atualização disponível"}
        </strong>
        <span className="md-update-version">{state.version}</span>
      </div>

      {state.kind === "available" && state.notes && (
        <p className="md-update-notes">{state.notes}</p>
      )}

      {state.kind === "ready" && (
        <p className="md-update-notes">
          Reinicie para usar a versão nova. Suas notas e tarefas ficam onde estão.
        </p>
      )}

      {state.kind === "error" && <p className="md-update-error">{state.message}</p>}

      {busy && (
        <div
          className="md-update-bar"
          role="progressbar"
          aria-label="Progresso do download"
          {...(state.progress === null
            ? {}
            : {
                "aria-valuemin": 0,
                "aria-valuemax": 100,
                "aria-valuenow": Math.round(state.progress * 100),
              })}
        >
          <div
            className={
              state.progress === null
                ? "md-update-bar-fill md-update-bar-fill--indeterminate"
                : "md-update-bar-fill"
            }
            style={state.progress === null ? undefined : { width: `${state.progress * 100}%` }}
          />
        </div>
      )}

      <div className="md-update-actions">
        {state.kind === "available" && (
          <>
            <button className="md-tab" onClick={dismiss}>Depois</button>
            <button className="md-primary" onClick={install}>Atualizar</button>
          </>
        )}
        {state.kind === "downloading" && (
          <span className="md-update-hint">
            {state.progress === null
              ? "Baixando…"
              : `${Math.round(state.progress * 100)}%`}
          </span>
        )}
        {state.kind === "ready" && (
          <>
            <button className="md-tab" onClick={dismiss}>Depois</button>
            <button className="md-primary" onClick={restart}>Reiniciar agora</button>
          </>
        )}
        {state.kind === "error" && (
          <>
            <button className="md-tab" onClick={dismiss}>Fechar</button>
            <button className="md-primary" onClick={retry}>Tentar de novo</button>
          </>
        )}
      </div>
    </div>
  );
}
