use crate::models::MonitorConfig;
use anyhow::Result;
use sqlx::SqlitePool;

pub struct MonitorConfigRepository;

impl MonitorConfigRepository {
    /// 获取所有监控配置，按创建时间降序排列
    pub async fn get_all(pool: &SqlitePool) -> Result<Vec<MonitorConfig>> {
        let configs = sqlx::query_as::<_, MonitorConfig>(
            "SELECT * FROM monitor_configs ORDER BY created_at DESC",
        )
        .fetch_all(pool)
        .await?;
        Ok(configs)
    }

    /// 获取所有激活的监控配置
    pub async fn get_active(pool: &SqlitePool) -> Result<Vec<MonitorConfig>> {
        let configs =
            sqlx::query_as::<_, MonitorConfig>("SELECT * FROM monitor_configs WHERE is_active = 1")
                .fetch_all(pool)
                .await?;
        Ok(configs)
    }

    /// 删除所有监控配置
    pub async fn delete_all(pool: &SqlitePool) -> Result<()> {
        sqlx::query("DELETE FROM monitor_configs")
            .execute(pool)
            .await?;
        Ok(())
    }

    /// 批量保存监控配置（在事务中执行）
    pub async fn save_batch(pool: &SqlitePool, configs: &[MonitorConfig]) -> Result<()> {
        let mut tx = pool.begin().await?;

        // 清空现有配置
        sqlx::query("DELETE FROM monitor_configs")
            .execute(&mut *tx)
            .await?;

        // 插入新配置
        for config in configs {
            sqlx::query(
                r#"
                INSERT INTO monitor_configs (
                    symbol, interval_type, frequency, history_hours, shadow_ratio,
                    main_shadow_body_ratio, volume_multiplier, order_size,
                    risk_reward_ratio, enable_auto_trading, enable_dingtalk,
                    long_k_long, short_k_short, trade_direction, is_active,
                    order_type, expected_profit_rate
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&config.symbol)
            .bind(&config.interval_type)
            .bind(config.frequency)
            .bind(config.history_hours)
            .bind(config.shadow_ratio)
            .bind(config.main_shadow_body_ratio)
            .bind(config.volume_multiplier)
            .bind(config.order_size)
            .bind(config.risk_reward_ratio)
            .bind(config.enable_auto_trading)
            .bind(config.enable_dingtalk)
            .bind(config.long_k_long)
            .bind(config.short_k_short)
            .bind(&config.trade_direction)
            .bind(config.is_active)
            .bind(&config.order_type)
            .bind(config.expected_profit_rate)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
