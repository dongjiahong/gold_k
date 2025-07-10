use crate::models::{DingTalkMarkdown, DingTalkMessage, DingTalkText, Signal, TradingSignal};
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct DingTalkService {
    client: Client,
    webhook_url: Option<String>,
}

impl DingTalkService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            webhook_url: None,
        }
    }

    pub fn set_webhook_url(&mut self, url: &str) {
        self.webhook_url = Some(url.to_string());
    }

    pub fn has_webhook(&self) -> bool {
        self.webhook_url.is_some()
    }

    pub async fn send_text_message(&self, content: &str) -> Result<()> {
        let message = DingTalkMessage {
            msgtype: "text".to_string(),
            text: Some(DingTalkText {
                content: content.to_string(),
            }),
            markdown: None,
        };

        self.send_message(message).await
    }

    pub async fn send_markdown_message(&self, title: &str, text: &str) -> Result<()> {
        let message = DingTalkMessage {
            msgtype: "markdown".to_string(),
            text: None,
            markdown: Some(DingTalkMarkdown {
                title: title.to_string(),
                text: text.to_string(),
            }),
        };

        self.send_message(message).await
    }

    pub async fn send_signal_alert(&self, signal: &Signal) -> Result<()> {
        let candle_type_text = if signal.candle_type == "bull" {
            "阳线"
        } else {
            "阴线"
        };
        let shadow_type_text = if signal.shadow_type == "upper" {
            "上影线"
        } else {
            "下影线"
        };

        let shadow_multiple =
            (signal.main_shadow_length / signal.body_length * 100.0).round() / 100.0;
        let volume_multiple = if let Some(avg_vol) = signal.avg_volume {
            (signal.volume / avg_vol * 100.0).round() / 100.0
        } else {
            1.0
        };

        let timestamp = utils::format_timestamp(signal.timestamp, 8);

        let title = format!("🚨 K线信号报警 - {}", signal.symbol);

        let markdown_text = format!(
            r#"
# {}
---
- **交易对**: {}
- **时间**: {}
- **周期**: {}
- **价格**: {:.4}
---
## 📊 信号详情
- **K线类型**: {}{}
- **影/实体倍数**: {:.2}x  
- **成交量倍数**: {:.2}x  
---
## 📈 技术指标
- **开盘价**: {:.4}
- **最高价**: {:.4}  
- **最低价**: {:.4}
- **收盘价**: {:.4}
- **成交量**: {}
---
> ⚠️ 此为系统自动监控信号，仅供参考，请结合其他指标做出投资决策
            "#,
            title,
            signal.symbol,
            timestamp,
            signal.interval_type,
            signal.close_price,
            candle_type_text,
            shadow_type_text,
            shadow_multiple,
            volume_multiple,
            signal.open_price,
            signal.high_price,
            signal.low_price,
            signal.close_price,
            signal.volume as u64
        );

        self.send_markdown_message(&title, &markdown_text).await
    }

    pub async fn send_trading_signal(&self, trading_signal: &TradingSignal) -> Result<()> {
        let direction_emoji = if trading_signal.signal_type == "long" {
            "📈"
        } else {
            "📉"
        };
        let direction_text = if trading_signal.signal_type == "long" {
            "做多"
        } else {
            "做空"
        };

        let timestamp = utils::format_timestamp(trading_signal.timestamp, 8);

        let risk_reward = (trading_signal.take_profit - trading_signal.entry_price).abs()
            / (trading_signal.entry_price - trading_signal.stop_loss).abs();

        let title = format!(
            "💡 K线交易信号 - {} {}",
            trading_signal.symbol, direction_emoji
        );

        let markdown_text = format!(
            r#"
# {}
---
- **交易对**: {}
- **时间**: {}
- **方向**: {} {}
- **入场价**: {:.4}
- **止损价**: {:.4}
- **止盈价**: {:.4}
- **风险收益比**: 1:{:.1}
- **信心等级**: {}
---
## 💭 分析理由
{}
---
> 🎯 请根据自身风险承受能力谨慎操作
            "#,
            title,
            trading_signal.symbol,
            timestamp,
            direction_text,
            direction_emoji,
            trading_signal.entry_price,
            trading_signal.stop_loss,
            trading_signal.take_profit,
            risk_reward,
            trading_signal.confidence,
            trading_signal.reason
        );

        self.send_markdown_message(&title, &markdown_text).await
    }

    pub async fn test_connection(&self) -> Result<()> {
        self.send_text_message(
            "🔔 Gate.io K线监控工具测试消息\n\n如果您收到此消息，说明钉钉机器人配置成功！",
        )
        .await
    }

    async fn send_message(&self, message: DingTalkMessage) -> Result<()> {
        let webhook_url = self
            .webhook_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Webhook URL not configured"))?;

        debug!("Sending DingTalk message: {:?}", message);

        let response = self
            .client
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .json(&message)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!("DingTalk response status: {}", status);
        debug!("DingTalk response body: {}", response_text);

        if !status.is_success() {
            return Err(anyhow::anyhow!(
                "DingTalk API request failed: {} - {}",
                status,
                response_text
            ));
        }

        let result: Value = serde_json::from_str(&response_text)?;

        if let Some(errcode) = result.get("errcode").and_then(|v| v.as_i64()) {
            if errcode != 0 {
                let errmsg = result
                    .get("errmsg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error");
                return Err(anyhow::anyhow!("DingTalk message send failed: {}", errmsg));
            }
        }

        debug!("DingTalk message sent successfully");
        Ok(())
    }
}
