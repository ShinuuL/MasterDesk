//! `masterdesk-infrastructure` — implementações concretas dos ports do
//! domínio (SQLite via sqlx, plugins do Tauri, etc.).
//!
//! Fase 1: crate vazio, só provando que a dependência compila na direção
//! certa (infrastructure -> domain, nunca o contrário).
//! Fase 2 adiciona `SqliteNoteRepository` real (ADR-003) aqui.

// Módulos reais entram na Fase 2:
// pub mod sqlite;
// pub mod notifications;
