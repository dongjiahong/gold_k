use crate::models::KlineData;
use anyhow::{Result, anyhow};
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use serde_urlencoded;
use sha2::{Digest, Sha512};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

type HmacSha512 = Hmac<Sha512>;

#[derive(Debug, Clone)]
pub struct GateService {
    client: Client,
    api_key: Option<String>,
    secret_key: Option<String>,
    base_url: String,
}

impl GateService {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            api_key: None,
            secret_key: None,
            base_url: "https://api.gateio.ws/api/v4".to_string(),
        }
    }

    pub fn update_credentials(&mut self, api_key: &str, secret_key: &str) {
        self.api_key = Some(api_key.to_string());
        self.secret_key = Some(secret_key.to_string());
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

        // Gate.io K线数据格式: [timestamp, volume, close, high, low, open, ...]
        let klines = data
            .as_array()
            .ok_or_else(|| anyhow!("Invalid response format"))?;

        let mut result = Vec::new();
        for kline in klines {
            let kline_array = kline
                .as_array()
                .ok_or_else(|| anyhow!("Invalid kline format"))?;

            if kline_array.len() < 6 {
                warn!("Incomplete kline data: {:?}", kline_array);
                continue;
            }

            let timestamp = kline_array[0]
                .as_str()
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or_else(|| anyhow!("Invalid timestamp"))?;

            let volume = kline_array[1]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| anyhow!("Invalid volume"))?;

            let close = kline_array[2]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| anyhow!("Invalid close price"))?;

            let high = kline_array[3]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| anyhow!("Invalid high price"))?;

            let low = kline_array[4]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| anyhow!("Invalid low price"))?;

            let open = kline_array[5]
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
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
}
