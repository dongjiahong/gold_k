set RUST_LOG=info
set SERVER_LOG=.\logs\run.log
.\gold_k-windows-amd64.exe -c app.toml web
timeout /t 2 /nobreak > null
start "" http://localhost:3000