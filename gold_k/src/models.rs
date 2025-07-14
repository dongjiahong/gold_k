use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Default, Serialize, Deserialize, FromRow)]
pub struct ApiKey {
    pub id: i64,
    pub name: String,
    pub api_key: String,
    pub secret_key: String,
    pub webhook_url: Option<String>, // 钉钉的webhook url
    pub cookie: Option<String>,      // 浏览器cookie 方便调用gate的v2接口
    pub contracts: Option<String>,   // 存放合约数据
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Contract {
    pub order_price_round: String, // 合约价格精度
    pub quanto_multiplier: String, // 合约数量乘数
    pub name: String,              // 合约名称, BTC_USDT
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
    #[sqlx(skip)]
    pub main_profit: f64,
    pub shadow_ratio: f64,
    pub volume_multiplier: f64,
    pub avg_volume: Option<f64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Order {
    pub id: i64,
    pub symbol: String,
    pub side: String,    // 'buy' or 'sell'
    pub order_size: i64, // 合约数量张
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
    pub interval_type: String, // k线类型，1m、5m、15m、30m、1h、4h、1d
    pub frequency: i64,        // 监控间隔时间
    pub history_hours: f64,    // 历史成交量数据回溯时
    pub shadow_ratio: f64,     // 影线占比
    pub main_shadow_body_ratio: f64, // 主影线与实体占比
    pub volume_multiplier: f64, // 成交量倍数
    pub order_size: i64,       // 张
    pub risk_reward_ratio: f64, // 风险收益比
    pub enable_auto_trading: bool,
    pub enable_dingtalk: bool,
    pub long_k_long: bool,       // 阳K才做多
    pub short_k_short: bool,     // 阴K才做空
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
    pub total_contracts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingSignal {
    pub symbol: String,
    pub timestamp: i64,
    pub signal_type: String, // 'long' or 'short'
    pub entry_price: f64,
    pub order_size: i64, // 张
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

impl MonitorConfig {
    pub fn interval_type_to_minutes(&self) -> f64 {
        match self.interval_type.as_str() {
            "1m" => 1.0,
            "3m" => 3.0,
            "5m" => 5.0,
            "15m" => 15.0,
            "30m" => 30.0,
            "1h" => 60.0,
            "4h" => 240.0,
            "1d" => 1440.0,
            _ => 0.0,
        }
    }

    pub fn interval_type_to_seconds(&self) -> i64 {
        (self.interval_type_to_minutes() * 60.0) as i64
    }
}
