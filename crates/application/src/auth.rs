//! Casos de uso de Autenticação (Fase 4).
//! Orquestra o port `AuthenticationProvider` + validação de domínio.
//! Nenhuma lógica de hashing mora aqui — isso é responsabilidade da
//! infraestrutura (Argon2). O domínio valida formato; a aplicação coordena.

use std::sync::Arc;

use masterdesk_domain::{
    ports::AuthenticationProvider, validate_password, validate_username, DomainResult, User,
};

/// Entrada para criação de conta local.
#[derive(Debug, Clone)]
pub struct CreateUserInput {
    pub username: String,
    pub password: String,
}

/// Entrada para login local.
#[derive(Debug, Clone)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

/// Resultado de autenticação exposto à UI — **nunca** inclui `password_hash`
/// (seção 11/18 do CLAUDE.md: minimizar exposição de segredos).
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub user: UserView,
}

/// Visão pública de um usuário, sem o hash de senha.
#[derive(Debug, Clone)]
pub struct UserView {
    pub id: uuid::Uuid,
    pub username: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl AuthService {
    /// Registra um usuário local e abre a sessão.
    pub async fn register(&self, input: CreateUserInput) -> DomainResult<AuthResult> {
        // Validação de domínio redundante (defesa em profundidade); o provedor
        // também valida, mas antecipamos aqui para erro consistente.
        validate_username(&input.username)?;
        validate_password(&input.password)?;
        let user = self
            .provider
            .register(&input.username, &input.password)
            .await?;
        Ok(AuthResult {
            user: to_view(&user),
        })
    }

    /// Autentica um usuário e abre a sessão.
    pub async fn login(&self, input: LoginInput) -> DomainResult<AuthResult> {
        let user = self
            .provider
            .login(&input.username, &input.password)
            .await?;
        Ok(AuthResult {
            user: to_view(&user),
        })
    }

    /// Encerra a sessão atual (não-falha se não houver sessão).
    pub async fn logout(&self) -> DomainResult<()> {
        self.provider.logout().await
    }

    /// Consulta se há sessão autenticada ativa.
    pub async fn is_authenticated(&self) -> DomainResult<bool> {
        self.provider.is_authenticated().await
    }
}

fn to_view(user: &User) -> UserView {
    // Jamais vaza `password_hash` para a UI.
    UserView {
        id: user.id,
        username: user.username.clone(),
        created_at: user.created_at,
    }
}

/// Service de autenticação. Mantém apenas o port — a sessão em si vive na
/// implementação concreta (`LocalAuthRepository`), não aqui.
pub struct AuthService {
    provider: Arc<dyn AuthenticationProvider>,
}

impl AuthService {
    pub fn new(provider: Arc<dyn AuthenticationProvider>) -> Self {
        Self { provider }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::Utc;
    use masterdesk_domain::{DomainError, User, UserId};
    use std::collections::HashMap;
    use std::sync::Mutex;

    // -----------------------------------------------------------------------
    // In-memory AuthProvider para testar o AuthService isolado de SQLite/Argon2
    // -----------------------------------------------------------------------

    struct InMemoryAuthProvider {
        users: Mutex<HashMap<String, (User, String)>>, // username -> (user, plaintext p/ teste)
        session: Mutex<Option<UserId>>,
    }

    impl InMemoryAuthProvider {
        fn new() -> Self {
            Self {
                users: Mutex::new(HashMap::new()),
                session: Mutex::new(None),
            }
        }
    }

    fn make_user(username: &str) -> User {
        User::new(username, "not-a-real-hash").unwrap()
    }

    #[async_trait]
    impl AuthenticationProvider for InMemoryAuthProvider {
        async fn register(&self, username: &str, password: &str) -> DomainResult<User> {
            let mut users = self.users.lock().unwrap();
            let key = username.trim().to_lowercase();
            if users.contains_key(&key) {
                return Err(DomainError::Conflict("username already exists".into()));
            }
            let mut user = make_user(username);
            let stored = password.to_string();
            user.password_hash = stored.clone();
            *self.session.lock().unwrap() = Some(user.id);
            users.insert(key, (user.clone(), stored));
            Ok(user)
        }

        async fn login(&self, username: &str, password: &str) -> DomainResult<User> {
            let users = self.users.lock().unwrap();
            let key = username.trim().to_lowercase();
            match users.get(&key) {
                Some((user, stored)) if stored == password => {
                    *self.session.lock().unwrap() = Some(user.id);
                    Ok(user.clone())
                }
                _ => Err(DomainError::Unauthorized),
            }
        }

        async fn logout(&self) -> DomainResult<()> {
            *self.session.lock().unwrap() = None;
            Ok(())
        }

        async fn is_authenticated(&self) -> DomainResult<bool> {
            Ok(self.session.lock().unwrap().is_some())
        }
    }

    fn provider() -> Arc<dyn AuthenticationProvider> {
        Arc::new(InMemoryAuthProvider::new())
    }

    #[tokio::test]
    async fn register_and_check_auth() {
        let svc = AuthService::new(provider());
        let res = svc
            .register(CreateUserInput {
                username: "alice".into(),
                password: "superSecret1".into(),
            })
            .await
            .unwrap();
        assert_eq!(res.user.username, "alice");
        assert!(svc.is_authenticated().await.unwrap());
    }

    #[tokio::test]
    async fn login_success_and_fail() {
        let svc = AuthService::new(provider());
        svc.register(CreateUserInput {
            username: "bob".into(),
            password: "password123".into(),
        })
        .await
        .unwrap();
        svc.logout().await.unwrap();

        // login correto
        let res = svc
            .login(LoginInput {
                username: "bob".into(),
                password: "password123".into(),
            })
            .await
            .unwrap();
        assert_eq!(res.user.username, "bob");
        assert!(svc.is_authenticated().await.unwrap());

        // senha errada
        svc.logout().await.unwrap();
        let err = svc
            .login(LoginInput {
                username: "bob".into(),
                password: "wrong".into(),
            })
            .await;
        assert!(matches!(err, Err(DomainError::Unauthorized)));
    }

    #[tokio::test]
    async fn register_duplicate_conflict() {
        let svc = AuthService::new(provider());
        svc.register(CreateUserInput {
            username: "carol".into(),
            password: "password123".into(),
        })
        .await
        .unwrap();
        let dup = svc
            .register(CreateUserInput {
                username: "carol".into(),
                password: "anotherpass".into(),
            })
            .await;
        assert!(matches!(dup, Err(DomainError::Conflict(_))));
    }

    #[tokio::test]
    async fn auth_result_never_exposes_password_hash() {
        let svc = AuthService::new(provider());
        let res = svc
            .register(CreateUserInput {
                username: "dave".into(),
                password: "password123".into(),
            })
            .await
            .unwrap();
        // UserView só tem id/username/created_at — sem campo password_hash.
        assert_eq!(res.user.username, "dave");
        let now = Utc::now();
        assert!(res.user.created_at <= now);
    }

    #[tokio::test]
    async fn validation_bubbles_before_provider() {
        let svc = AuthService::new(provider());
        // username curto
        let err = svc
            .register(CreateUserInput {
                username: "ab".into(),
                password: "password123".into(),
            })
            .await;
        assert!(matches!(err, Err(DomainError::Validation(_))));
        // senha curta
        let err = svc
            .register(CreateUserInput {
                username: "validuser".into(),
                password: "short".into(),
            })
            .await;
        assert!(matches!(err, Err(DomainError::Validation(_))));
    }
}
