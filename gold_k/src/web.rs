use askama::Template;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, services::ServeDir};
use tracing::{info, warn};

use crate::repository::{
    ApiKeyRepository, MonitorConfigRepository, OrderRepository, SignalRepository,
};
use crate::services::*;
use crate::templates::*;
use crate::{config::get_global_config, models::*};

pub type AppState = Arc<AppStateInner>;

pub struct AppStateInner {
    pub db: SqlitePool,
    pub gate_service: RwLock<GateService>,
    pub monitor_service: RwLock<MonitorService>,
}

pub async fn start() -> anyhow::Result<()> {
    // 初始化数据库
    let c = get_global_config().await;
    let db = SqlitePool::connect(&c.database_url).await?;

    // 运行数据库迁移
    sqlx::migrate!("../migrations").run(&db).await?;

    // 初始化服务
    let mut gate_service = GateService::new();
    let monitor_service = MonitorService::new(db.clone());

    // 加载当前活跃的API配置
    if let Ok(Some(key)) = ApiKeyRepository::get_active(&db).await {
        gate_service.update_credentials(&key.api_key, &key.secret_key);
        if let Some(cookie) = &key.cookie {
            gate_service.set_cookie(cookie);
        }
        if let Some(contracts) = &key.contracts {
            gate_service.set_contracts(contracts);
        }
        info!("Loaded API configuration: {}", key.name);
    }

    // 创建应用状态
    let state = Arc::new(AppStateInner {
        db,
        gate_service: RwLock::new(gate_service),
        monitor_service: RwLock::new(monitor_service),
    });

    // 创建路由
    let app = Router::new()
        .route("/", get(dashboard))
        .route("/api/keys", get(get_api_keys).post(save_api_keys))
        .route("/api/keys/current", get(get_current_api_key))
        .route("/api/keys/{id}/activate", post(activate_api_key))
        .route("/api/keys/{id}", post(delete_api_key))
        .route("/api/contracts/fetch", post(fetch_contracts))
        .route("/api/monitor/start", post(start_monitor))
        .route("/api/monitor/stop", post(stop_monitor))
        .route("/api/monitor/status", get(get_monitor_status))
        .route("/api/signals", get(get_signals))
        .route("/api/orders", get(get_orders))
        .route(
            "/api/configs",
            get(get_monitor_configs).post(save_monitor_configs),
        )
        .route("/api/dingding/test", get(dingding_test))
        .route("/keys", get(keys_page))
        .route("/monitor", get(monitor_page))
        .nest_service("/static", ServeDir::new("static"))
        .layer(ServiceBuilder::new().layer(CorsLayer::permissive()))
        .with_state(state);

    info!("Server starting on http://localhost:3000");

    let listener = tokio::net::TcpListener::bind("localhost:3000").await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// 页面路由处理器
async fn dashboard() -> impl IntoResponse {
    let template = DashboardTemplate {};
    Html(template.render().unwrap())
}

async fn keys_page() -> impl IntoResponse {
    let template = KeysTemplate {};
    Html(template.render().unwrap())
}

async fn monitor_page() -> impl IntoResponse {
    let template = MonitorTemplate {};
    Html(template.render().unwrap())
}

// API 路由处理器
async fn get_api_keys(State(state): State<AppState>) -> impl IntoResponse {
    match ApiKeyRepository::get_all(&state.db).await {
        Ok(keys) => Json(keys).into_response(),
        Err(e) => {
            warn!("Failed to get api keys: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn save_api_keys(
    State(state): State<AppState>,
    Json(payload): Json<SaveApiKeysRequest>,
) -> impl IntoResponse {
    // 先删除所有现有配置
    if let Err(e) = ApiKeyRepository::delete_all(&state.db).await {
        warn!("Failed to clear api keys: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // 插入新配置
    match ApiKeyRepository::save(
        &state.db,
        &payload.name,
        &payload.api_key,
        &payload.secret_key,
        payload.webhook_url.as_deref(),
        payload.cookie.as_deref(),
    )
    .await
    {
        Ok(_) => {
            // 更新服务配置
            let mut gate_service = state.gate_service.write().await;
            gate_service.update_credentials(&payload.api_key, &payload.secret_key);
            if let Some(cookie) = &payload.cookie {
                gate_service.set_cookie(cookie);
            }

            Json(serde_json::json!({"success": true})).into_response()
        }
        Err(e) => {
            warn!("Failed to save api keys: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn activate_api_key(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> impl IntoResponse {
    // 先将所有密钥设置为非活跃状态
    if let Err(e) = ApiKeyRepository::deactivate_all(&state.db).await {
        warn!("Failed to deactivate api keys: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // 激活指定密钥
    match ApiKeyRepository::activate(&state.db, id).await {
        Ok(_) => {
            // 获取激活的密钥并更新服务配置
            if let Ok(Some(key)) = ApiKeyRepository::get_by_id(&state.db, id).await {
                let mut gate_service = state.gate_service.write().await;
                gate_service.update_credentials(&key.api_key, &key.secret_key);
                if let Some(cookie) = &key.cookie {
                    gate_service.set_cookie(cookie);
                }
                if let Some(contracts) = &key.contracts {
                    gate_service.set_contracts(contracts);
                }
            }

            Json(serde_json::json!({"success": true})).into_response()
        }
        Err(e) => {
            warn!("Failed to activate api key: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn delete_api_key(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> impl IntoResponse {
    match ApiKeyRepository::delete_by_id(&state.db, id).await {
        Ok(_) => Json(serde_json::json!({"success": true})).into_response(),
        Err(e) => {
            warn!("Failed to delete api key: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn start_monitor(State(state): State<AppState>) -> impl IntoResponse {
    let mut monitor_service = state.monitor_service.write().await;
    match monitor_service.start().await {
        Ok(_) => {
            Json(serde_json::json!({"success": true, "message": "监控已启动"})).into_response()
        }
        Err(e) => {
            warn!("Failed to start monitor: {}", e);
            Json(serde_json::json!({"success": false, "message": format!("启动失败: {}", e)}))
                .into_response()
        }
    }
}

async fn stop_monitor(State(state): State<AppState>) -> impl IntoResponse {
    let mut monitor_service = state.monitor_service.write().await;
    monitor_service.stop().await;
    Json(serde_json::json!({"success": true, "message": "监控已停止"})).into_response()
}

async fn get_monitor_status(State(state): State<AppState>) -> impl IntoResponse {
    let monitor_service = state.monitor_service.read().await;
    let status = monitor_service.get_status().await;
    Json(status).into_response()
}

async fn get_signals(State(state): State<AppState>) -> impl IntoResponse {
    match SignalRepository::get_recent(&state.db, 100).await {
        Ok(signals) => Json(signals).into_response(),
        Err(e) => {
            warn!("Failed to get signals: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_orders(State(state): State<AppState>) -> impl IntoResponse {
    match OrderRepository::get_recent(&state.db, 100).await {
        Ok(orders) => Json(orders).into_response(),
        Err(e) => {
            warn!("Failed to get orders: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_monitor_configs(State(state): State<AppState>) -> impl IntoResponse {
    match MonitorConfigRepository::get_all(&state.db).await {
        Ok(configs) => Json(configs).into_response(),
        Err(e) => {
            warn!("Failed to get monitor configs: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn save_monitor_configs(
    State(state): State<AppState>,
    Json(configs): Json<Vec<MonitorConfig>>,
) -> impl IntoResponse {
    match MonitorConfigRepository::save_batch(&state.db, &configs).await {
        Ok(_) => Json(serde_json::json!({"success": true})).into_response(),
        Err(e) => {
            warn!("Failed to save monitor configs: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn get_current_api_key(State(state): State<AppState>) -> impl IntoResponse {
    match ApiKeyRepository::get_active(&state.db).await {
        Ok(Some(key)) => Json(key).into_response(),
        Ok(None) => Json(serde_json::Value::Null).into_response(),
        Err(e) => {
            warn!("Failed to get current api key: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn fetch_contracts(State(state): State<AppState>) -> impl IntoResponse {
    // 获取当前活跃的API配置
    let current_key = match ApiKeyRepository::get_active(&state.db).await {
        Ok(Some(key)) => key,
        Ok(None) => {
            return Json(serde_json::json!({"success": false, "message": "未找到活跃的API配置"}))
                .into_response();
        }
        Err(e) => {
            warn!("Failed to get current api key: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // 使用Gate服务获取合约数据
    let gate_service = state.gate_service.read().await;
    match gate_service.get_contracts("usdt").await {
        Ok(contracts) => {
            let contracts_json = serde_json::to_string(&contracts).unwrap_or_default();

            // 更新数据库中的合约数据
            if let Err(e) =
                ApiKeyRepository::update_contracts(&state.db, current_key.id, &contracts_json).await
            {
                warn!("Failed to update contracts: {}", e);
            }

            Json(serde_json::json!({
                "success": true,
                "count": contracts.len(),
                "data": contracts,
                "message": format!("成功获取{}个合约", contracts.len())
            }))
            .into_response()
        }
        Err(e) => {
            warn!("Failed to fetch contracts: {}", e);
            Json(serde_json::json!({
                "success": false,
                "message": format!("获取合约失败: {}", e)
            }))
            .into_response()
        }
    }
}

#[derive(Deserialize)]
struct SaveApiKeysRequest {
    name: String,
    api_key: String,
    secret_key: String,
    webhook_url: Option<String>,
    cookie: Option<String>,
}

async fn dingding_test(State(state): State<AppState>) -> impl IntoResponse {
    // 获取当前活跃的API配置以获取webhook_url
    let current_key = match ApiKeyRepository::get_active(&state.db).await {
        Ok(Some(key)) => key,
        Ok(None) => {
            return Json(serde_json::json!({
                "success": false,
                "message": "未找到活跃的API配置"
            }))
            .into_response();
        }
        Err(e) => {
            warn!("Failed to get current api key: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // 检查是否配置了webhook_url
    let webhook_url = match &current_key.webhook_url {
        Some(url) if !url.is_empty() => url,
        _ => {
            return Json(serde_json::json!({
                "success": false,
                "message": "未配置钉钉机器人Webhook URL"
            }))
            .into_response();
        }
    };

    // 创建钉钉服务实例并设置webhook URL
    let mut dingtalk_service = crate::services::dingtalk::DingTalkService::new();
    dingtalk_service.set_webhook_url(webhook_url);

    // 发送测试消息
    match dingtalk_service.test_connection().await {
        Ok(_) => Json(serde_json::json!({
            "success": true,
            "message": "钉钉机器人测试成功！请检查您的钉钉群聊"
        }))
        .into_response(),
        Err(e) => {
            warn!("DingTalk test failed: {}", e);
            Json(serde_json::json!({
                "success": false,
                "message": format!("钉钉机器人测试失败: {}", e)
            }))
            .into_response()
        }
    }
}
