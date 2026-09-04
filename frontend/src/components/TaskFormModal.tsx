import { useState } from "react";
import type { CreateTaskPayload, ExternalWorkItem, Priority } from "../types";
import * as api from "../api";
import { Modal } from "./Modal";

const PRIORITIES: Priority[] = ["Low", "Medium", "High", "Urgent"];

const PRIORITY_LABEL: Record<Priority, string> = {
  Low: "Baixa",
  Medium: "Média",
  High: "Alta",
  Urgent: "Urgente",
};

const PRESET_THRESHOLDS: { label: string; minutes: number }[] = [
  { label: "5m", minutes: 5 },
  { label: "10m", minutes: 10 },
  { label: "15m", minutes: 15 },
  { label: "30m", minutes: 30 },
  { label: "1h", minutes: 60 },
  { label: "2h", minutes: 120 },
];

/** Mínimo aceito pelo backend em `mastersys_search_tickets`. */
const SEARCH_MIN_CHARS = 3;

interface Props {
  /**
   * Como o diálogo abre: `plain` = tarefa comum (o vínculo fica atrás de um
   * interruptor), `link` = já aberto no vínculo.
   */
  mode: "plain" | "link";
  /**
   * Chamado que o vínculo já vem apontando — vem do card de onde o usuário
   * clicou "Vincular tarefa". `null` = ele escolhe (digitando ou buscando).
   */
  initialLink?: { ticket: string; client: string | null } | null;
  onClose: () => void;
  /** Chamado depois de criar, para o quadro recarregar. */
  onCreated: () => void;
}

/**
 * Criação de tarefa em diálogo — e, no modo `link`, o "vincular nova tarefa".
 *
 * ## O que o modo `link` faz, e o que deliberadamente não faz
 *
 * Cria uma tarefa **local** que aponta para um chamado. O Mastersys não é
 * tocado: a integração não tem escrita (ADR-006), e uma tarefa criada lá
 * apareceria na fila de outra pessoa e mudaria o quadro do suporte. O pedido
 * era exatamente o contrário — acompanhar um chamado "sem interferir na
 * Mastersys".
 *
 * Também não grava espelho (`external`). Espelho que não volta na fila da
 * origem é retirado pela sincronização; esta tarefa precisa sobreviver, porque
 * o dono dela é o usuário.
 *
 * ## Status personalizado
 *
 * Texto livre. Não há cadastro para validar contra, e reaproveitar o catálogo
 * do Mastersys seria mentir sobre a procedência do valor: o selo dele é
 * tracejado no card justamente para não se confundir com o status da origem.
 *
 * ## O número do chamado
 *
 * Pode ser digitado ou escolhido na busca do Mastersys — a busca é
 * conveniência, não exigência. Digitar direto continua valendo (chamado antigo
 * fora da janela de sincronização, ou instalação sem busca).
 */
export function TaskFormModal({ mode, initialLink = null, onClose, onCreated }: Props) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [priority, setPriority] = useState<Priority>("Medium");
  const [deadlineLocal, setDeadlineLocal] = useState("");
  const [thresholds, setThresholds] = useState<Set<number>>(new Set());
  const [customMinutes, setCustomMinutes] = useState("");

  /**
   * Vincular está ligado?
   *
   * Interruptor, e não um segundo botão no quadro: vincular é a mesma criação
   * de tarefa com um campo a mais. O caminho comum é clicar "Vincular tarefa"
   * no card do chamado, que já entra aqui ligado e preenchido; o interruptor
   * cobre o caso de vincular a um chamado que não está no quadro.
   */
  const [linking, setLinking] = useState(mode === "link");
  const [ticket, setTicket] = useState(initialLink?.ticket ?? "");
  const [client, setClient] = useState(initialLink?.client ?? "");
  const [customStatus, setCustomStatus] = useState("");

  const [searchTerm, setSearchTerm] = useState("");
  const [results, setResults] = useState<ExternalWorkItem[] | null>(null);
  const [searching, setSearching] = useState(false);

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canSave = title.trim().length > 0 && (!linking || ticket.trim().length > 0);

  const toggleThreshold = (minutes: number) => {
    setThresholds((prev) => {
      const next = new Set(prev);
      if (next.has(minutes)) next.delete(minutes);
      else next.add(minutes);
      return next;
    });
  };

  const handleSearch = async () => {
    setSearching(true);
    setError(null);
    try {
      setResults(await api.mastersysSearchTickets(searchTerm));
    } catch (e) {
      setError(String(e));
      setResults(null);
    } finally {
      setSearching(false);
    }
  };

  /** Preenche o formulário com o chamado escolhido, sem substituir o que o
   *  usuário já escreveu à mão. */
  const chooseTicket = (item: ExternalWorkItem) => {
    const ref = item.reference;
    setTicket(ref.ticket ?? "");
    setClient(ref.client ?? "");
    if (!title.trim()) setTitle(item.title);
    setResults(null);
  };

  const handleSave = async () => {
    if (!canSave) return;
    setSaving(true);
    setError(null);
    try {
      const minutes = new Set<number>(thresholds);
      const custom = parseInt(customMinutes, 10);
      if (!isNaN(custom) && custom > 0) minutes.add(custom);
      const thresholdsArr = Array.from(minutes);

      const deadline =
        deadlineLocal && !isNaN(new Date(deadlineLocal).getTime())
          ? new Date(deadlineLocal).toISOString()
          : undefined;

      const payload: CreateTaskPayload = {
        title: title.trim(),
        description: description.trim() || undefined,
        priority,
        deadline,
        reminder_thresholds: thresholdsArr.length > 0 ? thresholdsArr : undefined,
      };
      if (linking) {
        payload.link = {
          ticket: ticket.trim(),
          client: client.trim() || null,
          custom_status: customStatus.trim() || null,
        };
      }

      await api.createTask(payload);
      onCreated();
      onClose();
    } catch (e) {
      // Erro de validação do domínio (status longo, chamado vazio) chega aqui
      // e fica no diálogo: fechar levaria embora o que a pessoa digitou.
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <Modal
      title={
        linking
          ? initialLink
            ? `Nova tarefa vinculada ao chamado #${initialLink.ticket}`
            : "Nova tarefa vinculada a um chamado"
          : "Nova tarefa"
      }
      hint={
        linking
          ? "Cria uma tarefa sua apontando para um chamado. Nada é escrito no Mastersys, e a sincronização não altera nem remove esta tarefa."
          : undefined
      }
      onClose={onClose}
      footer={
        <>
          {error && (
            <p className="md-modal-error" style={{ marginRight: "auto" }}>
              {error}
            </p>
          )}
          <button className="md-btn" onClick={onClose} disabled={saving}>
            Cancelar
          </button>
          <button
            className="md-primary md-primary-accent"
            onClick={() => void handleSave()}
            disabled={!canSave || saving}
            aria-disabled={!canSave || saving}
          >
            {saving ? "Salvando…" : linking ? "Criar e vincular" : "Criar tarefa"}
          </button>
        </>
      }
    >
      <label className="md-toggle">
        <input
          type="checkbox"
          checked={linking}
          onChange={(e) => setLinking(e.target.checked)}
        />
        Vincular a um chamado
      </label>

      {linking && (
        <section className="md-panel-section">
          <label htmlFor="link-search" className="md-eyebrow">
            {initialLink
              ? "Trocar o chamado (opcional)"
              : "Buscar o chamado no Mastersys (opcional)"}
          </label>
          <div className="md-modal-row">
            <input
              id="link-search"
              className="md-input"
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              placeholder="Nº, título, cliente…"
              onKeyDown={(e) => {
                if (e.key === "Enter" && searchTerm.trim().length >= SEARCH_MIN_CHARS) {
                  void handleSearch();
                }
              }}
            />
            <button
              className="md-btn"
              style={{ flex: "0 0 auto" }}
              onClick={() => void handleSearch()}
              disabled={searching || searchTerm.trim().length < SEARCH_MIN_CHARS}
            >
              {searching ? "Buscando…" : "Buscar"}
            </button>
          </div>
          {results !== null &&
            (results.length === 0 ? (
              <p className="md-panel-note">
                Nenhum chamado encontrado. Você ainda pode digitar o número
                abaixo.
              </p>
            ) : (
              <ul className="md-remote-list">
                {results.map((item) => (
                  <li key={item.reference.external_id} className="md-remote-item">
                    {item.reference.ticket && (
                      <span className="md-stamp-ticket">#{item.reference.ticket}</span>
                    )}
                    <span className="md-remote-title">{item.title}</span>
                    {item.reference.client && (
                      <span className="md-stamp-client">{item.reference.client}</span>
                    )}
                    <button className="md-btn md-btn--ghost" onClick={() => chooseTicket(item)}>
                      Usar este
                    </button>
                  </li>
                ))}
              </ul>
            ))}

          <div className="md-modal-row">
            <div className="md-field">
              <label htmlFor="link-ticket">Nº do chamado</label>
              <input
                id="link-ticket"
                className="md-input"
                value={ticket}
                onChange={(e) => setTicket(e.target.value)}
                placeholder="ex. 991"
                maxLength={64}
                required
              />
            </div>
            <div className="md-field">
              <label htmlFor="link-client">Cliente (opcional)</label>
              <input
                id="link-client"
                className="md-input"
                value={client}
                onChange={(e) => setClient(e.target.value)}
                maxLength={200}
              />
            </div>
          </div>

          <div className="md-field">
            <label htmlFor="link-status">Status personalizado (opcional)</label>
            <input
              id="link-status"
              className="md-input"
              value={customStatus}
              onChange={(e) => setCustomStatus(e.target.value)}
              placeholder="ex. aguardando peça"
              maxLength={64}
            />
            <span className="md-panel-note">
              Seu vocabulário, não o do Mastersys — aparece no card com contorno
              tracejado para não se confundir com o status do chamado.
            </span>
          </div>
        </section>
      )}

      <div className="md-field">
        <label htmlFor="task-modal-title">Título</label>
        <input
          id="task-modal-title"
          className="md-input"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="O que precisa ser feito"
          maxLength={200}
          autoFocus={!linking}
        />
      </div>

      <div className="md-field">
        <label htmlFor="task-modal-desc">Descrição</label>
        <textarea
          id="task-modal-desc"
          className="md-input"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          placeholder="Detalhe o que for necessário para retomar a tarefa depois"
          rows={5}
        />
      </div>

      <div className="md-modal-row">
        <div className="md-field">
          <label htmlFor="task-modal-prio">Prioridade</label>
          <select
            id="task-modal-prio"
            className="md-select"
            value={priority}
            onChange={(e) => setPriority(e.target.value as Priority)}
          >
            {PRIORITIES.map((p) => (
              <option key={p} value={p}>
                {PRIORITY_LABEL[p]}
              </option>
            ))}
          </select>
        </div>
        <div className="md-field">
          <label htmlFor="task-modal-deadline">Prazo</label>
          <input
            id="task-modal-deadline"
            type="datetime-local"
            className="md-input"
            value={deadlineLocal}
            onChange={(e) => setDeadlineLocal(e.target.value)}
          />
        </div>
      </div>

      <div className="md-field">
        <span className="md-eyebrow">Lembretes antes do prazo</span>
        <div style={{ display: "flex", gap: 6, alignItems: "center", flexWrap: "wrap" }}>
          {PRESET_THRESHOLDS.map((p) => (
            <button
              key={p.minutes}
              type="button"
              className="md-chip"
              aria-pressed={thresholds.has(p.minutes)}
              onClick={() => toggleThreshold(p.minutes)}
            >
              {p.label}
            </button>
          ))}
          <label style={{ fontSize: 12, display: "flex", alignItems: "center", gap: 6 }}>
            Outro:
            <input
              type="number"
              min={1}
              value={customMinutes}
              onChange={(e) => setCustomMinutes(e.target.value)}
              placeholder="min"
              className="md-input"
              style={{ width: 72, padding: "6px 8px", minHeight: 32 }}
            />
          </label>
        </div>
        {!deadlineLocal && thresholds.size > 0 && (
          <span className="md-panel-note">
            Lembrete só dispara com prazo definido — escolha um acima.
          </span>
        )}
      </div>
    </Modal>
  );
}
