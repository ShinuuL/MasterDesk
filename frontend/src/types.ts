export type Priority = "Low" | "Medium" | "High" | "Urgent";

export interface Note {
  id: string;
  title: string;
  content: string;
  tags: string[];
  priority: Priority;
  deadline: string | null;
  color: string;
  opacity: number;
  pinned: boolean;
  always_on_top: boolean;
  archived: boolean;
  position: [number, number];
  size: [number, number];
  created_at: string;
  updated_at: string;
}

export interface CreateNotePayload {
  title: string;
  content: string;
  tags?: string[];
  priority?: Priority;
  color?: string;
  opacity?: number;
  position?: [number, number];
  size?: [number, number];
}

export interface UpdateNotePayload {
  title?: string;
  content?: string;
  tags?: string[];
  priority?: Priority;
  color?: string;
  opacity?: number;
  deadline?: string | null;
  position?: [number, number];
  size?: [number, number];
  pinned?: boolean;
  always_on_top?: boolean;
}

// ---------------------------------------------------------------------------
// Tasks (Fase 3)
// ---------------------------------------------------------------------------

export interface Task {
  id: string;
  title: string;
  description: string;
  priority: Priority;
  deadline: string | null;
  reminder_thresholds: ReminderThreshold[];
  completed: boolean;
  /** Origem externa. `null` = tarefa local (o caso padrão). */
  external: ExternalRef | null;
  /**
   * Vínculo manual com um chamado, criado pelo usuário aqui dentro.
   *
   * Diferente de `external`: aqui a tarefa é local e o dono é o usuário —
   * nenhuma sincronização a sobrescreve nem a retira do quadro. É o que
   * permite acompanhar trabalho de um chamado sem escrever no Mastersys.
   */
  link: TicketLink | null;
  created_at: string;
  updated_at: string;
}

/** Vínculo manual de uma tarefa local a um chamado (item 2 das features). */
export interface TicketLink {
  ticket: string;
  client: string | null;
  /** Status escrito pelo usuário. Livre — não vem do catálogo do Mastersys. */
  custom_status: string | null;
}

export type ReminderThreshold =
  | { Minutes: number }
  | { Hours: number }
  | { Custom: { minutes_before: number } };

export interface CreateTaskPayload {
  title: string;
  description?: string;
  priority?: Priority;
  deadline?: string;
  reminder_thresholds?: number[];
  link?: TicketLinkPayload;
}

export interface UpdateTaskPayload {
  title?: string;
  description?: string;
  priority?: Priority;
  deadline?: string | null;
  reminder_thresholds?: number[];
  /** Grava/substitui o vínculo. Ausente = não mexe. */
  link?: TicketLinkPayload;
  /** Remove o vínculo. Flag explícita para não confundir com "ausente". */
  unlink?: boolean;
}

export interface TicketLinkPayload {
  ticket: string;
  client?: string | null;
  custom_status?: string | null;
}

// ---------------------------------------------------------------------------
// Auth (Fase 4)
// ---------------------------------------------------------------------------

export interface AuthPayload {
  id: string;
  username: string;
  created_at: string;
  authenticated: boolean;
}

export interface RegisterPayload {
  username: string;
  password: string;
}

export interface LoginPayload {
  username: string;
  password: string;
}

// ---------------------------------------------------------------------------
// Anotações dentro de tarefas
// ---------------------------------------------------------------------------

export interface TaskNote {
  id: string;
  task_id: string;
  content: string;
  done: boolean;
  created_at: string;
  updated_at: string;
}

// ---------------------------------------------------------------------------
// Integração com sistema de suporte (ADR-006)
// ---------------------------------------------------------------------------

export type ExternalSystem = "Mastersys";
export type ExternalKind = "Task" | "Ticket";

export interface ExternalRef {
  system: ExternalSystem;
  kind: ExternalKind;
  external_id: string;
  /** Nome do cliente, quando o item tem um. */
  client: string | null;
  /** Número do chamado vinculado. */
  ticket: string | null;
  /** Status cru da origem, ex. `aguardando_retorno_cliente`. */
  status_label: string | null;
  /**
   * A origem considera este item **parado**: em espera, concluído ou cancelado.
   *
   * Prazo de item parado não significa urgência — um chamado em pós-atendimento
   * com prazo vencido está aguardando, não atrasado. Quem preenche é o adapter,
   * a partir do catálogo de status (`MastersysTicketStatus.is_parked`).
   */
  status_parked: boolean;
  /**
   * O usuário é o **analista responsável** do item na origem (`assigned_to`).
   *
   * Os dois papéis podem ser verdadeiros (abri e sou o responsável) e os dois
   * podem ser falsos — nesse caso a origem não informou papel para este item,
   * o que é diferente de dizer que o usuário não tem papel nenhum. A UI então
   * não mostra papel, em vez de afirmar algo que não sabe.
   */
  role_analyst: boolean;
  /** O usuário é o **atendente** do item na origem (`created_by`). */
  role_attendant: boolean;
}

/**
 * Um status de chamado como cadastrado no Mastersys.
 *
 * Espelhado localmente para o quadro ter o rótulo em pt-BR ("Pós Atendimento",
 * não "pos atendimento"), a cor definida na origem, e saber quais status vêm
 * pré-marcados no filtro.
 */
export interface MastersysTicketStatus {
  /** Slug, ex. `pos_atendimento`. Casa com `ExternalRef.status_label`. */
  value: string;
  label: string;
  /** Hex da origem. NÃO usar cru — passar por `noteSurface()` (ADR-009). */
  color: string;
  /** Vem pré-marcado no filtro. `false` para finalizado/cancelado/pós-atendimento. */
  default_filter: boolean;
  is_final: boolean;
  pauses_sla: boolean;
  display_order: number;
}

/**
 * Item de trabalho cru vindo da origem, antes de virar espelho local.
 *
 * O quadro não usa isto no fluxo normal — os espelhos chegam como `Task`. Este
 * tipo aparece só no resultado da **busca ao vivo**, que é consulta: nada aqui
 * está gravado no banco local.
 */
export interface ExternalWorkItem {
  reference: ExternalRef;
  title: string;
  description: string;
  priority: Priority;
  deadline: string | null;
  completed: boolean;
  /** Saiu da fila na origem (cancelado). Sinal de sincronização, não de exibição. */
  removed: boolean;
}

export interface SupportIdentity {
  system: ExternalSystem;
  user_id: string;
  display_name: string;
  email: string | null;
}

export interface MastersysStatus {
  endpoint: string | null;
  connected: boolean;
  identity: SupportIdentity | null;
  /**
   * Janela, em dias, de chamados buscados na sincronização.
   *
   * Vale mostrar na tela de integração: um chamado aberto mais antigo que a
   * janela não aparece no quadro, e sem essa informação a ausência parece bug.
   */
  ticket_window_days: number;
}

/**
 * Resultado da última sincronização automática.
 *
 * Existe porque a falha do sync automático é silenciosa de propósito, e sem
 * registro "está demorando" e "não está acontecendo" ficam indistinguíveis.
 */
export interface LastSync {
  /** ISO8601 UTC. */
  at: string;
  /** `timer`, `tempo real` ou `pedido`. */
  trigger: string;
  /** `null` = deu certo. */
  error: string | null;
  report: SyncReport | null;
}

export interface SyncReport {
  imported: number;
  updated: number;
  removed: number;
  kept_with_notes: number;
}
