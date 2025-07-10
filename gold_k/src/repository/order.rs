use anyhow::Result;
use sqlx::SqlitePool;
use crate::models::{Order, TradingSignal};

pub struct OrderRepository;

impl OrderRepository {
    /// 获取最近的订单，限制数量
    pub async fn get_recent(pool: &SqlitePool, limit: i64) -> Result<Vec<Order>> {
        let orders = sqlx::query_as::<_, Order>(
            "SELECT * FROM orders ORDER BY timestamp DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(pool)
        .await?;
        Ok(orders)
    }

    /// 获取订单总数
    pub async fn count(pool: &SqlitePool) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM orders")
            .fetch_one(pool)
            .await?;
        Ok(count)
    }

    /// 保存交易信号生成的订单
    pub async fn save_from_trading_signal(
        pool: &SqlitePool,
        trading_signal: &TradingSignal,
        signal_id: i64,
    ) -> Result<i64> {
        let side = if trading_signal.signal_type == "long" {
            "buy"
        } else {
            "sell"
        };

        let result = sqlx::query(
            r#"
            INSERT INTO orders (
                symbol, side, order_size, entry_price, take_profit_price, 
                stop_loss_price, risk_reward_ratio, signal_id, timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&trading_signal.symbol)
        .bind(side)
        .bind(trading_signal.order_size)
        .bind(trading_signal.entry_price)
        .bind(trading_signal.take_profit)
        .bind(trading_signal.stop_loss)
        .bind(
            (trading_signal.take_profit - trading_signal.entry_price).abs()
                / (trading_signal.entry_price - trading_signal.stop_loss).abs(),
        )
        .bind(signal_id)
        .bind(trading_signal.timestamp)
        .execute(pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// 保存订单
    pub async fn save(
        pool: &SqlitePool,
        symbol: &str,
        side: &str,
        order_size: i64,
        entry_price: f64,
        take_profit_price: f64,
        stop_loss_price: f64,
        risk_reward_ratio: f64,
        signal_id: Option<i64>,
        timestamp: i64,
    ) -> Result<i64> {
        let result = sqlx::query(
            r#"
            INSERT INTO orders (
                symbol, side, order_size, entry_price, take_profit_price, 
                stop_loss_price, risk_reward_ratio, signal_id, timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(symbol)
        .bind(side)
        .bind(order_size)
        .bind(entry_price)
        .bind(take_profit_price)
        .bind(stop_loss_price)
        .bind(risk_reward_ratio)
        .bind(signal_id)
        .bind(timestamp)
        .execute(pool)
        .await?;

        Ok(result.last_insert_rowid())
    }
}
