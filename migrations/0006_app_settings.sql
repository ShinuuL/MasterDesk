-- Configuração local chave/valor (endpoint da integração, preferências que
-- precisam sobreviver a reinstalação do frontend, etc).
--
-- SEGURANÇA (CLAUDE §11/13/18): esta tabela é para configuração **não
-- sensível**. Tokens, senhas e API keys NÃO entram aqui — vão para o cofre do
-- SO via `keyring` (Windows Credential Manager / macOS Keychain / Secret
-- Service), implementado em `crates/infrastructure/src/secret_store.rs`.
-- O banco fica no diretório do usuário sem criptografia; guardar segredo aqui
-- seria equivalente a texto plano.

CREATE TABLE IF NOT EXISTS app_settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL              -- ISO8601 UTC
);
