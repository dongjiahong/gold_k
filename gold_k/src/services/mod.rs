pub mod dingtalk;
pub mod gate;
pub mod monitor;

pub use dingtalk::DingTalkService;
pub use gate::GateService;
pub use monitor::MonitorService;

pub fn build_order_data(
    symbol: &str,
    order_type: &str,
    side: &str,
    price: f64,
    size: i64,
    take_profit_price: Option<f64>,
    stop_loss_price: Option<f64>,
) -> serde_json::Value {
    use serde_json::json;

    // 构建新的Web API格式的订单数据
    let mut order_data = json!({
        "order": {
            "contract": symbol,
            "size": if side == "buy" { size.abs() } else { -size.abs() },
            "text": "web",
            "tif": "gtc"
        }
    });

    // 设置价格
    if order_type == "limit" {
        order_data["order"]["price"] = json!(price.to_string());
    } else {
        order_data["order"]["price"] = json!("0"); // 市价单
        order_data["order"]["tif"] = json!("ioc");
    }

    // 添加止盈设置
    if let Some(tp_price) = take_profit_price {
        if tp_price > 0.0 {
            order_data["stop_profit"] = json!({
                "trigger_price_type": 0, // 标记价格触发
                "trigger_price": tp_price.to_string(),
                "order_price": "0" // 市价执行
            });
        }
    }

    // 添加止损设置
    if let Some(sl_price) = stop_loss_price {
        if sl_price > 0.0 {
            order_data["stop_loss"] = json!({
                "trigger_price_type": 0, // 标记价格触发
                "trigger_price": sl_price.to_string(),
                "order_price": "0" // 市价执行
            });
        }
    }

    order_data
}
