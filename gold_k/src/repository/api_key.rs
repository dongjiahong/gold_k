use crate::models::{ApiKey, Contract};
use anyhow::Result;
use sqlx::SqlitePool;

pub struct ApiKeyRepository;

impl ApiKeyRepository {
    /// 获取所有API密钥，按创建时间降序排列
    pub async fn get_all(pool: &SqlitePool) -> Result<Vec<ApiKey>> {
        let keys = sqlx::query_as::<_, ApiKey>("SELECT * FROM api_keys ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?;
        Ok(keys)
    }

    /// 获取当前激活的API密钥
    pub async fn get_active(pool: &SqlitePool) -> Result<Option<ApiKey>> {
        let key = sqlx::query_as::<_, ApiKey>("SELECT * FROM api_keys WHERE is_active = 1 LIMIT 1")
            .fetch_optional(pool)
            .await?;
        Ok(key)
    }

    /// 根据ID获取API密钥
    pub async fn get_by_id(pool: &SqlitePool, id: i64) -> Result<Option<ApiKey>> {
        let key = sqlx::query_as::<_, ApiKey>("SELECT * FROM api_keys WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(key)
    }

    /// 根据指定币获取配置
    pub async fn get_contract_by_symbol(
        pool: &SqlitePool,
        symbol: &str,
    ) -> Result<Option<Contract>> {
        // 1. 获取contracts
        let key = ApiKeyRepository::get_active(pool).await?;

        if key.is_none() {
            return Ok(None);
        }
        let contracts_str = key.unwrap().contracts;
        if contracts_str.is_none() {
            return Ok(None);
        }

        // 2. 反序列化
        let contracts: Vec<Contract> = serde_json::from_str(contracts_str.unwrap().as_str())?;
        Ok(contracts.into_iter().find(|c| c.name == symbol))
    }

    /// 删除所有API密钥
    pub async fn delete_all(pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM api_keys").execute(pool).await?;
        Ok(())
    }

    /// 保存新的API密钥
    pub async fn save(
        pool: &SqlitePool,
        name: &str,
        api_key: &str,
        secret_key: &str,
        webhook_url: Option<&str>,
        cookie: Option<&str>,
    ) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO api_keys (name, api_key, secret_key, webhook_url, cookie, is_active)
            VALUES (?, ?, ?, ?, ?, 1)
            "#,
        )
        .bind(name)
        .bind(api_key)
        .bind(secret_key)
        .bind(webhook_url)
        .bind(cookie)
        .execute(pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// 将所有API密钥设置为非激活状态
    pub async fn deactivate_all(pool: &SqlitePool) -> Result<()> {
        sqlx::query("UPDATE api_keys SET is_active = 0")
            .execute(pool)
            .await?;
        Ok(())
    }

    /// 激活指定ID的API密钥
    pub async fn activate(pool: &SqlitePool, id: i64) -> Result<()> {
        sqlx::query("UPDATE api_keys SET is_active = 1 WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// 删除指定ID的API密钥
    pub async fn delete_by_id(pool: &SqlitePool, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM api_keys WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// 更新API密钥的合约数据
    pub async fn update_contracts(pool: &SqlitePool, id: i64, contracts: &str) -> Result<()> {
        sqlx::query("UPDATE api_keys SET contracts = ? WHERE id = ?")
            .bind(contracts)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
