//! `LocalAuthRepository` — implementação local e isolada do port
//! `AuthenticationProvider` (Fase 4). Usa SQLite via sqlx (ADR-003) para
//! persistir usuários e Argon2 (rustcrypto `argon2`) para hashear senhas.
//!
//! Segurança (CLAUDE §11/18):
//! - Senha **nunca** é armazenada em plaintext; apenas o hash Argon2 é gravado.
//! - Senha **nunca** é logada. Erros são mapeados para `DomainError` sem expor
//!   detalhes sensíveis.
//! - A sessão vive **em memória** (`Mutex<Option<UserId>>`). Só com "manter
//!   conectado" ela sobrevive ao fechamento do app: o segredo vai para o cofre
//!   do SO e o banco guarda apenas o hash dele (ver `remember_session`).
//!
//! Arquitetura: este crate (infrastructure) é o único autorizado a usar
//! sqlx/argon2. O domínio/application permanecem agnósticos.

use std::sync::Mutex;

use argon2::{
    password_hash::{
        rand_core::{OsRng, RngCore},
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
    },
    Argon2,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use masterdesk_domain::{
    normalize_username, ports::AuthenticationProvider, validate_password, validate_username,
    DomainError, DomainResult, User, UserId,
};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::secret_store::{SecretKey, SecretStore, SecretStoreError};

/// Por quanto tempo uma sessão lembrada vale **sem uso**. Deslizante: cada
/// abertura do app renova o prazo, então só expira quem fica um mês sem abrir.
const REMEMBER_FOR_DAYS: i64 = 30;

/// 256 bits do `OsRng` — o segredo não é senha escolhida por gente, então não
/// há dicionário a atacar; o hash no banco existe para quem copiar o `.db`
/// não levar junto uma sessão utilizável.
const SESSION_SECRET_BYTES: usize = 32;

/// Sessão em memória: guarda o `UserId` do usuário autenticado, se houver.
#[derive(Debug)]
struct Session {
    user_id: Option<UserId>,
}

/// O único lugar do cofre que a sessão lembrada usa. Existe como trait para os
/// testes não tocarem no Credential Manager de quem roda `cargo test`.
trait SessionVault: Send + Sync + std::fmt::Debug {
    fn load(&self) -> Result<Option<String>, SecretStoreError>;
    fn store(&self, value: &str) -> Result<(), SecretStoreError>;
    fn delete(&self) -> Result<(), SecretStoreError>;
}

impl SessionVault for SecretStore {
    fn load(&self) -> Result<Option<String>, SecretStoreError> {
        SecretStore::load(self, SecretKey::LocalSessionToken)
    }
    fn store(&self, value: &str) -> Result<(), SecretStoreError> {
        SecretStore::store(self, SecretKey::LocalSessionToken, value)
    }
    fn delete(&self) -> Result<(), SecretStoreError> {
        SecretStore::delete(self, SecretKey::LocalSessionToken)
    }
}

/// Repositório de autenticação local.
#[derive(Debug)]
pub struct LocalAuthRepository {
    pool: SqlitePool,
    session: Mutex<Session>,
    vault: Box<dyn SessionVault>,
}

impl LocalAuthRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self::with_vault(pool, Box::new(SecretStore::new()))
    }

    fn with_vault(pool: SqlitePool, vault: Box<dyn SessionVault>) -> Self {
        Self {
            pool,
            session: Mutex::new(Session { user_id: None }),
            vault,
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
// Sessão lembrada ("manter conectado")
// ---------------------------------------------------------------------------

fn random_session_secret() -> String {
    let mut bytes = [0u8; SESSION_SECRET_BYTES];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Não passa por `hash_password` porque aquele aplica a política de senha de
/// gente (tamanho), que não se aplica a um segredo gerado aqui.
fn hash_session_secret(secret: &str) -> DomainResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| DomainError::Persistence)
}

/// `<uuid>.<64 hex>`. Qualquer outra forma é tratada como token corrompido.
fn parse_session_token(token: &str) -> Option<(Uuid, &str)> {
    let (id, secret) = token.split_once('.')?;
    let id = Uuid::parse_str(id).ok()?;
    let well_formed =
        secret.len() == SESSION_SECRET_BYTES * 2 && secret.bytes().all(|b| b.is_ascii_hexdigit());
    well_formed.then_some((id, secret))
}

#[derive(Debug, sqlx::FromRow)]
struct RememberedRow {
    id: String,
    username: String,
    password_hash: String,
    created_at: String,
    secret_hash: String,
    expires_at: String,
}

impl LocalAuthRepository {
    /// Revoga a sessão lembrada. O banco é a revogação que vale: sem a linha,
    /// o que sobrar no cofre não autentica ninguém. Por isso o erro do banco
    /// sobe, e o do cofre não impede o logout.
    async fn forget_remembered(&self) -> DomainResult<()> {
        sqlx::query("DELETE FROM remembered_sessions")
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        let _ = self.vault.delete();
        Ok(())
    }
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
        self.forget_remembered().await
    }

    async fn is_authenticated(&self) -> DomainResult<bool> {
        Ok(self.current_user_id().is_some())
    }

    async fn remember_session(&self) -> DomainResult<bool> {
        let user_id = self
            .current_user_id()
            .ok_or_else(|| DomainError::unauthorized("nenhuma sessão aberta para lembrar"))?;

        let id = Uuid::new_v4();
        let secret = random_session_secret();
        let secret_hash = hash_session_secret(&secret)?;
        let now = Utc::now();

        // Um só lugar no cofre → uma só sessão lembrada por usuário do SO.
        // Apagar as anteriores evita linha órfã que nenhum token alcança.
        let mut tx = self.pool.begin().await.map_err(map_sqlx_err)?;
        sqlx::query("DELETE FROM remembered_sessions")
            .execute(&mut *tx)
            .await
            .map_err(map_sqlx_err)?;
        sqlx::query(
            "INSERT INTO remembered_sessions (id, user_id, secret_hash, created_at, expires_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(id.to_string())
        .bind(user_id.to_string())
        .bind(&secret_hash)
        .bind(now.to_rfc3339())
        .bind((now + Duration::days(REMEMBER_FOR_DAYS)).to_rfc3339())
        .execute(&mut *tx)
        .await
        .map_err(map_sqlx_err)?;
        tx.commit().await.map_err(map_sqlx_err)?;

        if self.vault.store(&format!("{id}.{secret}")).is_err() {
            // Sem cofre não há onde guardar o segredo com segurança — e texto
            // plano em disco está fora de questão (CLAUDE §11). A linha sem
            // token seria lixo; o login em si continua valendo.
            sqlx::query("DELETE FROM remembered_sessions WHERE id = ?1")
                .bind(id.to_string())
                .execute(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
            return Ok(false);
        }
        Ok(true)
    }

    async fn restore_session(&self) -> DomainResult<Option<User>> {
        // Cofre indisponível equivale a "nada lembrado": o usuário vê a tela
        // de login, que é exatamente o comportamento de antes deste recurso.
        let token = match self.vault.load() {
            Ok(Some(t)) => t,
            Ok(None) | Err(_) => return Ok(None),
        };
        let Some((id, secret)) = parse_session_token(&token) else {
            self.forget_remembered().await?;
            return Ok(None);
        };

        let row: Option<RememberedRow> = sqlx::query_as(
            "SELECT u.id, u.username, u.password_hash, u.created_at, s.secret_hash, s.expires_at \
             FROM remembered_sessions s JOIN users u ON u.id = s.user_id WHERE s.id = ?1",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(map_sqlx_err)?;

        let Some(row) = row else {
            self.forget_remembered().await?;
            return Ok(None);
        };

        let now = Utc::now();
        let expired = row
            .expires_at
            .parse::<DateTime<Utc>>()
            .map_or(true, |exp| exp <= now);
        if expired || !verify_password(secret, &row.secret_hash) {
            self.forget_remembered().await?;
            return Ok(None);
        }

        sqlx::query("UPDATE remembered_sessions SET expires_at = ?1 WHERE id = ?2")
            .bind((now + Duration::days(REMEMBER_FOR_DAYS)).to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;

        let user = row_to_user(UserRow {
            id: row.id,
            username: row.username,
            password_hash: row.password_hash,
            created_at: row.created_at,
        })?;
        self.set_session(user.id);
        Ok(Some(user))
    }

    async fn forget_remembered_session(&self) -> DomainResult<()> {
        self.forget_remembered().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    /// Cofre em memória. `unavailable = true` imita o Linux sem Secret Service.
    #[derive(Debug, Default)]
    struct MemVault {
        value: Mutex<Option<String>>,
        unavailable: bool,
    }

    impl SessionVault for std::sync::Arc<MemVault> {
        fn load(&self) -> Result<Option<String>, SecretStoreError> {
            if self.unavailable {
                return Err(SecretStoreError::Unavailable);
            }
            Ok(self.value.lock().unwrap().clone())
        }
        fn store(&self, value: &str) -> Result<(), SecretStoreError> {
            if self.unavailable {
                return Err(SecretStoreError::Unavailable);
            }
            *self.value.lock().unwrap() = Some(value.to_string());
            Ok(())
        }
        fn delete(&self) -> Result<(), SecretStoreError> {
            *self.value.lock().unwrap() = None;
            Ok(())
        }
    }

    /// Nunca usa o cofre real: `cargo test` não pode deslogar o dev.
    async fn fresh_repo() -> LocalAuthRepository {
        let pool = migrated_pool().await;
        let repo = repo_on(&pool, &std::sync::Arc::new(MemVault::default()));
        repo.ensure_schema().await.unwrap();
        repo
    }

    /// Pool com o schema real (inclui `remembered_sessions`) e um cofre
    /// compartilhado, para simular "fechar e reabrir o app" criando um
    /// segundo repositório sobre o mesmo banco e o mesmo cofre.
    async fn migrated_pool() -> SqlitePool {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        // Uma conexão só: cada conexão `:memory:` seria um banco diferente.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .in_memory(true)
                    .foreign_keys(true),
            )
            .await
            .unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        pool
    }

    fn repo_on(pool: &SqlitePool, vault: &std::sync::Arc<MemVault>) -> LocalAuthRepository {
        LocalAuthRepository::with_vault(pool.clone(), Box::new(vault.clone()))
    }

    #[tokio::test]
    async fn remembered_session_survives_restart() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());

        let first = repo_on(&pool, &vault);
        let u = first.register("gina", "password123").await.unwrap();
        assert!(first.remember_session().await.unwrap());

        // "Reabre o app": repositório novo, sessão em memória vazia.
        let second = repo_on(&pool, &vault);
        assert!(!second.is_authenticated().await.unwrap());
        let restored = second
            .restore_session()
            .await
            .unwrap()
            .expect("sessão lembrada");
        assert_eq!(restored.id, u.id);
        assert!(second.is_authenticated().await.unwrap());
    }

    #[tokio::test]
    async fn vault_holds_token_but_db_holds_only_a_hash() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());
        let repo = repo_on(&pool, &vault);
        repo.register("hugo", "password123").await.unwrap();
        repo.remember_session().await.unwrap();

        let token = vault.value.lock().unwrap().clone().unwrap();
        let (_, secret) = parse_session_token(&token).expect("token bem formado");
        let stored: String = sqlx::query_scalar("SELECT secret_hash FROM remembered_sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(stored.starts_with("$argon2"));
        assert!(!stored.contains(secret));
    }

    #[tokio::test]
    async fn logout_forgets_remembered_session() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());
        let repo = repo_on(&pool, &vault);
        repo.register("iris", "password123").await.unwrap();
        repo.remember_session().await.unwrap();

        repo.logout().await.unwrap();
        assert!(vault.value.lock().unwrap().is_none());
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM remembered_sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0);
        assert!(repo_on(&pool, &vault)
            .restore_session()
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn db_revocation_wins_over_a_leftover_vault_token() {
        // O cofre falhou ao apagar (ou alguém restaurou um backup dele): sem a
        // linha no banco, o token que sobrou não pode autenticar.
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());
        let repo = repo_on(&pool, &vault);
        repo.register("joao", "password123").await.unwrap();
        repo.remember_session().await.unwrap();
        sqlx::query("DELETE FROM remembered_sessions")
            .execute(&pool)
            .await
            .unwrap();

        assert!(repo_on(&pool, &vault)
            .restore_session()
            .await
            .unwrap()
            .is_none());
        assert!(
            vault.value.lock().unwrap().is_none(),
            "token órfão deve ser limpo"
        );
    }

    #[tokio::test]
    async fn expired_remembered_session_is_rejected_and_cleared() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());
        let repo = repo_on(&pool, &vault);
        repo.register("kara", "password123").await.unwrap();
        repo.remember_session().await.unwrap();
        let past = (Utc::now() - Duration::minutes(1)).to_rfc3339();
        sqlx::query("UPDATE remembered_sessions SET expires_at = ?1")
            .bind(past)
            .execute(&pool)
            .await
            .unwrap();

        assert!(repo_on(&pool, &vault)
            .restore_session()
            .await
            .unwrap()
            .is_none());
        assert!(vault.value.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn tampered_secret_is_rejected() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());
        let repo = repo_on(&pool, &vault);
        repo.register("lara", "password123").await.unwrap();
        repo.remember_session().await.unwrap();

        let token = vault.value.lock().unwrap().clone().unwrap();
        let (id, _) = token.split_once('.').unwrap();
        *vault.value.lock().unwrap() = Some(format!("{id}.{}", "0".repeat(64)));
        assert!(repo_on(&pool, &vault)
            .restore_session()
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn restore_slides_the_expiry_forward() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());
        let repo = repo_on(&pool, &vault);
        repo.register("mila", "password123").await.unwrap();
        repo.remember_session().await.unwrap();
        let soon = (Utc::now() + Duration::days(1)).to_rfc3339();
        sqlx::query("UPDATE remembered_sessions SET expires_at = ?1")
            .bind(&soon)
            .execute(&pool)
            .await
            .unwrap();

        repo_on(&pool, &vault)
            .restore_session()
            .await
            .unwrap()
            .unwrap();
        let exp: String = sqlx::query_scalar("SELECT expires_at FROM remembered_sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        let exp = exp.parse::<DateTime<Utc>>().unwrap();
        assert!(exp > Utc::now() + Duration::days(REMEMBER_FOR_DAYS - 1));
    }

    #[tokio::test]
    async fn unavailable_vault_means_not_remembered_but_still_logged_in() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault {
            unavailable: true,
            ..Default::default()
        });
        let repo = repo_on(&pool, &vault);
        repo.register("nina", "password123").await.unwrap();

        assert!(!repo.remember_session().await.unwrap());
        assert!(repo.is_authenticated().await.unwrap());
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM remembered_sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 0, "sem cofre não deve sobrar linha no banco");
        assert!(repo.restore_session().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn remember_requires_an_open_session() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());
        assert!(matches!(
            repo_on(&pool, &vault).remember_session().await,
            Err(DomainError::Unauthorized(_))
        ));
    }

    #[tokio::test]
    async fn deleting_the_user_cascades_to_remembered_sessions() {
        let pool = migrated_pool().await;
        let vault = std::sync::Arc::new(MemVault::default());
        let repo = repo_on(&pool, &vault);
        let u = repo.register("olga", "password123").await.unwrap();
        repo.remember_session().await.unwrap();
        sqlx::query("DELETE FROM users WHERE id = ?1")
            .bind(u.id.to_string())
            .execute(&pool)
            .await
            .unwrap();
        assert!(repo_on(&pool, &vault)
            .restore_session()
            .await
            .unwrap()
            .is_none());
    }

    #[test]
    fn session_token_parsing_rejects_malformed_values() {
        let ok = format!("{}.{}", Uuid::new_v4(), "a".repeat(64));
        assert!(parse_session_token(&ok).is_some());
        assert!(parse_session_token("").is_none());
        assert!(parse_session_token("sem-ponto").is_none());
        assert!(parse_session_token(&format!("nao-uuid.{}", "a".repeat(64))).is_none());
        assert!(parse_session_token(&format!("{}.{}", Uuid::new_v4(), "a".repeat(63))).is_none());
        assert!(parse_session_token(&format!("{}.{}", Uuid::new_v4(), "z".repeat(64))).is_none());
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
        let pool = migrated_pool().await;
        let repo = repo_on(&pool, &std::sync::Arc::new(MemVault::default()));
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
