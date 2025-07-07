use crate::models::KlineData;
use anyhow::{Result, anyhow};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use serde_urlencoded;
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

type HmacSha512 = Hmac<Sha512>;

#[derive(Debug, Clone)]
pub struct GateService {
    client: Client,
    api_key: Option<String>,
    secret_key: Option<String>,
    base_url: String,
    cookie: Option<String>,
    contracts: Option<String>,
}

impl GateService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: None,
            secret_key: None,
            base_url: "https://api.gateio.ws/api/v4".to_string(),
            cookie: None,
            contracts: None,
        }
    }

    pub fn update_credentials(&mut self, api_key: &str, secret_key: &str) {
        self.api_key = Some(api_key.to_string());
        self.secret_key = Some(secret_key.to_string());
    }

    pub fn set_cookie(&mut self, cookie: &str) {
        self.cookie = Some(cookie.to_string());
    }

    pub fn set_contracts(&mut self, contracts: &str) {
        self.contracts = Some(contracts.to_string());
    }

    pub fn has_credentials(&self) -> bool {
        self.api_key.is_some() && self.secret_key.is_some()
    }

    async fn generate_signature(
        &self,
        method: &str,
        url_path: &str,
        query_string: &str,
        body: &str,
        timestamp: u64,
    ) -> Result<String> {
        let secret_key = self
            .secret_key
            .as_ref()
            .ok_or_else(|| anyhow!("Secret key not set"))?;

        // 计算 body 的 SHA512 哈希
        let body_hash = sha2::Sha512::digest(body.as_bytes());
        let body_hash_hex = hex::encode(body_hash);

        // 构建签名字符串
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}\n{}",
            method.to_uppercase(),
            url_path,
            query_string,
            body_hash_hex,
            timestamp
        );

        debug!("String to sign: {}", string_to_sign);

        // 生成 HMAC-SHA512 签名
        let mut mac = HmacSha512::new_from_slice(secret_key.as_bytes())
            .map_err(|e| anyhow!("Invalid secret key: {}", e))?;
        mac.update(string_to_sign.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        Ok(signature)
    }

    pub async fn get_kline_data(
        &self,
        symbol: &str,
        interval: &str,
        limit: usize,
        settle: &str,
    ) -> Result<Vec<KlineData>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let limit_str = limit.to_string();
        let mut query_params = HashMap::new();
        query_params.insert("contract", symbol);
        query_params.insert("interval", interval);
        query_params.insert("limit", &limit_str);

        let query_string = serde_urlencoded::to_string(&query_params)?;
        let url_path = format!("/futures/{}/candlesticks", settle);
        let url = format!("{}{}?{}", self.base_url, url_path, query_string);

        debug!("Request URL: {}", url);

        if !self.has_credentials() {
            return Err(anyhow!("API credentials not configured"));
        }

        let signature = self
            .generate_signature("GET", &url_path, &query_string, "", timestamp)
            .await?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("API key not set"))?;

        let response = self
            .client
            .get(&url)
            .header("KEY", api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!("Response status: {}", status);
        debug!("Response body: {}", response_text);

        if !status.is_success() {
            return Err(anyhow!(
                "API request failed: {} - {}",
                status,
                response_text
            ));
        }

        let data: Value = serde_json::from_str(&response_text)?;

        // Gate.io K线数据格式: [{"o":"","t":1234, "c":"", "l": "", "h": "", "v": 1 }]
        let klines = data
            .as_array()
            .ok_or_else(|| anyhow!("Invalid response format"))?;

        let mut result = Vec::new();
        for kline in klines {
            let kline_obj = kline
                .as_object()
                .ok_or_else(|| anyhow!("Invalid kline format"))?;

            let timestamp = kline_obj
                .get("t")
                .and_then(|t| t.as_i64())
                .ok_or_else(|| anyhow!("Invalid timestamp"))?;

            let volume = kline_obj
                .get("v")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| anyhow!("Invalid volume"))?;

            let close = kline_obj
                .get("c")
                .and_then(|c| c.as_str().and_then(|s| s.parse::<f64>().ok()))
                .ok_or_else(|| anyhow!("Invalid close price"))?;

            let high = kline_obj
                .get("h")
                .and_then(|h| h.as_str().and_then(|s| s.parse::<f64>().ok()))
                .ok_or_else(|| anyhow!("Invalid high price"))?;

            let low = kline_obj
                .get("l")
                .and_then(|l| l.as_str().and_then(|s| s.parse::<f64>().ok()))
                .ok_or_else(|| anyhow!("Invalid low price"))?;

            let open = kline_obj
                .get("o")
                .and_then(|o| o.as_str().and_then(|s| s.parse::<f64>().ok()))
                .ok_or_else(|| anyhow!("Invalid open price"))?;

            result.push(KlineData {
                timestamp,
                open,
                high,
                low,
                close,
                volume,
            });
        }

        Ok(result)
    }

    pub async fn get_contracts(&self, settle: &str) -> Result<Vec<Value>> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let url_path = format!("/futures/{}/contracts", settle);
        let url = format!("{}{}", self.base_url, url_path);

        if !self.has_credentials() {
            return Err(anyhow!("API credentials not configured"));
        }

        let signature = self
            .generate_signature("GET", &url_path, "", "", timestamp)
            .await?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("API key not set"))?;

        let response = self
            .client
            .get(&url)
            .header("KEY", api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            return Err(anyhow!(
                "API request failed: {} - {}",
                status,
                response_text
            ));
        }

        let contracts: Vec<Value> = serde_json::from_str(&response_text)?;
        Ok(contracts)
    }

    pub async fn place_order(
        &self,
        symbol: &str,
        side: &str, // "buy" or "sell"
        size: f64,
        price: Option<f64>,
        settle: &str,
    ) -> Result<Value> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        let mut order_data = serde_json::json!({
            "contract": symbol,
            "size": if side == "buy" { size.abs() } else { -size.abs() },
            "text": "rust-client"
        });

        if let Some(p) = price {
            order_data["price"] = serde_json::Value::String(p.to_string());
            order_data["tif"] = serde_json::Value::String("gtc".to_string());
        } else {
            order_data["price"] = serde_json::Value::String("0".to_string());
            order_data["tif"] = serde_json::Value::String("ioc".to_string());
        }

        let body = serde_json::to_string(&order_data)?;
        let url_path = format!("/futures/{}/orders", settle);
        let url = format!("{}{}", self.base_url, url_path);

        if !self.has_credentials() {
            return Err(anyhow!("API credentials not configured"));
        }

        let signature = self
            .generate_signature("POST", &url_path, "", &body, timestamp)
            .await?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("API key not set"))?;

        let response = self
            .client
            .post(&url)
            .header("KEY", api_key)
            .header("Timestamp", timestamp.to_string())
            .header("SIGN", signature)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        if !status.is_success() {
            return Err(anyhow!(
                "API request failed: {} - {}",
                status,
                response_text
            ));
        }

        let result: Value = serde_json::from_str(&response_text)?;
        Ok(result)
    }

    /// 使用Web API进行止盈止损下单
    pub async fn place_order_with_stop_profit_loss(
        &self,
        order_data: Value,
        settle: &str,
    ) -> Result<Value> {
        let web_api_url = format!(
            "https://www.gate.com/apiw/v2/futures/{}/price_orders/order_stop_order",
            settle
        );

        // 检查是否有cookie和contracts（作为CSRF token）
        let cookie_string = self
            .cookie
            .as_ref()
            .ok_or_else(|| anyhow!("Cookie未设置，请确保已在gate.com上登录"))?;

        let csrf_token = self.set_web_credentials()?;

        let body = serde_json::to_string(&order_data)?;

        debug!("使用Web API进行止盈止损下单: {}", body);
        debug!("CSRF Token: {}", csrf_token);
        debug!("Request URL: {}", web_api_url);

        let response = self
            .client
            .post(&web_api_url)
            .header("Content-Type", "application/json")
            .header("Origin", "https://www.gate.com")
            .header("csrftoken", csrf_token)
            .header("Cookie", cookie_string)
            .header("Accept", "application/json, text/plain, */*")
            .header("Referer", "https://www.gate.com/")
            .header(
                "User-Agent",
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36",
            )
            .body(body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        debug!(
            "Web API止盈止损响应状态: {}, 响应内容: {}",
            status, response_text
        );

        if !status.is_success() {
            let error_data: Result<Value, _> = serde_json::from_str(&response_text);
            let error_message = match error_data {
                Ok(data) => data
                    .get("message")
                    .or_else(|| data.get("error"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("未知错误")
                    .to_string(),
                Err(_) => "解析错误响应失败".to_string(),
            };

            return Err(anyhow!("Web API错误 ({}): {}", status, error_message));
        }

        let result: Value =
            serde_json::from_str(&response_text).map_err(|e| anyhow!("解析响应失败: {}", e))?;

        Ok(result)
    }

    /// 设置从浏览器获取的完整cookie字符串和CSRF token
    pub fn set_web_credentials(&self) -> Result<String> {
        // 从cookie中提取CSRF token
        let cookie = self
            .cookie
            .as_ref()
            .ok_or_else(|| anyhow!("Cookie未设置"))?;

        for cookie_pair in cookie.split(';') {
            let trimmed = cookie_pair.trim();
            if trimmed.starts_with("csrftoken=") {
                let csrftoken = trimmed[10..].to_string();
                return Ok(csrftoken);
            }
        }
        return Err(anyhow!("无法从cookie中提取CSRF Token"));
    }
}
