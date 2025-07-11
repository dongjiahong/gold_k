
# gold K

## 使用方法
### 1. mac or linux
```bash
mkdir -p ./data
touch -c ./data/gold_k.sqlite
touch 'database_url="./data/gold_k.sqlite"' > app.toml
gold_k -c app.toml web
# open http://localhost:3000
```

### 2. windows
1. 在release中下载最新的可执行文件如：gold_k-windows-amd64.exe
2. 下载启动脚本并放入同一目录 start_gold.bat
3. 双击启动脚本
4. 使用tampermonkey脚本`get_cookie.js`来获取gate.io的登录cookie
5. 填入并运行