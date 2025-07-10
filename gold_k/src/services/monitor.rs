use crate::models::*;
use crate::services::{DingTalkService, GateService, build_order_data};
use anyhow::{Result, anyhow};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct MonitorService {
    db: SqlitePool,
    is_running: Arc<RwLock<bool>>,
    active_tasks: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
    gate_service: Arc<RwLock<GateService>>,
    dingtalk_service: Arc<RwLock<DingTalkService>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Contract {
    pub order_price_round: String, // 合约价格精度
    pub quanto_multiplier: String, // 合约数量乘数
    pub name: String,              // 合约名称, BTC_USDT
}

impl MonitorService {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            is_running: Arc::new(RwLock::new(false)),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            gate_service: Arc::new(RwLock::new(GateService::new())),
            dingtalk_service: Arc::new(RwLock::new(DingTalkService::new())),
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Err(anyhow!("Monitor is already running"));
        }

        // 获取监控配置
        let configs = self.get_active_configs().await?;
        if configs.is_empty() {
            return Err(anyhow!("No active monitor configurations found"));
        }

        // 获取API配置
        self.update_services().await?;

        *is_running = true;
        drop(is_running); // 释放所有权

        // 为每个配置启动监控任务
        let mut tasks = self.active_tasks.write().await;
        for config in configs {
            let task_handle = self.start_symbol_monitor(config.clone()).await;
            tasks.insert(
                format!("{}_{}", config.symbol, config.interval_type),
                task_handle,
            );
        }

        info!(
            "Monitor service started with {} active configurations",
            tasks.len()
        );
        Ok(())
    }

    pub async fn stop(&mut self) {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return;
        }

        *is_running = false;
        drop(is_running);

        // 停止所有监控任务
        let mut tasks = self.active_tasks.write().await;
        for (symbol, task) in tasks.drain() {
            task.abort();
            debug!("Stopped monitor task for {}", symbol);
        }

        info!("Monitor service stopped");
    }

    pub async fn get_status(&self) -> MonitorStatus {
        let is_running = *self.is_running.read().await;
        let tasks = self.active_tasks.read().await;
        let active_symbols: Vec<String> = tasks.keys().cloned().collect();

        let total_signals = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM signals")
            .fetch_one(&self.db)
            .await
            .unwrap_or(0);

        let total_orders = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM orders")
            .fetch_one(&self.db)
            .await
            .unwrap_or(0);

        let last_check = if is_running {
            Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64,
            )
        } else {
            None
        };

        let api_key =
            sqlx::query_as::<_, ApiKey>("SELECT * FROM api_keys WHERE is_active = 1 LIMIT 1")
                .fetch_one(&self.db)
                .await
                .unwrap_or_default();

        // json反序列化api_key.contracts
        let mut total_contracts = 0;
        if let Some(contracts_str) = &api_key.contracts {
            let contracts: Vec<Contract> = serde_json::from_str(contracts_str).unwrap_or_default();
            total_contracts = contracts.len() as i64;
            debug!(
                "total contracts: {:?}, contracts: {:?}",
                total_contracts, contracts_str
            );
        } else {
            warn!("No contracts found for API Key: {:?}", api_key);
        }

        MonitorStatus {
            is_running,
            active_symbols,
            last_check,
            total_signals,
            total_orders,
            total_contracts,
        }
    }

    async fn get_active_configs(&self) -> Result<Vec<MonitorConfig>> {
        let configs =
            sqlx::query_as::<_, MonitorConfig>("SELECT * FROM monitor_configs WHERE is_active = 1")
                .fetch_all(&self.db)
                .await?;

        Ok(configs)
    }

    async fn update_services(&self) -> Result<()> {
        // 获取活跃的API密钥
        let api_key =
            sqlx::query_as::<_, ApiKey>("SELECT * FROM api_keys WHERE is_active = 1 LIMIT 1")
                .fetch_optional(&self.db)
                .await?;

        if let Some(key) = api_key {
            // 更新Gate服务配置
            {
                let mut gate_service = self.gate_service.write().await;
                gate_service.update_credentials(&key.api_key, &key.secret_key);

                // 更新cookie
                if let Some(cookie) = &key.cookie {
                    gate_service.set_cookie(cookie);
                }

                // 更新合约数据
                if let Some(contracts) = &key.contracts {
                    gate_service.set_contracts(contracts);
                }
            }

            // 更新钉钉服务配置
            if let Some(webhook_url) = &key.webhook_url {
                let mut dingtalk_service = self.dingtalk_service.write().await;
                dingtalk_service.set_webhook_url(webhook_url);
            }

            Ok(())
        } else {
            Err(anyhow!("No active API key found"))
        }
    }

    async fn start_symbol_monitor(&self, config: MonitorConfig) -> tokio::task::JoinHandle<()> {
        let db = self.db.clone();
        let gate_service = self.gate_service.clone();
        let dingtalk_service = self.dingtalk_service.clone();
        let is_running = self.is_running.clone();

        info!("Starting symbol monitor for {}", config.symbol);
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(config.frequency as u64));

            loop {
                interval_timer.tick().await;

                // 检查是否应该继续运行
                if !*is_running.read().await {
                    warn!("Symbol monitor for {} is stopping", config.symbol);
                    break;
                }

                if let Err(e) =
                    Self::check_symbol_signals(&db, &gate_service, &dingtalk_service, &config).await
                {
                    error!("Error checking signals for {}: {}", config.symbol, e);
                }
            }
        })
    }

    async fn check_symbol_signals(
        db: &SqlitePool,
        gate_service: &Arc<RwLock<GateService>>,
        dingtalk_service: &Arc<RwLock<DingTalkService>>,
        config: &MonitorConfig,
    ) -> Result<()> {
        info!(
            "Checking signals for {} on {}",
            config.symbol, config.interval_type
        );

        // 获取K线数据
        let gate = gate_service.read().await;
        let klines = gate
            .get_kline_data(&config.symbol, &config.interval_type, 50, "usdt")
            .await?;
        drop(gate);

        if klines.len() < 5 {
            warn!("Insufficient kline data for {}", config.symbol);
            return Ok(());
        }

        // 分析最新的K线
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let interval_seconds = config.interval_type_to_seconds();
        let last_kline = &klines[klines.len() - 1];

        // 计算这个K线应该结束的时间
        let kline_end_time = last_kline.timestamp + interval_seconds;

        // 使用已收盘的K线数据
        let (latest_kline, historical_klines) = if now < kline_end_time {
            (&klines[klines.len() - 2], &klines[..klines.len() - 2])
        } else {
            (&klines[klines.len() - 1], &klines[..klines.len() - 1])
        };

        // 检查是否满足信号条件
        if let Some(signal) = Self::analyze_kline_signal(latest_kline, historical_klines, config) {
            // 检查是否已经记录过这个信号（防重复）
            let existing_signal = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM signals WHERE symbol = ? AND timestamp = ? AND interval_type = ?"
            )
            .bind(&config.symbol)
            .bind(signal.timestamp)
            .bind(&config.interval_type)
            .fetch_one(db)
            .await?;

            if existing_signal > 0 {
                warn!(
                    "Signal already recorded for {} at {}",
                    config.symbol, signal.timestamp
                );
                return Ok(());
            }

            // 保存信号到数据库
            let signal_id = Self::save_signal(db, &signal).await?;

            info!("New signal detected for {}: {:?}", config.symbol, signal);

            // 发送钉钉通知
            if config.enable_dingtalk {
                let dingtalk = dingtalk_service.read().await;
                if dingtalk.has_webhook() {
                    if let Err(e) = dingtalk.send_signal_alert(&signal).await {
                        error!("Failed to send DingTalk alert: {}", e);
                    }
                }
            }

            // 如果启用自动交易，生成交易信号
            if config.enable_auto_trading {
                if let Some(trading_signal) = Self::generate_trading_signal(&signal, config) {
                    // 下单
                    {
                        let order_data = build_order_data(
                            &trading_signal.symbol,
                            "market", // 使用市价单
                            if trading_signal.signal_type == "long" {
                                "buy"
                            } else {
                                "sell"
                            },
                            trading_signal.entry_price,
                            trading_signal.order_size,
                            Some(trading_signal.take_profit),
                            Some(trading_signal.stop_loss),
                        );

                        let gate_service = gate_service.read().await;
                        if let Err(e) = gate_service
                            .place_order_with_stop_profit_loss(order_data.clone(), "usdt")
                            .await
                        {
                            error!("Failed to place order: {}", e);
                            return Ok(());
                        }
                        info!("Order placed successfully: {:?}", order_data);
                    }

                    // 发送钉钉通知
                    if config.enable_dingtalk {
                        let dingtalk = dingtalk_service.read().await;
                        if dingtalk.has_webhook() {
                            if let Err(e) = dingtalk.send_trading_signal(&trading_signal).await {
                                error!("Failed to send DingTalk alert: {}", e);
                            }
                        }
                    }

                    // 保存订单记录
                    Self::save_order(db, &trading_signal, signal_id).await?;

                    info!("Trading signal generated: {:?}", trading_signal);
                }
            }
        }

        Ok(())
    }

    fn analyze_kline_signal(
        latest: &KlineData,
        historical: &[KlineData],
        config: &MonitorConfig,
    ) -> Option<Signal> {
        // 计算影线和实体长度
        let body_length = (latest.close - latest.open).abs();
        let upper_shadow_length = latest.high - latest.close.max(latest.open);
        let lower_shadow_length = latest.open.min(latest.close) - latest.low;

        // 检查是否有长影线
        let has_long_upper = upper_shadow_length > body_length * config.main_shadow_body_ratio;
        let has_long_lower = lower_shadow_length > body_length * config.main_shadow_body_ratio;

        // 阴线/实体不符合
        if !has_long_upper && !has_long_lower {
            warn!(
                "signal: {} shadow body ratio < {} ",
                config.symbol, config.main_shadow_body_ratio
            );
            return None;
        }

        // 确定主影线类型和长度
        let (shadow_type, main_shadow_length, shadow_ratio) = if has_long_upper && has_long_lower {
            if upper_shadow_length >= lower_shadow_length {
                (
                    "upper",
                    upper_shadow_length,
                    if lower_shadow_length > 0.0 {
                        upper_shadow_length / lower_shadow_length
                    } else {
                        upper_shadow_length * 10000.0 // 当只有一边影线时，放大比例保证通过
                    },
                )
            } else {
                (
                    "lower",
                    lower_shadow_length,
                    if upper_shadow_length > 0.0 {
                        lower_shadow_length / upper_shadow_length
                    } else {
                        lower_shadow_length * 10000.0
                    },
                )
            }
        } else if has_long_upper {
            (
                "upper",
                upper_shadow_length,
                if lower_shadow_length > 0.0 {
                    upper_shadow_length / lower_shadow_length
                } else {
                    upper_shadow_length * 10000.0
                },
            )
        } else {
            (
                "lower",
                lower_shadow_length,
                if upper_shadow_length > 0.0 {
                    lower_shadow_length / upper_shadow_length
                } else {
                    lower_shadow_length * 10000.0
                },
            )
        };

        // 检查影线比例是否满足条件
        if shadow_ratio < config.shadow_ratio {
            warn!("shadow ratio :{} < {} ", shadow_ratio, config.shadow_ratio);
            return None;
        }

        // 获取所需的阴线，通过config.history_hours和config.interval_type来确定需要多少历史数据
        let required_history =
            (config.history_hours * 60.0 / config.interval_type_to_minutes()) as usize;

        if required_history > historical.len() {
            warn!(
                "Not enough historical data, symbol: {}, required: {}, available: {}",
                config.symbol,
                required_history,
                historical.len()
            );
            return None;
        }

        let historical_data =
            &historical[historical.len().saturating_sub(required_history as usize)..];

        // 计算平均成交量
        let avg_volume =
            historical_data.iter().map(|k| k.volume).sum::<f64>() / historical_data.len() as f64;

        let volume_multiplier = latest.volume / avg_volume;

        // 检查成交量是否满足条件
        if volume_multiplier < config.volume_multiplier {
            warn!(
                "volume multiplier :{} < {} ",
                volume_multiplier, config.volume_multiplier
            );
            return None;
        }

        // 确定K线类型
        let candle_type = if latest.close > latest.open {
            "bull"
        } else {
            "bear"
        };

        Some(Signal {
            id: 0, // 将在数据库插入时设置
            symbol: config.symbol.clone(),
            timestamp: latest.timestamp,
            open_price: latest.open,
            high_price: latest.high,
            low_price: latest.low,
            close_price: latest.close,
            volume: latest.volume,
            interval_type: config.interval_type.clone(),
            candle_type: candle_type.to_string(),
            shadow_type: shadow_type.to_string(),
            body_length,
            main_shadow_length,
            shadow_ratio,
            volume_multiplier,
            avg_volume: Some(avg_volume),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        })
    }

    fn generate_trading_signal(signal: &Signal, config: &MonitorConfig) -> Option<TradingSignal> {
        // 根据影线类型确定交易方向
        let signal_type = match signal.shadow_type.as_str() {
            "upper" => "short",
            "lower" => "long",
            _ => {
                warn!("Unknown shadow type: {}", signal.shadow_type);
                return None;
            }
        };

        // 检查配置的交易方向限制
        match config.trade_direction.as_str() {
            "long" if signal_type == "short" => {
                warn!(
                    "Trade direction mismatch: {} vs {}",
                    config.trade_direction, signal_type
                );
                return None;
            }
            "short" if signal_type == "long" => {
                warn!(
                    "Trade direction mismatch: {} vs {}",
                    config.trade_direction, signal_type
                );
                return None;
            }
            _ => {}
        }

        // 计算入场价、止损价和止盈价
        let entry_price = signal.close_price;
        let (stop_loss, take_profit) = if signal_type == "long" {
            let stop_loss = signal.low_price;
            let risk = entry_price - stop_loss;
            let take_profit = entry_price + risk * config.risk_reward_ratio;
            (stop_loss, take_profit)
        } else {
            let stop_loss = signal.high_price;
            let risk = stop_loss - entry_price;
            let take_profit = entry_price - risk * config.risk_reward_ratio;
            (stop_loss, take_profit)
        };

        // 计算信心等级
        let confidence = if signal.shadow_ratio >= 3.0 && signal.volume_multiplier >= 2.0 {
            "high"
        } else if signal.shadow_ratio >= 2.0 && signal.volume_multiplier >= 1.5 {
            "medium"
        } else {
            "low"
        };

        let reason = format!(
            "检测到{}影线信号，影线比例{:.1}:1，成交量倍数{:.1}x",
            if signal_type == "long" { "下" } else { "上" },
            signal.shadow_ratio,
            signal.volume_multiplier
        );

        Some(TradingSignal {
            symbol: signal.symbol.clone(),
            timestamp: signal.timestamp,
            signal_type: signal_type.to_string(),
            order_size: config.order_size,
            entry_price,
            stop_loss,
            take_profit,
            confidence: confidence.to_string(),
            reason,
        })
    }

    async fn save_signal(db: &SqlitePool, signal: &Signal) -> Result<i64> {
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
        .execute(db)
        .await?;

        Ok(result.last_insert_rowid())
    }

    async fn save_order(
        db: &SqlitePool,
        trading_signal: &TradingSignal,
        signal_id: i64,
    ) -> Result<()> {
        let side = if trading_signal.signal_type == "long" {
            "buy"
        } else {
            "sell"
        };

        sqlx::query(
            r#"
            INSERT INTO orders (
                symbol, side, order_size, entry_price, take_profit_price, 
                stop_loss_price, risk_reward_ratio, signal_id, timestamp
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&trading_signal.symbol)
        .bind(side)
        .bind(trading_signal.order_size) // 默认订单大小，应该从配置中获取
        .bind(trading_signal.entry_price)
        .bind(trading_signal.take_profit)
        .bind(trading_signal.stop_loss)
        .bind(
            (trading_signal.take_profit - trading_signal.entry_price).abs()
                / (trading_signal.entry_price - trading_signal.stop_loss).abs(),
        )
        .bind(signal_id)
        .bind(trading_signal.timestamp)
        .execute(db)
        .await?;

        Ok(())
    }
}
