// ==UserScript==
// @name         Gate.io 获取cookie
// @namespace    http://tampermonkey.net/
// @version      0.1
// @description  从Gate.io获取登陆后的cookie
// @author       You
// @match        https://www.gate.com/*
// @grant        GM_setClipboard
// ==/UserScript==

(function() {
    'use strict';

    // 创建按钮元素
    const button = document.createElement('button');
    button.textContent = '获取Cookie';
    button.style.cssText = `
        position: fixed;
        top: 60px;
        right: 20px;
        z-index: 9999;
        padding: 10px 15px;
        background-color: #007bff;
        color: white;
        border: none;
        border-radius: 5px;
        cursor: pointer;
        font-size: 8px;
        box-shadow: 0 2px 5px rgba(0,0,0,0.2);
    `;

    // 按钮点击事件
    button.addEventListener('click', function() {
        // 获取当前页面的cookie
        const cookie = document.cookie;
        
        if (cookie) {
            // 检查是否包含csrftoken字段
            const hasCSRFToken = cookie.includes('csrftoken=');
            
            if (!hasCSRFToken) {
                alert('未检测到登录状态！\n请先登录Gate.io账户后再获取Cookie。');
                return;
            }
            
            // 复制到剪贴板
            GM_setClipboard(cookie);
            
            // 显示对话框
            alert('Cookie已复制到剪贴板！\n\n' + cookie);
        } else {
            alert('未找到Cookie信息');
        }
    });

    // 将按钮添加到页面
    document.body.appendChild(button);

    // 鼠标悬停效果
    button.addEventListener('mouseenter', function() {
        button.style.backgroundColor = '#0056b3';
    });

    button.addEventListener('mouseleave', function() {
        button.style.backgroundColor = '#007bff';
    });

})();