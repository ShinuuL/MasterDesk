import { invoke } from "@tauri-apps/api/core";
import type {
  Note,
  CreateNotePayload,
  UpdateNotePayload,
  Task,
  CreateTaskPayload,
  UpdateTaskPayload,
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
