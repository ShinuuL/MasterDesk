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
