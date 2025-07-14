use std::env;
use std::fs::OpenOptions;
use time::{UtcOffset, macros::format_description};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{self, EnvFilter, fmt, fmt::time::OffsetTime};

pub fn init_tracing() {
    let local_time = OffsetTime::new(
        UtcOffset::from_hms(8, 0, 0).unwrap(),
        format_description!("[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:3]"),
    );

    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    // 创建控制台输出层
    let console_layer = fmt::layer()
        .with_timer(local_time.clone())
        .with_line_number(true)
        .with_file(true);

    // 检查是否设置了 SERVER_LOG 环境变量
    if let Ok(log_file_path) = env::var("SERVER_LOG") {
        // 创建或打开日志文件
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
            .unwrap_or_else(|e| panic!("无法打开日志文件 {}: {}", log_file_path, e));

        // 创建文件输出层
        let file_layer = fmt::layer()
            .with_timer(local_time)
            .with_line_number(true)
            .with_file(true)
            .with_writer(file)
            .with_ansi(false); // 文件中不需要颜色代码

        // 同时输出到控制台和文件
        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .init();

        tracing::info!("日志将输出到控制台和文件: {}", log_file_path);
    } else {
        // 只输出到控制台
        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .init();

        tracing::info!("日志仅输出到控制台");
    }
}
