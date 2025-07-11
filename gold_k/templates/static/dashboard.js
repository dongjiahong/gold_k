// 仪表板页面专用JavaScript - dashboard.js

// 加载系统状态
async function loadStatus() {
    try {
        const status = await apiRequest('/api/monitor/status');

        const setTextIfExists = (id, text) => {
            const element = document.getElementById(id);
            if (element) element.textContent = text;
        };

        const setStatusIfExists = (id, isRunning) => {
            const element = document.getElementById(id);
            if (element) {
                element.textContent = isRunning ? '运行中' : '已停止';
                element.className = 'status-value ' + (isRunning ? 'status-running' : 'status-stopped');
            }
        };

        setStatusIfExists('monitor-status', status.is_running);
        setTextIfExists('active-symbols', status.active_symbols ? status.active_symbols.length : 0);
        setTextIfExists('total-signals', status.total_signals || 0);
        setTextIfExists('total-orders', status.total_orders || 0);
        setTextIfExists('total-contracts', status.total_contracts || 0);
    } catch (error) {
        console.error('Failed to load status:', error);
    }
}

// 重写switchTab函数，专门处理dashboard的标签页
function switchTab(tabName) {
    // 隐藏所有标签内容
    document.querySelectorAll('.tab-content').forEach(tab => {
        tab.classList.remove('active');
    });

    // 移除所有按钮的active状态
    document.querySelectorAll('.tab-button').forEach(btn => {
        btn.classList.remove('active');
    });

    // 显示选中的标签内容
    const targetTab = document.getElementById(tabName + '-tab');
    if (targetTab) {
        targetTab.classList.add('active');
    }

    // 激活对应的按钮 - 通过事件源获取
    event.target.classList.add('active');

    // 根据标签页加载对应数据
    if (tabName === 'signals') {
        loadSignals();
    } else if (tabName === 'orders') {
        loadOrders();
    }
}

// 加载信号记录
async function loadSignals() {
    try {
        const signals = await apiRequest('/api/signals');

        const tbody = document.getElementById('signals-tbody');
        if (!tbody) return;

        if (signals.length === 0) {
            tbody.innerHTML = '<tr><td colspan="10" class="no-data">暂无信号记录</td></tr>';
            return;
        }

        tbody.innerHTML = signals.map(signal => `
            <tr>
                <td>${formatTime(signal.timestamp * 1000)}</td>
                <td>${signal.symbol}</td>
                <td class="${signal.shadow_type === 'lower' ? 'signal-buy' : 'signal-sell'}">${signal.shadow_type}</td>
                <td>${signal.interval_type}</td>
                <td>${formatPrice(signal.close_price)}</td>
                <td>${formatNumber(signal.shadow_ratio)}</td>
                <td>${formatNumber(signal.main_shadow_length / signal.body_length)}</td>
                <td>${formatNumber(signal.main_shadow_length)}</td>
                <td>${formatNumber(signal.avg_volume)}</td>
                <td>${signal.candle_type}</td>
            </tr>
        `).join('');
    } catch (error) {
        console.error('Failed to load signals:', error);
        const tbody = document.getElementById('signals-tbody');
        if (tbody) {
            tbody.innerHTML = '<tr><td colspan="10" class="no-data">加载失败</td></tr>';
        }
    }
}

// 加载交易记录
async function loadOrders() {
    try {
        const orders = await apiRequest('/api/orders');

        const tbody = document.getElementById('orders-tbody');
        if (!tbody) return;

        if (orders.length === 0) {
            tbody.innerHTML = '<tr><td colspan="7" class="no-data">暂无交易记录</td></tr>';
            return;
        }

        tbody.innerHTML = orders.map(order => `
            <tr>
                <td>${formatTime(order.timestamp * 1000)}</td>
                <td>${order.symbol}</td>
                <td class="${order.side === 'buy' ? 'signal-buy' : 'signal-sell'}">${order.side.toUpperCase()}</td>
                <td>${formatNumber(order.order_size)}</td>
                <td>${formatPrice(order.entry_price)}</td>
                <td>${formatPrice(order.take_profit_price)}</td>
                <td>${formatPrice(order.stop_loss_price)}</td>
            </tr>
        `).join('');
    } catch (error) {
        console.error('Failed to load orders:', error);
        const tbody = document.getElementById('orders-tbody');
        if (tbody) {
            tbody.innerHTML = '<tr><td colspan="7" class="no-data">加载失败</td></tr>';
        }
    }
}

// 拉取合约数据
async function fetchContracts() {
    try {
        console.log('正在拉取合约数据...');

        // 创建 AbortController 用于超时控制
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 60000); // 1分钟超时

        const result = await apiRequest('/api/contracts/fetch', {
            method: 'POST',
            signal: controller.signal
        });

        clearTimeout(timeoutId); // 清除超时定时器

        // 存放在local storage
        localStorage.setItem('gate_contracts', JSON.stringify(result.data));
        console.log(`合约数据拉取成功！共获取${result.count}个合约`);
    } catch (error) {
        if (error.name === 'AbortError') {
            console.log('拉取失败: 请求超时（1分钟）');
        } else {
            console.log('拉取失败: ' + error.message);
        }
    }
}

// 页面初始化
document.addEventListener('DOMContentLoaded', function() {
    // 加载初始数据
    loadStatus();
    loadSignals();
    fetchContracts();

    // 每30秒刷新一次状态和数据
    setInterval(() => {
        loadStatus();
        // 根据当前活跃的标签页刷新对应数据
        const activeTab = document.querySelector('.tab-content.active');
        if (activeTab) {
            if (activeTab.id === 'signals-tab') {
                loadSignals();
            } else if (activeTab.id === 'orders-tab') {
                loadOrders();
            }
        }
    }, 30000);
});
