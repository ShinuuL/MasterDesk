use thiserror::Error;

/// Erro de domínio. Implementações de infraestrutura (sqlx, tauri, http)
/// devem mapear seus próprios erros para este tipo na borda do adapter —
/// nunca vazar `sqlx::Error` ou similar para cima do `domain` (seção 17 e 18
/// do CLAUDE.md: erros com contexto útil, sem vazar detalhes sensíveis).
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("not found")]
    NotFound,

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("persistence failure")]
    Persistence,

    #[error("integration not configured")]
    IntegrationNotConfigured,

    /// Falha ao falar com um sistema externo (rede, timeout, resposta
    /// inesperada). A mensagem é feita para ser mostrada ao usuário, então o
    /// adapter deve escrevê-la sem URL de token, header ou corpo de resposta
    /// (seção 13/18 do CLAUDE.md: nunca logar/vazar segredo).
    #[error("integration failure: {0}")]
    Integration(String),

    /// Ação recusada por credencial ou sessão.
    ///
    /// Carrega o motivo **já em português e voltado ao usuário**, porque
    /// "unauthorized" cobria situações que exigem ações opostas: senha errada
    /// (corrigir e tentar de novo), sessão expirada (reconectar), conta inativa
    /// (falar com o administrador) e falta de permissão (não há o que tentar).
    /// Uma palavra só para todas elas deixava o usuário sem saber o que fazer.
    ///
    /// Quem levanta escreve a frase; o `Display` a devolve como está, sem
    /// prefixo, porque ela vai direto para a tela.
    #[error("{0}")]
    Unauthorized(String),
}

impl DomainError {
    /// Atalho para os casos que não têm motivo mais específico a oferecer.
    pub fn unauthorized(reason: impl Into<String>) -> Self {
        DomainError::Unauthorized(reason.into())
    }
}

pub type DomainResult<T> = Result<T, DomainError>;
