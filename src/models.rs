use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: i64,
    pub name: String,
    pub api_key: String,
    pub secret_key: String,
    pub webhook_url: Option<String>,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Signal {
    pub id: i64,
    pub symbol: String,
    pub timestamp: i64,
    pub open_price: f64,
    pub high_price: f64,
    pub low_price: f64,
    pub close_price: f64,
    pub volume: f64,
    pub interval_type: String,
    pub candle_type: String, // 'bull' or 'bear'
    pub shadow_type: String, // 'upper' or 'lower'
    pub body_length: f64,
    pub main_shadow_length: f64,
    pub shadow_ratio: f64,
    pub volume_multiplier: f64,
    pub avg_volume: Option<f64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: i64,
    pub symbol: String,
    pub side: String, // 'buy' or 'sell'
    pub order_size: f64,
    pub entry_price: f64,
    pub take_profit_price: f64,
    pub stop_loss_price: f64,
    pub risk_reward_ratio: f64,
    pub signal_id: Option<i64>,
    pub timestamp: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MonitorConfig {
    pub id: Option<i64>,
    pub symbol: String,
    pub interval_type: String,
    pub frequency: i64,
    pub history_hours: i64,
    pub shadow_ratio: f64,
    pub main_shadow_body_ratio: f64,
    pub volume_multiplier: f64,
    pub order_size: f64,
    pub risk_reward_ratio: f64,
    pub enable_auto_trading: bool,
    pub enable_dingtalk: bool,
    pub trade_direction: String, // 'both', 'long', 'short'
    pub is_active: bool,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KlineData {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorStatus {
    pub is_running: bool,
    pub active_symbols: Vec<String>,
    pub last_check: Option<i64>,
    pub total_signals: i64,
    pub total_orders: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: String,
    pub timestamp: i64,
    pub signal_type: String, // 'long' or 'short'
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub confidence: String, // 'high', 'medium', 'low'
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkMessage {
    pub msgtype: String,
    pub text: Option<DingTalkText>,
    pub markdown: Option<DingTalkMarkdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkText {
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkMarkdown {
    pub title: String,
    pub text: String,
}
