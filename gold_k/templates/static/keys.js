// 密钥管理页面专用JavaScript - keys.js

// 加载当前配置
async function loadCurrentConfig() {
    const loadingEl = document.getElementById('loading');
    const configEl = document.getElementById('current-config');

    if (loadingEl) loadingEl.style.display = 'block';
    if (configEl) configEl.innerHTML = '';

    try {
        const config = await apiRequest('/api/keys/current');
        
        if (config) {
            // 填充表单
            const setValueIfExists = (id, value) => {
                const element = document.getElementById(id);
                if (element) element.value = value || '';
            };

            setValueIfExists('key-name', config.name);
            setValueIfExists('api-key', config.api_key);
            setValueIfExists('secret-key', config.secret_key);
            setValueIfExists('webhook-url', config.webhook_url);
            setValueIfExists('cookie', config.cookie);

            // 显示当前配置信息
            if (configEl) {
                configEl.innerHTML = `
                    <div class="key-item active">
                        <div class="key-item-header">
                            <div class="key-item-name">${config.name}</div>
                            <div class="key-item-status status-active">当前配置</div>
                        </div>
                        <div class="key-item-info">
                            API Key: ${config.api_key.substring(0, 10)}...<br>
                            ${config.webhook_url ? '钉钉通知: 已配置' : '钉钉通知: 未配置'}<br>
                            ${config.cookie ? 'Cookie: 已配置' : 'Cookie: 未配置'}<br>
                            ${config.contracts ? '合约数据: 已缓存' : '合约数据: 未缓存'}
                        </div>
                    </div>
                `;
            }
        } else {
            if (configEl) {
                configEl.innerHTML = '<p style="text-align: center; opacity: 0.7;">暂无配置，请添加API配置</p>';
            }
        }
    } catch (error) {
        if (configEl) {
            configEl.innerHTML = '<p style="text-align: center; opacity: 0.7;">暂无配置，请添加API配置</p>';
        }
        showMessage('加载配置失败: ' + error.message, 'error');
    } finally {
        if (loadingEl) loadingEl.style.display = 'none';
    }
}

// 保存配置
async function saveKeyConfig() {
    const formData = {
        name: document.getElementById('key-name')?.value || '',
        api_key: document.getElementById('api-key')?.value || '',
        secret_key: document.getElementById('secret-key')?.value || '',
        webhook_url: document.getElementById('webhook-url')?.value || null,
        cookie: document.getElementById('cookie')?.value || null
    };

    // 基本验证
    if (!formData.name || !formData.api_key || !formData.secret_key) {
        showMessage('请填写必要的配置信息', 'error');
        return;
    }

    try {
        await apiRequest('/api/keys', {
            method: 'POST',
            body: JSON.stringify(formData)
        });

        showMessage('配置保存成功！');
        loadCurrentConfig();
    } catch (error) {
        showMessage('保存失败: ' + error.message, 'error');
    }
}

// 拉取合约数据
async function fetchContracts() {
    try {
        showMessage('正在拉取合约数据...', 'success');
        const result = await apiRequest('/api/contracts/fetch', {
            method: 'POST'
        });

        showMessage(`合约数据拉取成功！共获取${result.count}个合约`);
        loadCurrentConfig();
    } catch (error) {
        showMessage('拉取失败: ' + error.message, 'error');
    }
}

// 测试钉钉通知
async function testDingTalk() {
    const webhookUrl = document.getElementById('webhook-url')?.value;
    if (!webhookUrl) {
        showMessage('请先输入钉钉Webhook URL', 'error');
        return;
    }

    try {
        const result = await apiRequest('/api/dingding/test');
        if (result.success) {
            showMessage('钉钉通知测试成功！');
        } else {
            showMessage('钉钉通知测试失败: ' + result.message, 'error');
        }
    } catch (error) {
        showMessage('测试失败: ' + error.message, 'error');
    }
}

// 页面初始化
document.addEventListener('DOMContentLoaded', function() {
    // 加载当前配置
    loadCurrentConfig();

    // 绑定表单提交事件
    const keyForm = document.getElementById('key-form');
    if (keyForm) {
        keyForm.addEventListener('submit', async (e) => {
            e.preventDefault();
            await saveKeyConfig();
        });
    }
});
