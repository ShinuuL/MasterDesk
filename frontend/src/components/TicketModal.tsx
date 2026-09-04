import { useState } from "react";
import type { MastersysTicketStatus, Task } from "../types";
import * as api from "../api";
import { Modal } from "./Modal";
import { StatusBadge } from "./StatusBadge";

interface Props {
  task: Task;
  catalog: MastersysTicketStatus[];
  parked: boolean;
  onClose: () => void;
  /** Chamado após alterar ou remover o vínculo, para o quadro recarregar. */
  onLinkChanged?: () => void;
}

function formatDeadline(iso: string | null): string {
  if (!iso) return "sem prazo";
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  return d.toLocaleString("pt-BR", { dateStyle: "short", timeStyle: "short" });
}

/**
 * Leitura do chamado ligado a uma tarefa, sem sair do quadro.
 *
 * ## O que aparece — e por que não mais que isso
 *
 * Só o que o MasterDesk **já tem espelhado**: número, cliente, status, prazo,
 * título, descrição e o papel do usuário. Nenhuma chamada nova é feita ao
 * abrir.
 *
 * Buscar o chamado completo (comentários, histórico, anexos) exigiria um
 * endpoint de detalhe cujo contrato não foi validado — e a Regra 1 do
 * CLAUDE.md proíbe inventar. Mostrar campos vazios de dados que o app não
 * puxa seria pior que não mostrá-los: pareceria chamado sem informação, em vez
 * de informação que este app não busca.
 *
 * O rodapé diz de onde vem o que está na tela, para o usuário saber quando
 * ainda precisa abrir o Mastersys.
 */
export function TicketModal({ task, catalog, parked, onClose, onLinkChanged }: Props) {
  const external = task.external;
  const link = task.link;

  // Status personalizado é editável aqui porque status muda — um valor que só
  // pode ser escrito na criação da tarefa não é status, é rótulo fixo.
  const [status, setStatus] = useState(link?.custom_status ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dirty = link !== null && status.trim() !== (link?.custom_status ?? "");

  const saveStatus = async () => {
    if (!link) return;
    setSaving(true);
    setError(null);
    try {
      await api.updateTask(task.id, {
        link: {
          ticket: link.ticket,
          client: link.client,
          custom_status: status.trim() || null,
        },
      });
      onLinkChanged?.();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const unlink = async () => {
    if (!link) return;
    if (!confirm(`Desvincular esta tarefa do chamado #${link.ticket}? A tarefa continua no quadro.`)) {
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await api.updateTask(task.id, { unlink: true });
      onLinkChanged?.();
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };
  const ticket = external?.ticket ?? link?.ticket ?? null;
  const client = external?.client ?? link?.client ?? null;

  const roles = [
    external?.role_analyst ? "Analista responsável" : null,
    external?.role_attendant ? "Atendente" : null,
  ].filter((r): r is string => r !== null);

  return (
    <Modal
      title={ticket ? `Chamado #${ticket}` : "Chamado"}
      hint={
        external
          ? "Espelho do Mastersys. Editar aqui não altera o chamado."
          : "Tarefa sua, vinculada a este chamado. Nada aqui é escrito no Mastersys."
      }
      onClose={onClose}
      size="wide"
      footer={
        <>
          {error ? (
            <p className="md-modal-error" style={{ marginRight: "auto" }}>
              {error}
            </p>
          ) : (
            <span className="md-panel-note" style={{ marginRight: "auto" }}>
              Estes são os dados que o MasterDesk espelha. Comentários,
              histórico e anexos continuam no Mastersys.
            </span>
          )}
          {link && (
            <button className="md-btn md-btn--danger" onClick={() => void unlink()} disabled={saving}>
              Desvincular
            </button>
          )}
          <button className="md-btn" onClick={onClose} disabled={saving}>
            Fechar
          </button>
          {link && (
            <button
              className="md-primary md-primary-accent"
              onClick={() => void saveStatus()}
              disabled={!dirty || saving}
              aria-disabled={!dirty || saving}
            >
              {saving ? "Salvando…" : "Salvar status"}
            </button>
          )}
        </>
      }
    >
      <dl className="md-ticket-grid">
        <dt>Título</dt>
        <dd>{task.title}</dd>

        {client && (
          <>
            <dt>Cliente</dt>
            <dd>{client}</dd>
          </>
        )}

        <dt>Status</dt>
        <dd>
          {external?.status_label ? (
            <StatusBadge
              statusLabel={external.status_label}
              catalog={catalog}
              parked={parked}
            />
          ) : link?.custom_status ? (
            <span className="md-status-badge md-status-badge--custom">
              {link.custom_status}
            </span>
          ) : (
            <span className="md-quiet">não informado</span>
          )}
        </dd>

        <dt>Prazo</dt>
        <dd>{formatDeadline(task.deadline)}</dd>

        {roles.length > 0 && (
          <>
            <dt>Seu papel</dt>
            <dd>{roles.join(" e ")}</dd>
          </>
        )}

        <dt>Origem</dt>
        <dd>
          {external
            ? external.kind === "Ticket"
              ? "Chamado do Mastersys (espelhado)"
              : "Tarefa do Mastersys (espelhada)"
            : "Tarefa local vinculada por você"}
        </dd>
      </dl>

      {task.description && <div className="md-ticket-desc">{task.description}</div>}

      {link && (
        <div className="md-field">
          <label htmlFor="ticket-modal-status">Status personalizado</label>
          <input
            id="ticket-modal-status"
            className="md-input"
            value={status}
            onChange={(e) => setStatus(e.target.value)}
            placeholder="ex. aguardando peça"
            maxLength={64}
            onKeyDown={(e) => {
              if (e.key === "Enter" && dirty) void saveStatus();
            }}
          />
          <span className="md-panel-note">
            Vale só nesta tarefa e nada é enviado ao Mastersys. Em branco, o
            card fica sem selo de status.
          </span>
        </div>
      )}
    </Modal>
  );
}
