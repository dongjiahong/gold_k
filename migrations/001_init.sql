-- 信号记录表
CREATE TABLE IF NOT EXISTS signals (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    open_price REAL NOT NULL,
    high_price REAL NOT NULL,
    low_price REAL NOT NULL,
    close_price REAL NOT NULL,
    volume REAL NOT NULL,
    interval_type TEXT NOT NULL,
    candle_type TEXT NOT NULL, -- 'bull' or 'bear'
    shadow_type TEXT NOT NULL, -- 'upper' or 'lower'
    body_length REAL NOT NULL,
    main_shadow_length REAL NOT NULL,
    shadow_ratio REAL NOT NULL,
    volume_multiplier REAL NOT NULL,
    avg_volume REAL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- 订单记录表
CREATE TABLE IF NOT EXISTS orders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    side TEXT NOT NULL, -- 'buy' or 'sell'
    order_size REAL NOT NULL,
    entry_price REAL NOT NULL,
    take_profit_price REAL NOT NULL,
    stop_loss_price REAL NOT NULL,
    risk_reward_ratio REAL NOT NULL,
    signal_id INTEGER,
    timestamp INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    FOREIGN KEY (signal_id) REFERENCES signals(id)
);

-- API密钥配置表
CREATE TABLE IF NOT EXISTS api_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    api_key TEXT NOT NULL,
    secret_key TEXT NOT NULL,
    webhook_url TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- 监控配置表
CREATE TABLE IF NOT EXISTS monitor_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol TEXT NOT NULL,
    interval_type TEXT NOT NULL, -- '1m', '5m', '15m', etc.
    frequency INTEGER NOT NULL, -- 检查频率（秒）
    history_hours INTEGER NOT NULL DEFAULT 3, -- 历史数据小时数
    shadow_ratio REAL NOT NULL DEFAULT 2.0, -- 影线比例阈值
    main_shadow_body_ratio REAL NOT NULL DEFAULT 1.0, -- 主影线与实体比例
    volume_multiplier REAL NOT NULL DEFAULT 1.5, -- 成交量倍数阈值
    order_size REAL NOT NULL DEFAULT 1.0, -- 下单数量
    risk_reward_ratio REAL NOT NULL DEFAULT 1.2, -- 风险收益比
    enable_auto_trading BOOLEAN NOT NULL DEFAULT 0, -- 是否启用自动交易
    enable_dingtalk BOOLEAN NOT NULL DEFAULT 0, -- 是否启用钉钉通知
    trade_direction TEXT NOT NULL DEFAULT 'both', -- 'both', 'long', 'short'
    is_active BOOLEAN NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_signals_symbol_timestamp ON signals(symbol, timestamp);
CREATE INDEX IF NOT EXISTS idx_orders_symbol_timestamp ON orders(symbol, timestamp);
CREATE INDEX IF NOT EXISTS idx_monitor_configs_symbol ON monitor_configs(symbol);
