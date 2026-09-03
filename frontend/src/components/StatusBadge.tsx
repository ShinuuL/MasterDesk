import type { MastersysTicketStatus } from "../types";
import { noteSurface } from "../theme/noteSurface";
import { useTheme } from "../theme/useTheme";

interface Props {
  /** Slug cru da origem, ex. `pos_atendimento`. */
  statusLabel: string;
  /** Catálogo espelhado. Vazio = ainda não sincronizou. */
  catalog: MastersysTicketStatus[];
  /** A origem considera o item parado (vem de `ExternalRef.status_parked`). */
  parked?: boolean;
}

/** `aguardando_retorno_cliente` → `aguardando retorno cliente`. */
function humanizeStatus(raw: string): string {
  return raw.replace(/_/g, " ").trim();
}

/**
 * Status do item de origem, como selo colorido.
 *
 * ## Por que existe
 *
 * Antes o status era a última coisa da linha do selo, em `--text-faint`,
 * minúscula. Num card cheio de informação ele desaparecia — e é justamente o
 * dado que decide se o item precisa de ação agora.
 *
 * ## Cor
 *
 * O hex vem do cadastro do Mastersys, então o MasterNote e o suporte falam a
 * mesma língua visual, inclusive para status que o cliente criou.
 *
 * A cor **não** é aplicada crua: passa por `noteSurface()`, que ajusta a
 * luminosidade (nunca o matiz) até bater o piso de contraste AA nos dois temas
 * — ADR-009. Sem isso, um `#f59e0b` do suporte ficaria ilegível no tema escuro.
 *
 * ## Degradação
 *
 * Sem catálogo — nunca sincronizou, ou o endpoint falhou — cai no slug
 * humanizado sem cor, que é o comportamento anterior. Melhor um selo sem
 * enfeite que um espaço vazio.
 */
export function StatusBadge({ statusLabel, catalog, parked }: Props) {
  const { theme } = useTheme();
  const entry = catalog.find((s) => s.value === statusLabel);

  if (!entry) {
    return (
      <span className="md-status-badge md-status-badge--plain">
        {humanizeStatus(statusLabel)}
      </span>
    );
  }

  const surface = noteSurface(entry.color, theme === "dark");

  return (
    <span
      className={`md-status-badge ${parked ? "md-status-badge--parked" : ""}`}
      style={{
        background: surface.background,
        color: surface.text,
        borderColor: surface.border,
      }}
      // Item parado precisa se explicar: o usuário acabou de notar que ele
      // não está mais marcado como atrasado, e o motivo tem de estar à mão.
      title={
        parked
          ? `${entry.label} — a origem considera este item parado, então ele não conta como atrasado nem gera lembrete`
          : entry.label
      }
    >
      {entry.label}
    </span>
  );
}
