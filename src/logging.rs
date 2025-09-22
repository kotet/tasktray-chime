use anyhow::Result;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use std::path::Path;
use crate::config::LoggingConfig;

pub fn init_logging(config: &LoggingConfig) -> Result<()> {
    // ログディレクトリを作成
    std::fs::create_dir_all(&config.directory)?;

    // ログレベルをパース
    let level = match config.level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };


    // ファイルアペンダーの設定
    let file_appender = if config.rotate {
        RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("tasktray-chime")
            .filename_suffix("log")
            .max_log_files(config.max_files as usize)
            .build(&config.directory)?
    } else {
        RollingFileAppender::builder()
            .rotation(Rotation::NEVER)
            .filename_prefix("tasktray-chime")
            .filename_suffix("log")
            .build(&config.directory)?
    };

    // フォーマッタの設定
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true);

    // コンソール出力レイヤー（開発時用）
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_ansi(true)
        .with_target(false);

    // 環境フィルター
    let env_filter = EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy()
        .add_directive("tasktray_chime=trace".parse().unwrap());

    // サブスクライバーを初期化
    tracing_subscriber::registry()
        .with(file_layer.with_filter(env_filter.clone()))
        .with(console_layer.with_filter(env_filter))
        .init();

    tracing::info!("Logging initialized with level: {}", config.level);
    tracing::info!("Log directory: {}", config.directory);
    tracing::info!("Log rotation: {}", config.rotate);

    Ok(())
}

/// ログディレクトリをクリーンアップ（古いファイルを削除）
pub fn cleanup_old_logs(config: &LoggingConfig) -> Result<()> {
    let log_dir = Path::new(&config.directory);
    if !log_dir.exists() {
        return Ok(());
    }

    let mut log_files: Vec<_> = std::fs::read_dir(log_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.path().is_file() && 
            entry.path().extension().map_or(false, |ext| ext == "log")
        })
        .collect();

    // 更新日時でソート（新しい順）
    log_files.sort_by_key(|entry| {
        entry.metadata().ok()
            .and_then(|meta| meta.modified().ok())
            .unwrap_or_else(|| std::time::SystemTime::UNIX_EPOCH)
    });
    log_files.reverse();

    // 保持ファイル数を超えたファイルを削除
    if log_files.len() > config.max_files as usize {
        for file_entry in log_files.iter().skip(config.max_files as usize) {
            let path = file_entry.path();
            match std::fs::remove_file(&path) {
                Ok(()) => tracing::info!("Removed old log file: {:?}", path),
                Err(e) => tracing::warn!("Failed to remove old log file {:?}: {}", path, e),
            }
        }
    }

    Ok(())
}