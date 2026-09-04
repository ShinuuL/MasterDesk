import type { MastersysTicketStatus, Task } from "../types";
import { StatusBadge } from "./StatusBadge";

interface Props {
  task: Task;
  catalog: MastersysTicketStatus[];
  parked: boolean;
  /** Abre a leitura do chamado. Ausente = sem chamado para abrir. */
  onOpenTicket?: () => void;
}

/**
 * Linha de origem do card — **sempre presente**, inclusive em tarefa local.
 *
 * ## Por que sempre
 *
 * Antes o selo só era renderizado quando `task.external` existia. Uma tarefa
 * local não dizia nada sobre si, então "isto é local" era comunicado pela
 * ausência de selo — informação por omissão, legível só para quem já conhece o
 * app. Num quadro que mistura as duas coisas, saber de onde o item vem decide
 * o que se pode esperar dele (o espelho é sobrescrito pelo sync; a tarefa
 * local não).
 *
 * ## As três origens
 *
 * | Selo                    | Significado |
 * |-------------------------|-------------|
 * | `Local`                 | tarefa só sua, nenhum vínculo |
 * | `Local · Chamado #N`    | tarefa sua, vinculada por você a um chamado |
 * | `Chamado/Tarefa · Mastersys` | espelho do que está atribuído a você na origem |
 *
 * A distinção entre as duas primeiras importa porque só a segunda tem número
 * de chamado — e nenhuma das duas é tocada pela sincronização, o que é
 * justamente a diferença em relação à terceira.
 */
export function TaskOriginStamp({ task, catalog, parked, onOpenTicket }: Props) {
  const external = task.external;
  const link = task.link;

  const roles = external
    ? [
        external.role_analyst ? "Analista" : null,
        external.role_attendant ? "Atendente" : null,
      ].filter((r): r is string => r !== null)
    : [];

  const ticket = external?.ticket ?? link?.ticket ?? null;
  const client = external?.client ?? link?.client ?? null;

  return (
    <div className="md-stamp">
      {/* Status primeiro: é o dado que decide se o item precisa de ação agora. */}
      {external?.status_label && (
        <StatusBadge
          statusLabel={external.status_label}
          catalog={catalog}
          parked={parked}
        />
      )}
      {/* Status personalizado da tarefa vinculada. Contorno tracejado porque
          é vocabulário do usuário, não do catálogo do Mastersys. */}
      {!external && link?.custom_status && (
        <span
          className="md-status-badge md-status-badge--custom"
          title="Status criado por você nesta tarefa — não existe no Mastersys"
        >
          {link.custom_status}
        </span>
      )}

      {external ? (
        <span className="md-stamp-origin">
          {external.kind === "Ticket" ? "Chamado" : "Tarefa"} · Mastersys
        </span>
      ) : (
        <span
          className={`md-stamp-origin ${link ? "md-stamp-origin--linked" : "md-stamp-origin--local"}`}
          title={
            link
              ? "Tarefa sua, vinculada a um chamado. A sincronização não a altera nem a remove."
              : "Tarefa só sua — nenhum vínculo com o Mastersys."
          }
        >
          {link ? "Local · vinculada" : "Local"}
        </span>
      )}

      {/* Papel só aparece quando a origem informou: dois papéis diferentes
          (analista e atendente) e nenhum inventado. */}
      {roles.map((role) => (
        <span
          key={role}
          className="md-stamp-role"
          title={
            role === "Analista"
              ? "Você é o analista responsável deste chamado no Mastersys"
              : "Você é o atendente — quem abriu ou assumiu este chamado"
          }
        >
          {role}
        </span>
      ))}

      {ticket &&
        (onOpenTicket ? (
          <button
            type="button"
            className="md-stamp-ticket md-stamp-link"
            onClick={onOpenTicket}
            title="Ver os dados do chamado"
          >
            #{ticket}
          </button>
        ) : (
          <span className="md-stamp-ticket">#{ticket}</span>
        ))}

      {client && (
        <span className="md-stamp-client" title={client}>
          {client}
        </span>
      )}
    </div>
  );
}
