use crate::models::Signal;
use anyhow::Result;
use sqlx::SqlitePool;

pub struct SignalRepository;

impl SignalRepository {
    /// 获取最近的信号，限制数量
    pub async fn get_recent(pool: &SqlitePool, limit: i64) -> Result<Vec<Signal>> {
        let signals =
            sqlx::query_as::<_, Signal>("SELECT * FROM signals ORDER BY timestamp DESC LIMIT ?")
                .bind(limit)
                .fetch_all(pool)
                .await?;
        Ok(signals)
    }

    /// 获取信号总数
    pub async fn count(pool: &SqlitePool) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM signals")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }

    /// 检查指定条件的信号是否已存在
    pub async fn exists(
        pool: &SqlitePool,
        symbol: &str,
        timestamp: i64,
        interval_type: &str,
    ) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM signals WHERE symbol = ? AND timestamp = ? AND interval_type = ?",
        )
        .bind(symbol)
        .bind(timestamp)
        .bind(interval_type)
        .fetch_one(pool)
        .await?;
        Ok(count > 0)
    }

    /// 保存信号到数据库
    pub async fn save(pool: &SqlitePool, signal: &Signal) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO signals (
                symbol, timestamp, open_price, high_price, low_price, close_price, 
                volume, interval_type, candle_type, shadow_type, body_length, 
                main_shadow_length, shadow_ratio, volume_multiplier, avg_volume
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&signal.symbol)
        .bind(signal.timestamp)
        .bind(signal.open_price)
        .bind(signal.high_price)
        .bind(signal.low_price)
        .bind(signal.close_price)
        .bind(signal.volume)
        .bind(&signal.interval_type)
        .bind(&signal.candle_type)
        .bind(&signal.shadow_type)
        .bind(signal.body_length)
        .bind(signal.main_shadow_length)
        .bind(signal.shadow_ratio)
        .bind(signal.volume_multiplier)
        .bind(signal.avg_volume)
        .execute(pool)
        .await?;

        Ok(result.last_insert_rowid())
    }
}
