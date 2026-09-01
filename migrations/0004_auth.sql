-- Fase 4 — Schema de autenticação local (ADR-005)
-- SQLite via sqlx. Domínio permanece agnóstico ao armazenamento.
--
-- Segurança (CLAUDE §11/18):
-- - `password_hash` armazena apenas o hash Argon2id (PHC string) — NUNCA plaintext.
-- - `username` é UNIQUE com NOCASE: login/registro são case-insensitive para
--   evitar duplicatas por diferença de caixa ("alice" vs "Alice").
-- - CHECK valida o tamanho do username em nível de banco (defesa em profundidade;
--   a validação de conteúdo alfabético/underscore ocorre no domínio).

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,          -- UUID v4
    username      TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(username) >= 3 AND length(username) <= 32),
    password_hash TEXT NOT NULL,             -- hash Argon2id — nunca plaintext
    created_at    TEXT NOT NULL              -- ISO8601 UTC
);

-- Índice para buscas case-insensitive de login (consulta usa COLLATE NOCASE).
CREATE INDEX IF NOT EXISTS idx_users_username ON users(username COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS idx_users_created_at ON users(created_at);
