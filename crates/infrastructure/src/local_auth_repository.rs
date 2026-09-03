//! `LocalAuthRepository` — implementação local e isolada do port
//! `AuthenticationProvider` (Fase 4). Usa SQLite via sqlx (ADR-003) para
//! persistir usuários e Argon2 (rustcrypto `argon2`) para hashear senhas.
//!
//! Segurança (CLAUDE §11/18):
//! - Senha **nunca** é armazenada em plaintext; apenas o hash Argon2 é gravado.
//! - Senha **nunca** é logada. Erros são mapeados para `DomainError` sem expor
//!   detalhes sensíveis.
//! - A sessão vive **em memória** (`Mutex<Option<UserId>>`); não há token
//!   persistido em disco.
//!
//! Arquitetura: este crate (infrastructure) é o único autorizado a usar
//! sqlx/argon2. O domínio/application permanecem agnósticos.

use std::sync::Mutex;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use masterdesk_domain::{
    normalize_username, ports::AuthenticationProvider, validate_password, validate_username,
    DomainError, DomainResult, User, UserId,
};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Sessão em memória: guarda o `UserId` do usuário autenticado, se houver.
/// Não persiste em disco — o app pede login a cada inicialização.
#[derive(Debug)]
struct Session {
    user_id: Option<UserId>,
}

/// Repositório de autenticação local.
#[derive(Debug)]
pub struct LocalAuthRepository {
    pool: SqlitePool,
    session: Mutex<Session>,
}

impl LocalAuthRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            session: Mutex::new(Session { user_id: None }),
        }
    }

    /// Garante o schema `users` (usado como fallback inline quando a migração
    /// `0004_auth.sql` não rodou). A UNIQUE usa COLLATE NOCASE: login/registro
    /// são case-insensitive no nível do banco (defesa em profundidade).
    pub async fn ensure_schema(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id            TEXT PRIMARY KEY,
                username      TEXT NOT NULL COLLATE NOCASE UNIQUE CHECK (length(username) >= 3 AND length(username) <= 32),
                password_hash TEXT NOT NULL,
                created_at    TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    fn set_session(&self, user_id: UserId) {
        let mut s = self.session.lock().unwrap();
        s.user_id = Some(user_id);
    }

    fn clear_session(&self) {
        let mut s = self.session.lock().unwrap();
        s.user_id = None;
    }

    fn current_user_id(&self) -> Option<UserId> {
        self.session.lock().unwrap().user_id
    }
}

// ---------------------------------------------------------------------------
// Helpers de hashing (Argon2) — infraestrutura, nunca expostos ao domínio
// ---------------------------------------------------------------------------

/// Hasheia uma senha com Argon2id + salt aleatório (formato PHC string).
/// Retorna erro de validação se a senha não atender ao mínimo de domínio
/// (a regra de tamanho mora em `domain`, o hashing mora aqui).
fn hash_password(password: &str) -> DomainResult<String> {
    validate_password(password)?;
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| DomainError::Persistence)
}

/// Verifica uma senha contra um hash Argon2 armazenado. Não lança erro de
/// detalhe sobre qual parte falhou — retorna `Unauthorized` em qualquer falha
/// de verificação (evita oráculos de timing/erro, seção 18 do CLAUDE.md).
fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parsed = match PasswordHash::new(stored_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

// ---------------------------------------------------------------------------
// DB <-> Domain mapping
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct UserRow {
    id: String,
    username: String,
    password_hash: String,
    created_at: String,
}

fn row_to_user(row: UserRow) -> DomainResult<User> {
    let id = Uuid::parse_str(&row.id).map_err(|_| DomainError::Persistence)?;
    let created_at = row
        .created_at
        .parse::<DateTime<Utc>>()
        .map_err(|_| DomainError::Persistence)?;
    User::reconstitute(id, row.username, row.password_hash, created_at)
}

/// Uma frase só para "não existe" e "senha errada".
///
/// Distinguir as duas ajudaria quem está tentando descobrir que contas existem
/// nesta máquina, e não ajuda quem esqueceu a senha.
const WRONG_CREDENTIALS: &str = "usuário ou senha incorretos";

fn map_sqlx_err(_e: sqlx::Error) -> DomainError {
    // Nunca vazar detalhes de SQL / credenciais para o domínio / UI.
    DomainError::Persistence
}

// ---------------------------------------------------------------------------
// Port implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl AuthenticationProvider for LocalAuthRepository {
    async fn register(&self, username: &str, password: &str) -> DomainResult<User> {
        // Validação de domínio (formato username / força mínima da senha).
        validate_username(username)?;
        validate_password(password)?;

        // `normalize_username` e não `trim`: colapsa espaços internos também,
        // senão "ana  paula" e "ana paula" viram duas contas e quem se
        // cadastrou com a primeira não entra digitando a segunda.
        let username = normalize_username(username);

        // Duplicata → Conflict
        let existing: Option<String> =
            sqlx::query_scalar("SELECT username FROM users WHERE username = ?1 COLLATE NOCASE")
                .bind(&username)
                .fetch_optional(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
        if existing.is_some() {
            return Err(DomainError::Conflict(
                "já existe uma conta com esse nome nesta máquina".into(),
            ));
        }

        // Hash da senha (nunca plaintext no banco).
        let hash = hash_password(password)?;
        let user = User::new(username, hash)?;
        let now_str = user.created_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO users (id, username, password_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(user.id.to_string())
        .bind(&user.username)
        .bind(&user.password_hash)
        .bind(now_str)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        // Abre sessão automaticamente após o registro bem-sucedido.
        self.set_session(user.id);
        Ok(user)
    }

    async fn login(&self, username: &str, password: &str) -> DomainResult<User> {
        let username = normalize_username(username);

        let row: Option<UserRow> = sqlx::query_as(
            "SELECT id, username, password_hash, created_at FROM users WHERE username = ?1 COLLATE NOCASE",
        )
        .bind(&username)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        let row = match row {
            Some(r) => r,
            // Mesma mensagem de "senha errada", de propósito: dizer "usuário
            // não existe" revelaria quais contas existem nesta máquina.
            None => return Err(DomainError::unauthorized(WRONG_CREDENTIALS)),
        };

        // Verifica a senha contra o hash armazenado. Falha (user inexistente OU
        // senha errada) retorna o mesmo `Unauthorized` — sem vazar qual parte falhou.
        if !verify_password(password, &row.password_hash) {
            return Err(DomainError::unauthorized(WRONG_CREDENTIALS));
        }

        let user = row_to_user(row)?;
        self.set_session(user.id);
        Ok(user)
    }

    async fn logout(&self) -> DomainResult<()> {
        self.clear_session();
        Ok(())
    }

    async fn is_authenticated(&self) -> DomainResult<bool> {
        Ok(self.current_user_id().is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn fresh_repo() -> LocalAuthRepository {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let repo = LocalAuthRepository::new(pool.clone());
        repo.ensure_schema().await.unwrap();
        repo
    }

    #[tokio::test]
    async fn hash_and_verify_roundtrip() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert_ne!(hash, "correct horse battery staple"); // nunca plaintext
        assert!(verify_password("correct horse battery staple", &hash));
        assert!(!verify_password("wrong password", &hash));
        // hashes devem variar (sal aleatório)
        let h2 = hash_password("correct horse battery staple").unwrap();
        assert_ne!(hash, h2);
    }

    #[tokio::test]
    async fn register_and_login_flow() {
        let repo = fresh_repo().await;
        assert!(!repo.is_authenticated().await.unwrap());

        let u = repo.register("alice", "superSecret1").await.unwrap();
        assert_eq!(u.username, "alice");
        // após registrar, sessão aberta
        assert!(repo.is_authenticated().await.unwrap());

        // logout fecha sessão
        repo.logout().await.unwrap();
        assert!(!repo.is_authenticated().await.unwrap());

        // login com credenciais corretas
        let u2 = repo.login("alice", "superSecret1").await.unwrap();
        assert_eq!(u2.id, u.id);
        assert!(repo.is_authenticated().await.unwrap());
    }

    #[tokio::test]
    async fn login_fail_unauthorized() {
        let repo = fresh_repo().await;
        repo.register("bob", "password123").await.unwrap();
        repo.logout().await.unwrap();

        // senha errada
        assert!(matches!(
            repo.login("bob", "wrongpassword").await,
            Err(DomainError::Unauthorized(_))
        ));
        // usuário inexistente
        assert!(matches!(
            repo.login("ghost", "password123").await,
            Err(DomainError::Unauthorized(_))
        ));
    }

    #[tokio::test]
    async fn register_duplicate_conflict() {
        let repo = fresh_repo().await;
        repo.register("carol", "password123").await.unwrap();
        let dup = repo.register("carol", "anotherpass").await;
        assert!(matches!(dup, Err(DomainError::Conflict(_))));
        // o primeiro usuário permanece válido
        assert!(repo.is_authenticated().await.unwrap());
    }

    #[tokio::test]
    async fn register_validation_bubbles() {
        let repo = fresh_repo().await;
        // username inválido
        assert!(matches!(
            repo.register("ab", "password123").await,
            Err(DomainError::Validation(_))
        ));
        // senha curta
        assert!(matches!(
            repo.register("dave", "short").await,
            Err(DomainError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn no_plaintext_stored() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let repo = LocalAuthRepository::new(pool.clone());
        repo.ensure_schema().await.unwrap();
        repo.register("erin", "aVeryLongPass1").await.unwrap();

        let stored: String =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE username = 'erin'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_ne!(stored, "aVeryLongPass1");
        assert!(stored.starts_with("$argon2"));
    }

    #[tokio::test]
    async fn username_case_insensitive_unique() {
        let repo = fresh_repo().await;
        repo.register("Frank", "password123").await.unwrap();
        assert!(matches!(
            repo.register("frank", "password456").await,
            Err(DomainError::Conflict(_))
        ));
    }
}
