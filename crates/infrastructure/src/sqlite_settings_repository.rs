//! Configuração local chave/valor sobre SQLite (tabela `app_settings`).
//!
//! Guarda **apenas configuração não sensível** — endpoint da integração, id do
//! usuário na origem, preferências. Segredos vão para `SecretStore`
//! (cofre do SO). Ver o comentário da migration `0006_app_settings.sql`.

use chrono::Utc;
use masterdesk_domain::{DomainError, DomainResult};
use sqlx::SqlitePool;

/// Chaves conhecidas. Enum fechado pelo mesmo motivo de `SecretKey`: chave
/// digitada errada é um bug silencioso (lê `None` para sempre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    /// Base URL do Mastersys, ex. `https://suporte.exemplo.com`.
    MastersysBaseUrl,
    /// Id do usuário no Mastersys (inteiro serializado como texto).
    MastersysUserId,
    /// Nome de exibição do usuário no Mastersys, para a UI mostrar quem está
    /// conectado sem precisar de uma chamada de rede.
    MastersysUserName,
    /// E-mail do usuário no Mastersys.
    MastersysUserEmail,
}

impl SettingKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            SettingKey::MastersysBaseUrl => "mastersys.base_url",
            SettingKey::MastersysUserId => "mastersys.user_id",
            SettingKey::MastersysUserName => "mastersys.user_name",
            SettingKey::MastersysUserEmail => "mastersys.user_email",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SqliteSettingsRepository {
    pool: SqlitePool,
}

fn map_sqlx_err(_e: sqlx::Error) -> DomainError {
    DomainError::Persistence
}

impl SqliteSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, key: SettingKey) -> DomainResult<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT value FROM app_settings WHERE key = ?1")
                .bind(key.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(map_sqlx_err)?;
        // Valor vazio é equivalente a ausente — evita "endpoint configurado"
        // com string vazia por causa de um campo de formulário em branco.
        Ok(row.map(|(v,)| v).filter(|v| !v.trim().is_empty()))
    }

    pub async fn set(&self, key: SettingKey, value: &str) -> DomainResult<()> {
        sqlx::query(
            r#"
            INSERT INTO app_settings (key, value, updated_at)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(key.as_str())
        .bind(value.trim())
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_err)?;
        Ok(())
    }

    pub async fn remove(&self, key: SettingKey) -> DomainResult<()> {
        sqlx::query("DELETE FROM app_settings WHERE key = ?1")
            .bind(key.as_str())
            .execute(&self.pool)
            .await
            .map_err(map_sqlx_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fresh() -> SqliteSettingsRepository {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        SqliteSettingsRepository::new(pool)
    }

    #[test]
    fn setting_keys_are_distinct() {
        let all = [
            SettingKey::MastersysBaseUrl,
            SettingKey::MastersysUserId,
            SettingKey::MastersysUserName,
            SettingKey::MastersysUserEmail,
        ];
        let mut keys: Vec<&str> = all.iter().map(|k| k.as_str()).collect();
        keys.sort_unstable();
        let total = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), total);
    }

    #[tokio::test]
    async fn missing_key_is_none() {
        let repo = fresh().await;
        assert_eq!(repo.get(SettingKey::MastersysBaseUrl).await.unwrap(), None);
    }

    #[tokio::test]
    async fn set_get_overwrite_and_remove() {
        let repo = fresh().await;
        repo.set(SettingKey::MastersysBaseUrl, "https://a.exemplo.com")
            .await
            .unwrap();
        assert_eq!(
            repo.get(SettingKey::MastersysBaseUrl)
                .await
                .unwrap()
                .as_deref(),
            Some("https://a.exemplo.com")
        );

        repo.set(SettingKey::MastersysBaseUrl, "https://b.exemplo.com")
            .await
            .unwrap();
        assert_eq!(
            repo.get(SettingKey::MastersysBaseUrl)
                .await
                .unwrap()
                .as_deref(),
            Some("https://b.exemplo.com")
        );

        repo.remove(SettingKey::MastersysBaseUrl).await.unwrap();
        assert_eq!(repo.get(SettingKey::MastersysBaseUrl).await.unwrap(), None);
    }

    #[tokio::test]
    async fn blank_value_reads_back_as_absent() {
        let repo = fresh().await;
        repo.set(SettingKey::MastersysUserId, "   ").await.unwrap();
        assert_eq!(
            repo.get(SettingKey::MastersysUserId).await.unwrap(),
            None,
            "string vazia não pode passar por configuração válida"
        );
    }

    #[tokio::test]
    async fn values_are_trimmed_on_write() {
        let repo = fresh().await;
        repo.set(SettingKey::MastersysUserId, "  42\n")
            .await
            .unwrap();
        assert_eq!(
            repo.get(SettingKey::MastersysUserId)
                .await
                .unwrap()
                .as_deref(),
            Some("42")
        );
    }
}
