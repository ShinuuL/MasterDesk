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
  MastersysTicketStatus,
  ExternalWorkItem,
  LastSync,
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

/**
 * Ids das notas com janela aberta agora, direto do gerenciador de janelas.
 *
 * Preferir isto a rastrear o conjunto em estado de componente: estado local se
 * perde na troca de aba, e aí a nota reaparecia no quadro com a janela ainda
 * aberta — duas superfícies editando a mesma nota.
 */
export async function openNoteWindowIds(): Promise<string[]> {
  return invoke<string[]>("open_note_window_ids");
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

/**
 * Contador de anotações de todas as tarefas, numa chamada só.
 *
 * Tarefa sem anotação não vem no mapa — quem lê trata ausência como zero.
 */
export async function countAllTaskNotes(): Promise<Record<string, number>> {
  return invoke<Record<string, number>>("count_all_task_notes");
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

// ---------------------------------------------------------------------------
// Pop-out de tarefa
// ---------------------------------------------------------------------------

export async function openTaskWindow(id: string): Promise<void> {
  return invoke<void>("open_task_window", { id });
}

export async function closeTaskWindow(id: string): Promise<void> {
  return invoke<void>("close_task_window", { id });
}

/**
 * Ids das tarefas com janela aberta, direto do gerenciador de janelas.
 *
 * Serve também para achar janela órfã: um id daqui sem tarefa correspondente é
 * um espelho que o `retire_mirror` apagou e cuja janela ficou para trás.
 */
export async function openTaskWindowIds(): Promise<string[]> {
  return invoke<string[]>("open_task_window_ids");
}

/**
 * Grava a posição da janela destacada. **Não move a janela.**
 *
 * O nome importa: a versão anterior (`setTaskWindowPosition`) também aplicava a
 * posição, e como quem chama é o listener de `onMoved`, isso virava um laço —
 * gravar, mover, disparar `onMoved`, gravar… A janela agitava freneticamente.
 */
export async function saveTaskWindowPosition(
  id: string,
  x: number,
  y: number,
): Promise<void> {
  return invoke<void>("save_task_window_position", { id, x, y });
}

/** Grava o tamanho da janela destacada. **Não redimensiona a janela.** */
export async function saveTaskWindowSize(
  id: string,
  w: number,
  h: number,
): Promise<void> {
  return invoke<void>("save_task_window_size", { id, w, h });
}

export async function setTaskWindowAlwaysOnTop(
  id: string,
  enabled: boolean,
): Promise<void> {
  return invoke<void>("set_task_window_always_on_top", { id, enabled });
}

// ---------------------------------------------------------------------------
// Sincronização automática
// ---------------------------------------------------------------------------

/** Evento emitido pelo Rust após uma sincronização automática que mudou algo. */
export const MASTERSYS_SYNCED_EVENT = "masterdesk://mastersys-synced";

export async function mastersysPollInterval(): Promise<number> {
  return invoke<number>("mastersys_poll_interval");
}

export async function mastersysSetPollInterval(seconds: number): Promise<void> {
  return invoke<void>("mastersys_set_poll_interval", { seconds });
}

/**
 * O canal de tempo real está conectado?
 *
 * Distingue "acompanha em segundos" de "acompanha em minutos". Vale mostrar:
 * sem isso o usuário não tem como saber por que uma mudança levou 5 minutos
 * para aparecer no quadro.
 */
export async function mastersysRealtimeConnected(): Promise<boolean> {
  return invoke<boolean>("mastersys_realtime_connected");
}

/**
 * O que aconteceu na última sincronização automática.
 *
 * `null` = nenhuma rodou ainda nesta execução. Responde "está demorando ou nem
 * acontecendo?" sem precisar de log.
 */
export async function mastersysLastSync(): Promise<LastSync | null> {
  return invoke<LastSync | null>("mastersys_last_sync");
}

/** Pede uma sincronização agora, sem esperar o timer. */
export async function mastersysSyncNow(): Promise<void> {
  return invoke<void>("mastersys_sync_now");
}

/**
 * Escuta sincronizações automáticas. Devolve a função de cancelamento.
 *
 * O Rust só emite quando algo mudou de fato — emitir a cada ciclo faria o
 * quadro recarregar de 5 em 5 minutos sem motivo, perdendo scroll e seleção.
 */
export async function onMastersysSynced(
  handler: (report: SyncReport) => void,
): Promise<() => void> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<SyncReport>(MASTERSYS_SYNCED_EVENT, (e) => handler(e.payload));
}

export async function mastersysStatus(): Promise<MastersysStatus> {
  return invoke<MastersysStatus>("mastersys_status");
}

export async function mastersysSetEndpoint(endpoint: string): Promise<void> {
  return invoke<void>("mastersys_set_endpoint", { endpoint });
}

export async function mastersysSetTicketWindow(days: number): Promise<void> {
  return invoke<void>("mastersys_set_ticket_window", { days });
}

/**
 * Catálogo de status espelhado. Só lê o banco local — não toca a rede.
 * Lista vazia é resposta legítima: nunca sincronizou, ou o endpoint falhou.
 */
export async function mastersysStatusCatalog(): Promise<MastersysTicketStatus[]> {
  return invoke<MastersysTicketStatus[]>("mastersys_status_catalog");
}

/**
 * Busca ao vivo no acervo de chamados do Mastersys (mínimo 3 caracteres).
 *
 * O resultado é **consulta**: um chamado que não esteja atribuído a você não
 * pode virar espelho no quadro, porque a sincronização seguinte o apagaria.
 */
export async function mastersysSearchTickets(
  query: string,
): Promise<ExternalWorkItem[]> {
  return invoke<ExternalWorkItem[]>("mastersys_search_tickets", { query });
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
