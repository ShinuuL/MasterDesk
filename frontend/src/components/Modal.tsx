import { useEffect, useRef, type ReactNode } from "react";

interface Props {
  title: string;
  /** Frase curta sob o título, quando o diálogo precisa se explicar. */
  hint?: string;
  onClose: () => void;
  /** Rodapé — normalmente Cancelar + ação primária. */
  footer?: ReactNode;
  children: ReactNode;
  /** Largura máxima. `wide` para conteúdo de leitura (chamado). */
  size?: "default" | "wide";
}

/**
 * Diálogo centralizado, reusado por todos os fluxos de criação e leitura.
 *
 * ## Por que existe
 *
 * Criar nota e tarefa era feito numa barra de campos sempre visível no topo do
 * quadro. Isso custava altura em todas as telas para um ato pontual, e não
 * cabia mais: descrever a tarefa, escolher lembretes e vincular um chamado não
 * entram numa linha. O diálogo dá espaço ao formulário e devolve o espaço ao
 * conteúdo.
 *
 * ## Acessibilidade
 *
 * Segue o mesmo contrato do `MastersysPanel` (`role="dialog"`,
 * `aria-modal`, foco programático ao abrir, clique no fundo fecha) e
 * acrescenta **Esc para fechar**, que a barra de campos não tinha porque não
 * era um contexto modal.
 *
 * O foco vai para o próprio diálogo, não para o primeiro campo: assim o leitor
 * de tela anuncia o título e a explicação antes de o usuário começar a digitar.
 * Quem quiser foco num campo específico usa `autoFocus` nele.
 *
 * Não há armadilha de foco (focus trap): o app é uma janela só, sem conteúdo
 * navegável atrás que valha proteger, e implementar uma meia-boca costuma
 * prender o Tab em vez de organizá-lo.
 */
export function Modal({ title, hint, onClose, footer, children, size = "default" }: Props) {
  const dialogRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    dialogRef.current?.focus();
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="md-modal-overlay" onClick={onClose} role="presentation">
      <div
        ref={dialogRef}
        className={`md-modal ${size === "wide" ? "md-modal--wide" : ""}`}
        onClick={(e) => e.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        tabIndex={-1}
      >
        <header className="md-modal-head">
          <div>
            <h2>{title}</h2>
            {hint && <p className="md-modal-hint">{hint}</p>}
          </div>
          <button className="md-panel-close" onClick={onClose} aria-label="Fechar">
            ✕
          </button>
        </header>

        <div className="md-modal-body scroll-hidden">{children}</div>

        {footer && <footer className="md-modal-foot">{footer}</footer>}
      </div>
    </div>
  );
}
