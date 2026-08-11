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

    #[error("persistence failure")]
    Persistence,

    #[error("integration not configured")]
    IntegrationNotConfigured,

    #[error("unauthorized")]
    Unauthorized,
}

pub type DomainResult<T> = Result<T, DomainError>;
