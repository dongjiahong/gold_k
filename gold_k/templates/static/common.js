// 通用JavaScript函数 - Gate.io K线监控工具

// 显示消息提示
function showMessage(message, type = 'success') {
    const container = document.getElementById('message-container');
    if (!container) {
        console.warn('消息容器未找到');
        return;
    }
    
    container.innerHTML = `<div class="message ${type}">${message}</div>`;
    setTimeout(() => {
        container.innerHTML = '';
    }, 5000);
}

// 创建消息容器（如果不存在）
function ensureMessageContainer() {
    let container = document.getElementById('message-container');
    if (!container) {
        container = document.createElement('div');
        container.id = 'message-container';
        
        // 在第一个section之前插入
        const firstSection = document.querySelector('.section');
        if (firstSection) {
            firstSection.parentNode.insertBefore(container, firstSection);
        } else {
            // 如果没有section，在container内的开头插入
            const mainContainer = document.querySelector('.container, .container-small');
            if (mainContainer) {
                mainContainer.insertBefore(container, mainContainer.firstChild);
            }
        }
    }
    return container;
}

// 从localStorage获取合约信息
function getContractInfo() {
    try {
        const contractsData = localStorage.getItem('gate_contracts');
        return contractsData ? JSON.parse(contractsData) : [];
    } catch (error) {
        console.warn('获取合约信息失败:', error);
        return [];
    }
}

// 验证交易对是否存在
function validateSymbol(symbol) {
    const contracts = getContractInfo();
    return contracts.find(contract => contract.name === symbol);
}

// 获取合约乘数用于计算订单价值
function getQuantoMultiplier(symbol) {
    const contract = validateSymbol(symbol);
    return contract ? parseFloat(contract.quanto_multiplier) : 1;
}

// 计算订单价值（张数 * 乘数）
function calculateOrderValue(symbol, orderSize) {
    const multiplier = getQuantoMultiplier(symbol);
    return (orderSize * multiplier).toFixed(4);
}

// 通用API请求函数
async function apiRequest(url, options = {}) {
    try {
        const response = await fetch(url, {
            headers: {
                'Content-Type': 'application/json',
                ...options.headers
            },
            ...options
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }

        const contentType = response.headers.get('content-type');
        if (contentType && contentType.includes('application/json')) {
            return await response.json();
        } else {
            return await response.text();
        }
    } catch (error) {
        console.error('API请求失败:', error);
        throw error;
    }
}

// 格式化时间
function formatTime(timestamp) {
    if (!timestamp) return '-';
    const date = new Date(timestamp);
    return date.toLocaleString('zh-CN', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
        hour: '2-digit',
        minute: '2-digit',
        second: '2-digit'
    });
}

// 格式化数字
function formatNumber(num, decimals = 2) {
    if (num === null || num === undefined) return '-';
    return parseFloat(num).toFixed(decimals);
}

// 格式化价格
function formatPrice(price, symbol = '') {
    if (!price) return '-';
    
    // 根据价格大小决定小数位数
    const num = parseFloat(price);
    if (num >= 1000) {
        return num.toFixed(2);
    } else if (num >= 1) {
        return num.toFixed(4);
    } else {
        return num.toFixed(6);
    }
}

// 页面初始化时确保消息容器存在
document.addEventListener('DOMContentLoaded', function() {
    ensureMessageContainer();
});


