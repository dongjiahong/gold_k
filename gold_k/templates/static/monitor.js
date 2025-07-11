// 监控页面专用JavaScript - monitor.js

let configs = [];

// 添加配置项
function addConfig() {
    const config = {
        symbol: 'BTC_USDT',
        interval_type: '15m',
        frequency: 30,
        history_hours: 3,
        shadow_ratio: 4.5,
        main_shadow_body_ratio: 1.0,
        volume_multiplier: 1.5,
        order_size: 1.0,
        risk_reward_ratio: 1.2,
        enable_auto_trading: false,
        enable_dingtalk: false,
        trade_direction: 'both',
        is_active: true
    };
    configs.push(config);
    renderConfigs();
}

// 渲染配置列表
function renderConfigs() {
    const container = document.getElementById('config-list');
    if (!container) return;

    container.innerHTML = configs.map((config, index) => {
        const contract = validateSymbol(config.symbol);
        const isValidSymbol = !!contract;
        const orderValue = calculateOrderValue(config.symbol, config.order_size);

        return `
        <div class="config-item">
            <div class="config-header" onclick="toggleConfig(${index})">
                <div class="config-title">
                    ${config.symbol} - ${config.interval_type}
                    ${!isValidSymbol ? '<span style="color: #f44336; font-size: 0.8em;">⚠️ 无效交易对</span>' : ''}
                    <span class="config-toggle" id="toggle-${index}">▼</span>
                </div>
                <button class="btn btn-danger tiny" onclick="event.stopPropagation(); removeConfig(${index})">删除</button>
            </div>
            <div class="config-form" id="config-form-${index}">
                <div class="form-group">
                    <label>交易对 ${!isValidSymbol ? '<span style="color: #f44336;">*无效</span>' : ''}</label>
                    <input type="text" value="${config.symbol}" onchange="updateSymbol(${index}, this.value)" placeholder="如: BTC_USDT">
                </div>
                <div class="form-group">
                    <label>时间周期</label>
                    <select onchange="updateConfig(${index}, 'interval_type', this.value)">
                        <option value="1m" ${config.interval_type === '1m' ? 'selected' : ''}>1分钟</option>
                        <option value="5m" ${config.interval_type === '5m' ? 'selected' : ''}>5分钟</option>
                        <option value="15m" ${config.interval_type === '15m' ? 'selected' : ''}>15分钟</option>
                        <option value="30m" ${config.interval_type === '30m' ? 'selected' : ''}>30分钟</option>
                        <option value="1h" ${config.interval_type === '1h' ? 'selected' : ''}>1小时</option>
                        <option value="4h" ${config.interval_type === '4h' ? 'selected' : ''}>4小时</option>
                        <option value="1d" ${config.interval_type === '1d' ? 'selected' : ''}>1天</option>
                    </select>
                </div>
                <div class="form-group">
                    <label>检查频率(秒)</label>
                    <input type="number" min="3" value="${config.frequency}" onchange="updateConfig(${index}, 'frequency', parseInt(this.value))">
                </div>
                <div class="form-group">
                    <label>历史时间(小时)</label>
                    <input type="number" step="0.1" min="0.1" value="${config.history_hours}" onchange="updateConfig(${index}, 'history_hours', parseFloat(this.value))">
                </div>
                <div class="form-group">
                    <label>影线比例阈值</label>
                    <input type="number" step="0.1" min="0.1" value="${config.shadow_ratio}" onchange="updateConfig(${index}, 'shadow_ratio', parseFloat(this.value))">
                </div>
                <div class="form-group">
                    <label>影线/实体比例</label>
                    <input type="number" step="0.1" min="0.1" value="${config.main_shadow_body_ratio}" onchange="updateConfig(${index}, 'main_shadow_body_ratio', parseFloat(this.value))">
                </div>
                <div class="form-group">
                    <label>平均交量倍数</label>
                    <input type="number" step="0.1" min="0.1" value="${config.volume_multiplier}" onchange="updateConfig(${index}, 'volume_multiplier', parseFloat(this.value))">
                </div>
                <div class="form-group">
                    <label>订单大小(张) ${isValidSymbol ? `<span style="color: #4CAF50; font-size: 0.8em;">≈ ${orderValue} ${config.symbol.split('_')[0]}</span>` : ''}</label>
                    <input type="number" step="0.1" min="0.1" value="${config.order_size}" onchange="updateOrderSize(${index}, parseFloat(this.value))">
                </div>
                <div class="form-group">
                    <label>风险收益比</label>
                    <input type="number" step="0.1" min="0.1" value="${config.risk_reward_ratio}" onchange="updateConfig(${index}, 'risk_reward_ratio', parseFloat(this.value))">
                </div>
                <div class="form-group">
                    <label>交易方向</label>
                    <select onchange="updateConfig(${index}, 'trade_direction', this.value)">
                        <option value="both" ${config.trade_direction === 'both' ? 'selected' : ''}>双向</option>
                        <option value="long" ${config.trade_direction === 'long' ? 'selected' : ''}>仅做多</option>
                        <option value="short" ${config.trade_direction === 'short' ? 'selected' : ''}>仅做空</option>
                    </select>
                </div>
                <div class="form-group">
                    <div class="checkbox-group">
                        <input type="checkbox" ${config.enable_auto_trading ? 'checked' : ''} onchange="updateConfig(${index}, 'enable_auto_trading', this.checked)">
                        <label>启用自动交易</label>
                    </div>
                    <div class="checkbox-group">
                        <input type="checkbox" ${config.enable_dingtalk ? 'checked' : ''} onchange="updateConfig(${index}, 'enable_dingtalk', this.checked)">
                        <label>启用钉钉通知</label>
                    </div>
                </div>
            </div>
        </div>
    `}).join('');
}

// 更新交易对
function updateSymbol(index, value) {
    updateConfig(index, 'symbol', value);
    // 验证交易对并显示提示
    const contract = validateSymbol(value);
    if (!contract && value) {
        showMessage(`警告: 交易对 ${value} 不存在于合约列表中`, 'error');
    }
    renderConfigs(); // 重新渲染以更新订单价值显示
}

// 更新订单大小
function updateOrderSize(index, value) {
    updateConfig(index, 'order_size', value);
    renderConfigs(); // 重新渲染以更新订单价值显示
}

// 切换配置项折叠状态
function toggleConfig(index) {
    const form = document.getElementById(`config-form-${index}`);
    const toggle = document.getElementById(`toggle-${index}`);

    if (!form || !toggle) return;

    if (form.classList.contains('collapsed')) {
        form.classList.remove('collapsed');
        toggle.textContent = '▼';
    } else {
        form.classList.add('collapsed');
        toggle.textContent = '▶';
    }
}

// 更新配置
function updateConfig(index, key, value) {
    if (configs[index]) {
        configs[index][key] = value;
    }
}

// 删除配置
function removeConfig(index) {
    configs.splice(index, 1);
    renderConfigs();
}

// 保存配置前验证
async function saveConfigs() {
    // 验证所有交易对
    const invalidSymbols = configs.filter(config => !validateSymbol(config.symbol));
    if (invalidSymbols.length > 0) {
        const symbols = invalidSymbols.map(config => config.symbol).join(', ');
        showMessage(`错误: 以下交易对无效: ${symbols}`, 'error');
        return;
    }

    try {
        const result = await apiRequest('/api/configs', {
            method: 'POST',
            body: JSON.stringify(configs)
        });

        showMessage('配置保存成功！');
    } catch (error) {
        showMessage('保存失败: ' + error.message, 'error');
    }
}

// 加载配置
async function loadConfigs() {
    try {
        configs = await apiRequest('/api/configs');
        renderConfigs();
    } catch (error) {
        showMessage('加载配置失败: ' + error.message, 'error');
    }
}

// 启动监控
async function startMonitor() {
    try {
        const result = await apiRequest('/api/monitor/start', { method: 'POST' });

        if (result.success) {
            showMessage(result.message);
            loadStatus();
        } else {
            showMessage(result.message, 'error');
        }
    } catch (error) {
        showMessage('启动失败: ' + error.message, 'error');
    }
}

// 停止监控
async function stopMonitor() {
    try {
        const result = await apiRequest('/api/monitor/stop', { method: 'POST' });

        if (result.success) {
            showMessage(result.message);
            loadStatus();
        } else {
            showMessage(result.message, 'error');
        }
    } catch (error) {
        showMessage('停止失败: ' + error.message, 'error');
    }
}

// 加载状态
async function loadStatus() {
    try {
        const status = await apiRequest('/api/monitor/status');

        const indicator = document.getElementById('status-indicator');
        const startBtn = document.getElementById('start-btn');
        const stopBtn = document.getElementById('stop-btn');

        if (!indicator || !startBtn || !stopBtn) return;

        if (status.is_running) {
            indicator.textContent = '运行中';
            indicator.className = 'status-indicator status-running';
            startBtn.disabled = true;
            stopBtn.disabled = false;
        } else {
            indicator.textContent = '已停止';
            indicator.className = 'status-indicator status-stopped';
            startBtn.disabled = false;
            stopBtn.disabled = true;
        }
    } catch (error) {
        const indicator = document.getElementById('status-indicator');
        if (indicator) {
            indicator.textContent = '服务异常';
            indicator.className = 'status-indicator status-stopped';
        }
        showMessage('加载状态失败: ' + error.message, 'error');
    }
}

// 页面加载时初始化
document.addEventListener('DOMContentLoaded', function () {
    loadStatus();
    loadConfigs();
});

// 每5秒钟更新一次状态
setInterval(loadStatus, 5000);
