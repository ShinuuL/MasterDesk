import { invoke } from "@tauri-apps/api/core";
import type {
  Note,
  CreateNotePayload,
  UpdateNotePayload,
  Task,
  CreateTaskPayload,
  UpdateTaskPayload,
  AuthPayload,
  RegisterPayload,
  LoginPayload,
  TaskNote,
  MastersysStatus,
  SupportIdentity,
  SyncReport,
} from "./types";

export async function createNote(payload: CreateNotePayload): Promise<Note> {
  return invoke<Note>("create_note", { payload });
}

export async function getNote(id: string): Promise<Note> {
  return invoke<Note>("get_note", { id });
}

export async function listActiveNotes(): Promise<Note[]> {
  return invoke<Note[]>("list_active_notes");
}

export async function listArchivedNotes(): Promise<Note[]> {
  return invoke<Note[]>("list_archived_notes");
}

export async function listAllNotes(): Promise<Note[]> {
  return invoke<Note[]>("list_all_notes");
}

export async function updateNote(id: string, payload: UpdateNotePayload): Promise<Note> {
  return invoke<Note>("update_note", { id, payload });
}

export async function archiveNote(id: string): Promise<Note> {
  return invoke<Note>("archive_note", { id });
}

export async function unarchiveNote(id: string): Promise<Note> {
  return invoke<Note>("unarchive_note", { id });
}

export async function deleteNote(id: string): Promise<void> {
  return invoke<void>("delete_note", { id });
}

export async function togglePin(id: string): Promise<Note> {
  return invoke<Note>("toggle_pin", { id });
}

export async function setAlwaysOnTop(id: string, enabled: boolean): Promise<Note> {
  return invoke<Note>("set_always_on_top", { id, enabled });
}

export async function setWindowAlwaysOnTop(enabled: boolean): Promise<void> {
  return invoke<void>("set_window_always_on_top", { enabled });
}

// ---------------------------------------------------------------------------
// Note windows — "pinned notes" that survive main minimize
// ---------------------------------------------------------------------------

export async function openNoteWindow(
  id: string,
  title: string,
  color: string,
  x: number,
  y: number,
  w: number,
  h: number,
): Promise<void> {
  return invoke<void>("open_note_window", { id, title, color, x, y, w, h });
}

export async function closeNoteWindow(id: string): Promise<void> {
  return invoke<void>("close_note_window", { id });
}

export async function setNoteWindowAlwaysOnTop(
  id: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("set_note_window_always_on_top", { id, enabled });
}

export async function setNoteWindowPosition(
  id: string,
  x: number,
  y: number,
): Promise<void> {
  return invoke<void>("set_note_window_position", { id, x, y });
}

export async function setNoteWindowSize(
  id: string,
  w: number,
  h: number,
): Promise<void> {
  return invoke<void>("set_note_window_size", { id, w, h });
}

export async function isNoteWindowOpen(id: string): Promise<boolean> {
  return invoke<boolean>("is_note_window_open", { id });
}

// ---------------------------------------------------------------------------
// Tasks (Fase 3)
// ---------------------------------------------------------------------------

export async function createTask(payload: CreateTaskPayload): Promise<Task> {
  return invoke<Task>("create_task", { payload });
}

export async function getTask(id: string): Promise<Task> {
  return invoke<Task>("get_task", { id });
}

export async function listPendingTasks(): Promise<Task[]> {
  return invoke<Task[]>("list_pending_tasks");
}

export async function listCompletedTasks(): Promise<Task[]> {
  return invoke<Task[]>("list_completed_tasks");
}

export async function listAllTasks(): Promise<Task[]> {
  return invoke<Task[]>("list_all_tasks");
}

export async function updateTask(id: string, payload: UpdateTaskPayload): Promise<Task> {
  return invoke<Task>("update_task", { id, payload });
}

export async function completeTask(id: string): Promise<Task> {
  return invoke<Task>("complete_task", { id });
}

export async function reopenTask(id: string): Promise<Task> {
  return invoke<Task>("reopen_task", { id });
}

export async function deleteTask(id: string): Promise<void> {
  return invoke<void>("delete_task", { id });
}

export async function snoozeTask(id: string): Promise<void> {
  return invoke<void>("snooze_task", { id });
}

// ---------------------------------------------------------------------------
// Auth (Fase 4)
// ---------------------------------------------------------------------------

export async function authRegister(payload: RegisterPayload): Promise<AuthPayload> {
  return invoke<AuthPayload>("auth_register", { payload });
}

export async function authLogin(payload: LoginPayload): Promise<AuthPayload> {
  return invoke<AuthPayload>("auth_login", { payload });
}

export async function authLogout(): Promise<void> {
  return invoke<void>("auth_logout");
}

export async function authIsAuthenticated(): Promise<boolean> {
  return invoke<boolean>("auth_is_authenticated");
}

// ---------------------------------------------------------------------------
// Anotações dentro de tarefas
// ---------------------------------------------------------------------------

export async function addTaskNote(taskId: string, content: string): Promise<TaskNote> {
  return invoke<TaskNote>("add_task_note", { taskId, content });
}

export async function listTaskNotes(taskId: string): Promise<TaskNote[]> {
  return invoke<TaskNote[]>("list_task_notes", { taskId });
}

export async function countTaskNotes(taskId: string): Promise<number> {
  return invoke<number>("count_task_notes", { taskId });
}

export async function updateTaskNote(id: string, content: string): Promise<TaskNote> {
  return invoke<TaskNote>("update_task_note", { id, content });
}

export async function setTaskNoteDone(id: string, done: boolean): Promise<TaskNote> {
  return invoke<TaskNote>("set_task_note_done", { id, done });
}

export async function deleteTaskNote(id: string): Promise<void> {
  return invoke<void>("delete_task_note", { id });
}

// ---------------------------------------------------------------------------
// Integração Mastersys (ADR-006) — somente leitura
// ---------------------------------------------------------------------------

export async function mastersysStatus(): Promise<MastersysStatus> {
  return invoke<MastersysStatus>("mastersys_status");
}

export async function mastersysSetEndpoint(endpoint: string): Promise<void> {
  return invoke<void>("mastersys_set_endpoint", { endpoint });
}

export async function mastersysConnect(
  identifier: string,
  password: string,
): Promise<SupportIdentity> {
  return invoke<SupportIdentity>("mastersys_connect", { identifier, password });
}

export async function mastersysDisconnect(): Promise<SyncReport> {
  return invoke<SyncReport>("mastersys_disconnect");
}

/** `defaultReminders` em minutos antes do prazo, aplicado só a novos itens. */
export async function mastersysSync(defaultReminders: number[]): Promise<SyncReport> {
  return invoke<SyncReport>("mastersys_sync", { defaultReminders });
}
