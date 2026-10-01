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
    /// "Manter conectado": a sessão sobrevive ao fechamento do app.
    pub remember: bool,
}

/// Entrada para login local.
#[derive(Debug, Clone)]
pub struct LoginInput {
    pub username: String,
    pub password: String,
    /// "Manter conectado": a sessão sobrevive ao fechamento do app.
    pub remember: bool,
}

/// Resultado de autenticação exposto à UI — **nunca** inclui `password_hash`
/// (seção 11/18 do CLAUDE.md: minimizar exposição de segredos).
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub user: UserView,
    /// Se a sessão ficou lembrada. `false` com `remember = true` significa que
    /// o cofre do SO não estava disponível — a UI avisa em vez de fingir.
    pub remembered: bool,
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
        self.finish(&user, input.remember).await
    }

    /// Autentica um usuário e abre a sessão.
    pub async fn login(&self, input: LoginInput) -> DomainResult<AuthResult> {
        let user = self
            .provider
            .login(&input.username, &input.password)
            .await?;
        self.finish(&user, input.remember).await
    }

    /// Reabre a sessão lembrada, se houver. Chamado na abertura do app.
    pub async fn restore_session(&self) -> DomainResult<Option<AuthResult>> {
        Ok(self
            .provider
            .restore_session()
            .await?
            .map(|user| AuthResult {
                user: to_view(&user),
                remembered: true,
            }))
    }

    /// Sem `remember`, apaga qualquer sessão lembrada antes: quem desmarca a
    /// caixa num login está pedindo para esta máquina esquecê-lo, e deixar a
    /// sessão anterior valendo contrariaria isso.
    async fn finish(&self, user: &User, remember: bool) -> DomainResult<AuthResult> {
        let remembered = if remember {
            self.provider.remember_session().await?
        } else {
            self.provider.forget_remembered_session().await?;
            false
        };
        Ok(AuthResult {
            user: to_view(user),
            remembered,
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
        /// Sessão lembrada — sobrevive a `logout` só se ninguém a apagar.
        remembered: Mutex<Option<UserId>>,
    }

    impl InMemoryAuthProvider {
        fn new() -> Self {
            Self {
                users: Mutex::new(HashMap::new()),
                session: Mutex::new(None),
                remembered: Mutex::new(None),
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
                _ => Err(DomainError::unauthorized("usuário ou senha incorretos")),
            }
        }

        async fn logout(&self) -> DomainResult<()> {
            *self.session.lock().unwrap() = None;
            *self.remembered.lock().unwrap() = None;
            Ok(())
        }

        async fn is_authenticated(&self) -> DomainResult<bool> {
            Ok(self.session.lock().unwrap().is_some())
        }

        async fn remember_session(&self) -> DomainResult<bool> {
            let current = *self.session.lock().unwrap();
            let id = current.ok_or_else(|| DomainError::unauthorized("sem sessão"))?;
            *self.remembered.lock().unwrap() = Some(id);
            Ok(true)
        }

        async fn restore_session(&self) -> DomainResult<Option<User>> {
            let Some(id) = *self.remembered.lock().unwrap() else {
                return Ok(None);
            };
            let users = self.users.lock().unwrap();
            let user = users.values().map(|(u, _)| u).find(|u| u.id == id).cloned();
            if let Some(u) = &user {
                *self.session.lock().unwrap() = Some(u.id);
            }
            Ok(user)
        }

        async fn forget_remembered_session(&self) -> DomainResult<()> {
            *self.remembered.lock().unwrap() = None;
            Ok(())
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
                remember: false,
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
            remember: false,
        })
        .await
        .unwrap();
        svc.logout().await.unwrap();

        // login correto
        let res = svc
            .login(LoginInput {
                username: "bob".into(),
                password: "password123".into(),
                remember: false,
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
                remember: false,
            })
            .await;
        assert!(matches!(err, Err(DomainError::Unauthorized(_))));
    }

    #[tokio::test]
    async fn register_duplicate_conflict() {
        let svc = AuthService::new(provider());
        svc.register(CreateUserInput {
            username: "carol".into(),
            password: "password123".into(),
            remember: false,
        })
        .await
        .unwrap();
        let dup = svc
            .register(CreateUserInput {
                username: "carol".into(),
                password: "anotherpass".into(),
                remember: false,
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
                remember: false,
            })
            .await
            .unwrap();
        // UserView só tem id/username/created_at — sem campo password_hash.
        assert_eq!(res.user.username, "dave");
        let now = Utc::now();
        assert!(res.user.created_at <= now);
    }

    #[tokio::test]
    async fn remember_then_restore_reopens_the_session() {
        let svc = AuthService::new(provider());
        let res = svc
            .register(CreateUserInput {
                username: "rita".into(),
                password: "password123".into(),
                remember: true,
            })
            .await
            .unwrap();
        assert!(res.remembered);

        let restored = svc.restore_session().await.unwrap().expect("lembrada");
        assert_eq!(restored.user.id, res.user.id);
        assert!(restored.remembered);
    }

    #[tokio::test]
    async fn login_without_remember_forgets_a_previous_remembered_session() {
        let svc = AuthService::new(provider());
        svc.register(CreateUserInput {
            username: "saulo".into(),
            password: "password123".into(),
            remember: true,
        })
        .await
        .unwrap();

        let res = svc
            .login(LoginInput {
                username: "saulo".into(),
                password: "password123".into(),
                remember: false,
            })
            .await
            .unwrap();
        assert!(!res.remembered);
        assert!(svc.is_authenticated().await.unwrap(), "o login atual vale");
        assert!(svc.restore_session().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn logout_clears_the_remembered_session() {
        let svc = AuthService::new(provider());
        svc.register(CreateUserInput {
            username: "tais".into(),
            password: "password123".into(),
            remember: true,
        })
        .await
        .unwrap();
        svc.logout().await.unwrap();
        assert!(svc.restore_session().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn validation_bubbles_before_provider() {
        let svc = AuthService::new(provider());
        // username curto
        let err = svc
            .register(CreateUserInput {
                username: "ab".into(),
                password: "password123".into(),
                remember: false,
            })
            .await;
        assert!(matches!(err, Err(DomainError::Validation(_))));
        // senha curta
        let err = svc
            .register(CreateUserInput {
                username: "validuser".into(),
                password: "short".into(),
                remember: false,
            })
            .await;
        assert!(matches!(err, Err(DomainError::Validation(_))));
    }
}
