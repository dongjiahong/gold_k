
curl -X POST -H "Content-Type: application/json" http://localhost:3000/api/order/place -d '{"symbol": "DOGE_USDT", "order_type": "limit", "side": "buy", "entry_price": 0.19, "size": 1, "take_profit": 0.29, "stop_loss": 0.17}'
