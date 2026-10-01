-- "Manter conectado" da conta local.
--
-- O segredo da sessão lembrada mora no cofre do SO (Credential Manager no
-- Windows), no formato `<id>.<segredo>`. Aqui fica só o hash Argon2 do
-- segredo: quem copiar o banco não leva uma sessão utilizável.
--
-- `expires_at` é deslizante — renovado a cada restauração — e a linha some
-- em `logout`, ao expirar, ou junto com o usuário (CASCADE; exige
-- `foreign_keys(true)`, já ligado em `src-tauri/src/lib.rs`).

CREATE TABLE IF NOT EXISTS remembered_sessions (
    id          TEXT PRIMARY KEY,                -- UUID v4, parte pública do token
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    secret_hash TEXT NOT NULL,                   -- Argon2id (PHC) do segredo
    created_at  TEXT NOT NULL,                   -- ISO8601 UTC
    expires_at  TEXT NOT NULL                    -- ISO8601 UTC
);

CREATE INDEX IF NOT EXISTS idx_remembered_sessions_user ON remembered_sessions(user_id);
