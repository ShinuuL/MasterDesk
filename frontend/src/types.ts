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
  created_at: string;
  updated_at: string;
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
}

export interface UpdateTaskPayload {
  title?: string;
  description?: string;
  priority?: Priority;
  deadline?: string | null;
  reminder_thresholds?: number[];
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
}

export interface SyncReport {
  imported: number;
  updated: number;
  removed: number;
  kept_with_notes: number;
}
