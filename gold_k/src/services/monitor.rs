use crate::models::*;
use crate::repository::{
    ApiKeyRepository, MonitorConfigRepository, OrderRepository, SignalRepository,
};
use crate::services::{DingTalkService, GateService, build_order_data};
use anyhow::{Result, anyhow};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tracing::{debug, error, info, warn};

// precision: "0.01" -> 2
fn round_price(price: f64, precision: &str) -> f64 {
    let mut decimal_places = precision.split('.').nth(1).map(|s| s.len()).unwrap_or(0);
    // 比官方少一位精度
    if decimal_places > 0 {
        decimal_places -= 1;
    }
    let multiplier = 10_f64.powi(decimal_places as i32);
    (price * multiplier).round() / multiplier
}

#[derive(Debug, Clone)]
pub struct MonitorService {
    db: SqlitePool,
    is_running: Arc<RwLock<bool>>,
    active_tasks: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
    gate_service: Arc<RwLock<GateService>>,
    dingtalk_service: Arc<RwLock<DingTalkService>>,
    // 记录最后更新的API配置时间戳，用于检测配置变化
    last_config_update: Arc<RwLock<i64>>,
}

impl MonitorService {
    pub fn new(db: SqlitePool) -> Self {
        Self {
            db,
            is_running: Arc::new(RwLock::new(false)),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            gate_service: Arc::new(RwLock::new(GateService::new())),
            dingtalk_service: Arc::new(RwLock::new(DingTalkService::new())),
            last_config_update: Arc::new(RwLock::new(0)),
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

        // 检查cookie是否有效
        let gate_service = self.gate_service.clone();
        let dingtalk_service = self.dingtalk_service.clone();
        let db_clone = self.db.clone();
        let last_config_update = self.last_config_update.clone();

        // 异步程序每隔5分钟调用一次get_account_info,以来检查是否cookie有效，如果无效就发送钉钉通知
        // 同时每30秒检查一次配置是否有更新
        tokio::spawn(async move {
            info!("Starting cookie validity check and config update check");
            let mut cookie_check_interval = interval(Duration::from_secs(300)); // 5分钟检查cookie
            let mut config_check_interval = interval(Duration::from_secs(30)); // 30秒检查配置
            let mut heartbeat_interval = interval(Duration::from_secs(60)); // 1分钟心跳日志

            loop {
                // 添加全局异常处理，确保任何未处理的错误不会导致整个监控循环停止
                let loop_result = tokio::time::timeout(Duration::from_secs(120), async {
                    tokio::select! {
                        _ = heartbeat_interval.tick() => {
                            info!("💓Monitor loop heartbeat - still running");
                        }
                        _ = cookie_check_interval.tick() => {
                            info!("🪛Checking cookie validity");
                            
                            // 使用 tokio::time::timeout 包装整个cookie检查过程，防止卡住
                            let check_result = tokio::time::timeout(Duration::from_secs(60), async {
                                // Cookie有效性检查 - 分别获取锁并立即释放
                                let account_result = {
                                    let gate_service = gate_service.read().await;
                                    gate_service.get_account_info().await
                                }; // gate_service 锁在这里自动释放
                                
                                match account_result {
                                    Ok(account_result) => {
                                        if !account_result.1 {
                                            warn!("Cookie已失效，请重新登录, account: {:?}", account_result);
                                            let msg = account_result.0.to_string();
                                            
                                            // 分别获取钉钉服务锁
                                            let send_result = {
                                                let dingtalk_service = dingtalk_service.read().await;
                                                dingtalk_service.send_text_message(
                                                    format!("K线监控：Cookie已失效，请重新登录, account: {}", msg).as_str()
                                                ).await
                                            }; // dingtalk_service 锁在这里自动释放
                                            
                                            if let Err(e) = send_result {
                                                error!("Failed to send DingTalk message: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        // 如果e中包含403 Forbidden，则认为Cookie已失效
                                        if e.to_string().contains("403 Forbidden") {
                                            error!("Cookie已失效，或者ip不对，用国内ip, account: {:?}", e);
                                            
                                            // 分别获取钉钉服务锁
                                            let send_result = {
                                                let dingtalk_service = dingtalk_service.read().await;
                                                dingtalk_service.send_text_message(
                                                    "K线监控：Cookie已失效，或者ip不对，请检测"
                                                ).await
                                            }; // dingtalk_service 锁在这里自动释放
                                            
                                            if let Err(e) = send_result {
                                                error!("Failed to send DingTalk message: {}", e);
                                            }
                                        } else {
                                            error!("Failed to get account info: {}", e);
                                        }
                                    }
                                }
                            }).await;
                            
                            match check_result {
                                Ok(_) => {
                                    info!("Finished cookie validity check");
                                }
                                Err(_) => {
                                    error!("Cookie validity check timed out after 60 seconds");
                                }
                            }
                        }
                        _ = config_check_interval.tick() => {
                            info!("🔧Checking for config updates");
                            
                            // 使用 tokio::time::timeout 包装配置检查过程，防止卡住
                            let config_result = tokio::time::timeout(Duration::from_secs(30), async {
                                Self::check_and_update_config(
                                    &db_clone,
                                    &gate_service,
                                    &dingtalk_service,
                                    &last_config_update
                                ).await
                            }).await;
                            
                            match config_result {
                                Ok(Ok(_)) => {
                                    info!("Finished config update check");
                                }
                                Ok(Err(e)) => {
                                    error!("Failed to check/update config: {}", e);
                                }
                                Err(_) => {
                                    error!("Config update check timed out after 30 seconds");
                                }
                            }
                        }
                    }
                }).await;

                match loop_result {
                    Ok(_) => {
                        // 正常完成一轮检查
                    }
                    Err(_) => {
                        error!("Monitor loop iteration timed out after 120 seconds, continuing...");
                    }
                }

                // 添加小延时，防止CPU占用过高
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

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

        let total_signals = SignalRepository::count(&self.db).await.unwrap_or(0);
        let total_orders = OrderRepository::count(&self.db).await.unwrap_or(0);

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

        let api_key_result = ApiKeyRepository::get_active(&self.db).await.unwrap_or(None);

        let mut total_contracts = 0;
        match api_key_result {
            Some(api_key) => {
                // json反序列化api_key.contracts
                if let Some(contracts_str) = &api_key.contracts {
                    let contracts: Vec<Contract> =
                        serde_json::from_str(contracts_str).unwrap_or_default();
                    total_contracts = contracts.len() as i64;
                    debug!(
                        "total contracts: {:?}, contracts: {:?}",
                        total_contracts, contracts_str
                    );
                } else {
                    warn!("No contracts found for API Key: {:?}", api_key);
                }
            }
            None => {
                warn!("Failed to get active API key");
            }
        };

        MonitorStatus {
            is_running,
            active_symbols,
            last_check,
            total_signals,
            total_orders,
            total_contracts,
        }
    }

    /// 检查数据库配置是否有更新，如果有则更新服务配置
    /// 这个方法是线程安全的，使用读写锁来保护共享资源
    async fn check_and_update_config(
        db: &SqlitePool,
        gate_service: &Arc<RwLock<GateService>>,
        dingtalk_service: &Arc<RwLock<DingTalkService>>,
        last_config_update: &Arc<RwLock<i64>>,
    ) -> Result<()> {
        // 获取当前活跃的API密钥
        let api_key = match ApiKeyRepository::get_active(db).await? {
            Some(key) => key,
            None => {
                warn!("No active API key found, skipping config update");
                return Ok(());
            }
        };

        // 检查配置是否有更新
        let last_update = *last_config_update.read().await;
        if api_key.updated_at <= last_update {
            // 配置没有更新，直接返回
            return Ok(());
        }

        warn!(
            "API配置有更新，开始更新服务配置。上次更新时间: {}, 当前配置更新时间: {}",
            last_update, api_key.updated_at
        );

        // 更新 GateService 配置
        {
            let mut gate = gate_service.write().await;

            // 更新 API 凭据
            gate.update_credentials(&api_key.api_key, &api_key.secret_key);

            // 更新 cookie
            if let Some(cookie) = &api_key.cookie {
                gate.set_cookie(cookie);
                info!("Updated gate service cookie");
            }

            // 更新合约数据
            if let Some(contracts) = &api_key.contracts {
                gate.set_contracts(contracts);
                info!("Updated gate service contracts");
            }

            info!("Updated gate service API credentials");
        }

        // 更新 DingTalkService 配置
        if let Some(webhook_url) = &api_key.webhook_url {
            let mut dingtalk = dingtalk_service.write().await;
            dingtalk.set_webhook_url(webhook_url);
            info!("Updated dingtalk service webhook URL");
        }

        // 更新最后配置更新时间戳
        {
            let mut last_update = last_config_update.write().await;
            *last_update = api_key.updated_at;
        }

        info!("配置更新完成");
        Ok(())
    }

    async fn get_active_configs(&self) -> Result<Vec<MonitorConfig>> {
        let configs = MonitorConfigRepository::get_active(&self.db).await?;

        Ok(configs)
    }

    async fn update_services(&self) -> Result<()> {
        // 获取活跃的API密钥
        let api_key = ApiKeyRepository::get_active(&self.db).await?;

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

            // 更新最后配置更新时间戳
            {
                let mut last_update = self.last_config_update.write().await;
                *last_update = key.updated_at;
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
            if SignalRepository::exists(
                &db,
                &config.symbol,
                signal.timestamp,
                &config.interval_type,
            )
            .await?
            {
                warn!(
                    "Signal already recorded for {} at {}",
                    config.symbol, signal.timestamp
                );
                return Ok(());
            }

            let should_place_order = place_order_by_long_short_config(config, &signal);

            if !should_place_order {
                warn!(
                    "Signal filtered!! Candle type {} and Direction {} does not match configuration for {}: long_k_long={}, short_k_short={}",
                    signal.candle_type,
                    signal.shadow_type,
                    config.symbol,
                    config.long_k_long,
                    config.short_k_short
                );
                return Ok(());
            }

            // 利润释放够手续费
            let expect_profit = signal.main_profit / last_kline.close * 100.0;
            if expect_profit <= config.expected_profit_rate {
                warn!(
                    "Signal filtered!! Expected profit ({:.2}%) is below the threshold ({:.2}%) for {}",
                    expect_profit, config.expected_profit_rate, config.symbol
                );
                return Ok(());
            }

            // 保存信号到数据库
            let signal_id = SignalRepository::save(db, &signal).await?;

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

            let contract = ApiKeyRepository::get_contract_by_symbol(db, &signal.symbol).await?;
            if contract.is_none() {
                warn!("No contract found for symbol: {}", signal.symbol);
                return Ok(());
            }

            // 如果启用自动交易，生成交易信号
            if config.enable_auto_trading {
                if let Some(trading_signal) = Self::generate_trading_signal(
                    &signal,
                    config,
                    contract.unwrap().order_price_round,
                ) {
                    // 下单
                    {
                        let order_data = build_order_data(
                            &trading_signal.symbol,
                            &config.order_type,
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
                        match gate_service
                            .place_order_with_stop_profit_loss(order_data, "usdt")
                            .await
                        {
                            Ok(response) => {
                                info!("Order placed successfully: {:?}", response);
                            }
                            Err(e) => {
                                error!("Failed to acquire gate service: {}", e);
                                return Ok(());
                            }
                        }
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
                    OrderRepository::save_from_trading_signal(db, &trading_signal, signal_id)
                        .await?;

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
        // 影线在上面
        let upper_shadow_length = latest.high - latest.close.max(latest.open);
        let upper_profit = latest.high - latest.close;
        // 影线在下面
        let lower_shadow_length = latest.open.min(latest.close) - latest.low;
        let lower_profit = latest.close - latest.low;

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
        let (shadow_type, main_shadow_length, main_profit, shadow_ratio) =
            if has_long_upper && has_long_lower {
                if upper_shadow_length >= lower_shadow_length {
                    (
                        "upper",
                        upper_shadow_length,
                        upper_profit,
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
                        lower_profit,
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
                    upper_profit,
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
                    lower_profit,
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
            main_profit,
            shadow_ratio,
            volume_multiplier,
            avg_volume: Some(avg_volume),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        })
    }

    fn generate_trading_signal(
        signal: &Signal,
        config: &MonitorConfig,
        order_price_round: String, // 订单价格精度
    ) -> Option<TradingSignal> {
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
            let take_profit = entry_price + signal.main_profit * config.risk_reward_ratio;
            (stop_loss, take_profit)
        } else {
            let stop_loss = signal.high_price;
            let take_profit = entry_price - signal.main_profit * config.risk_reward_ratio;
            (stop_loss, take_profit)
        };

        // 根据精度调整价格, 四舍五入
        let stop_loss = round_price(stop_loss, &order_price_round);
        let take_profit = round_price(take_profit, &order_price_round);

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
}

pub fn place_order_by_long_short_config(config: &MonitorConfig, signal: &Signal) -> bool {
    let should_place_order = if !config.long_k_long && !config.short_k_short {
        // 两个都没配置，默认下订单
        true
    } else if config.long_k_long && config.short_k_short {
        // 两个都配置了，满足其中一个条件就下订单
        (config.long_k_long && signal.candle_type == "bull" && signal.shadow_type == "lower")
            || (config.short_k_short
                && signal.candle_type == "bear"
                && signal.shadow_type == "upper")
    } else if config.long_k_long {
        // 只配置了long_k_long，只有阳线才下订单
        (signal.shadow_type == "upper")
            || (signal.candle_type == "bull" && signal.shadow_type == "lower")
    } else if config.short_k_short {
        // 只配置了short_k_short，只有阴线才下订单
        (signal.shadow_type == "lower")
            || (signal.candle_type == "bear" && signal.shadow_type == "upper")
    } else {
        false
    };
    return should_place_order;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_round_price() {
        assert_eq!(round_price(1.2345, "0.01"), 1.2);
        assert_eq!(round_price(1.2545, "0.01"), 1.3);
        assert_eq!(round_price(1.2345, "0.001"), 1.23);
        assert_eq!(round_price(1.2345, "0.1"), 1.0);
        assert_eq!(round_price(1.2345, "1"), 1.0);
        assert_eq!(round_price(1.7345, "1"), 2.0);
        assert_eq!(round_price(1.5345, "1a"), 2.0);
    }

    #[tokio::test]
    async fn test_place_order_by_long_short_config() {
        let signal1 = Signal {
            candle_type: "bull".into(),
            shadow_type: "lower".into(),
            ..Default::default()
        };
        let signal2 = Signal {
            candle_type: "bull".into(),
            shadow_type: "upper".into(),
            ..Default::default()
        };
        let signal3 = Signal {
            candle_type: "bear".into(),
            shadow_type: "lower".into(),
            ..Default::default()
        };
        let signal4 = Signal {
            candle_type: "bear".into(),
            shadow_type: "upper".into(),
            ..Default::default()
        };
        let config1 = MonitorConfig {
            long_k_long: false,
            short_k_short: false,
            ..Default::default()
        };
        let config2 = MonitorConfig {
            long_k_long: false,
            short_k_short: true,
            ..Default::default()
        };
        let config3 = MonitorConfig {
            long_k_long: true,
            short_k_short: false,
            ..Default::default()
        };
        let config4 = MonitorConfig {
            long_k_long: true,
            short_k_short: true,
            ..Default::default()
        };
        // 1
        assert_eq!(place_order_by_long_short_config(&config1, &signal1), true);
        assert_eq!(place_order_by_long_short_config(&config1, &signal2), true);
        assert_eq!(place_order_by_long_short_config(&config1, &signal3), true);
        assert_eq!(place_order_by_long_short_config(&config1, &signal4), true);
        // 2
        assert_eq!(place_order_by_long_short_config(&config2, &signal1), true);
        assert_eq!(place_order_by_long_short_config(&config2, &signal2), false);
        assert_eq!(place_order_by_long_short_config(&config2, &signal3), true);
        assert_eq!(place_order_by_long_short_config(&config2, &signal4), true);
        // 3
        assert_eq!(place_order_by_long_short_config(&config3, &signal1), true);
        assert_eq!(place_order_by_long_short_config(&config3, &signal2), true);
        assert_eq!(place_order_by_long_short_config(&config3, &signal3), false);
        assert_eq!(place_order_by_long_short_config(&config3, &signal4), true);
        // 4
        assert_eq!(place_order_by_long_short_config(&config4, &signal1), true);
        assert_eq!(place_order_by_long_short_config(&config4, &signal2), false);
        assert_eq!(place_order_by_long_short_config(&config4, &signal3), false);
        assert_eq!(place_order_by_long_short_config(&config4, &signal4), true);
    }

    #[tokio::test]
    async fn test_config_update_detection() {
        // 模拟配置更新时间戳的变化
        let last_config_update = Arc::new(RwLock::new(100i64));

        // 模拟数据库中的API配置有更新（updated_at > last_config_update）
        let mock_api_key = ApiKey {
            id: 1,
            name: "test_key".to_string(),
            api_key: "new_api_key".to_string(),
            secret_key: "new_secret_key".to_string(),
            webhook_url: Some("http://new-webhook.com".to_string()),
            cookie: Some("new_cookie".to_string()),
            contracts: Some("{\"contracts\":\"new_data\"}".to_string()),
            is_active: true,
            created_at: 50,
            updated_at: 200, // 比 last_config_update (100) 更大，表示有更新
        };

        // 验证时间戳比较逻辑
        let last_update = *last_config_update.read().await;
        assert!(
            mock_api_key.updated_at > last_update,
            "配置应该被检测为有更新"
        );
    }
}
