//! `masterdesk-application` — orquestra casos de uso sobre os ports do
//! domínio. Casos de uso reais (Notes/Tasks) entram na Fase 2/3.
//! Este crate existe na Fase 1 só para fixar o padrão de injeção de
//! dependência via `Arc<dyn Trait>`.

use std::sync::Arc;

use masterdesk_domain::ports::NoteRepository;

/// Exemplo de padrão de wiring — NÃO é a implementação final do caso de uso
/// de notas (isso é Fase 2). Serve para o time validar o padrão de injeção
/// de dependência antes de escalar para todos os casos de uso.
pub struct NoteService {
    repository: Arc<dyn NoteRepository>,
}

impl NoteService {
    pub fn new(repository: Arc<dyn NoteRepository>) -> Self {
        Self { repository }
    }

    pub async fn list_active_notes(&self) -> masterdesk_domain::DomainResult<Vec<masterdesk_domain::Note>> {
        self.repository.list_active().await
    }
}
