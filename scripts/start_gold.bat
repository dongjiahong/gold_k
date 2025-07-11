@echo off
REM 创建data目录
if not exist data mkdir data

REM 创建数据库文件（仅当不存在时）
if not exist data\gold_k.sqlite (
    echo Creating database file...
    type nul > data\gold_k.sqlite
) else (
    echo Database file already exists, skipping...
)

REM 创建配置文件（仅当不存在时）
if not exist app.toml (
    echo Creating config file...
    echo database_url="./data/gold_k.sqlite" > app.toml
) else (
    echo Config file already exists, skipping...
)

REM 启动应用程序
start "" gold_k-windows-amd64.exe -c app.toml web

REM 等待一下让服务启动
timeout /t 3 /nobreak >nul

REM 打开浏览器
start "" http://localhost:3000

echo Gold_k application started and browser opened!
pause