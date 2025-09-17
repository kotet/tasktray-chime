#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod logging;
mod audio;
mod scheduler;
mod tray;
mod test_audio;

use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::signal;
use tracing::{info, error, warn};

use config::Config;
use audio::AudioPlayer;
use scheduler::CronScheduler;
use tray::{SystemTray, TrayMenuEvent};

#[tokio::main]
async fn main() -> Result<()> {
    // 設定ファイルを読み込み（存在しない場合は作成）
    let config_path = "config.yaml";
    let config = Config::load_or_create_default(config_path)
        .context("Failed to load or create config file")?;

    // ログシステムを初期化
    logging::init_logging(&config.logging)
        .context("Failed to initialize logging system")?;

    // panicハンドラーを設定してpanicログもファイルに出力
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info.location().unwrap_or_else(|| {
            std::panic::Location::caller()
        });
        
        let msg = match panic_info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match panic_info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<dyn Any>",
            }
        };

        error!("PANIC occurred at {}:{} - {}", 
               location.file(), 
               location.line(), 
               msg);
        
        // 標準エラーにも出力
        eprintln!("PANIC: {} at {}:{}", msg, location.file(), location.line());
    }));

    info!("Starting Tasktray Chime application");

    // テスト用音声ファイルを生成（存在しない場合）
    let test_sound_path = "./sounds/tick.wav";
    if !std::path::Path::new(test_sound_path).exists() {
        info!("Generating test audio file: {}", test_sound_path);
        if let Err(e) = test_audio::generate_test_audio(test_sound_path) {
            warn!("Failed to generate test audio: {}", e);
        } else {
            info!("Test audio file generated successfully");
        }
    }

    // 音声プレイヤーを初期化
    let audio_player = Arc::new(
        AudioPlayer::new(&config.audio)
            .context("Failed to initialize audio player")?
    );

    // 音声ファイルを事前にロード
    for schedule in &config.schedules {
        if schedule.enabled {
            if let Err(e) = audio_player.preload_sound(&schedule.file) {
                error!("Failed to preload sound file '{}': {}", schedule.file, e);
            }
        }
    }

    // cronスケジューラーを初期化
    let mut scheduler = CronScheduler::new(audio_player.clone());
    for schedule in &config.schedules {
        if let Err(e) = scheduler.add_schedule(schedule.clone()) {
            error!("Failed to add schedule: {}", e);
        }
    }

    // システムトレイを初期化
    let mut system_tray = SystemTray::new()
        .context("Failed to initialize system tray")?;

    // スケジューラーを開始
    let mut schedule_events = scheduler.start().await
        .context("Failed to start cron scheduler")?;

    info!("All systems initialized, entering main event loop");

    // メインイベントループ
    loop {
        tokio::select! {
            // システム終了シグナル
            _ = signal::ctrl_c() => {
                info!("Received shutdown signal");
                break;
            }

            // トレイメニューイベント
            menu_event = system_tray.recv_menu_event() => {
                if let Some(event) = menu_event {
                    match handle_tray_event(event, &mut system_tray, &config).await {
                        Ok(should_exit) => {
                            if should_exit {
                                info!("Exit requested from tray menu");
                                break;
                            }
                        }
                        Err(e) => error!("Error handling tray event: {}", e),
                    }
                }
            }

            // スケジュールイベント
            schedule_event = schedule_events.recv() => {
                if let Some(event) = schedule_event {
                    info!("Schedule '{}' executed at {}", 
                          event.schedule_id, 
                          event.triggered_at.format("%Y-%m-%d %H:%M:%S"));
                }
            }

            // 定期的なメンテナンスタスク（5分毎）
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(300)) => {
                if let Err(e) = logging::check_log_size(&config.logging) {
                    warn!("Failed to check log size: {}", e);
                }
                if let Err(e) = logging::cleanup_old_logs(&config.logging) {
                    warn!("Failed to cleanup old logs: {}", e);
                }
            }
        }
    }

    // 終了処理
    info!("Shutting down application");
    scheduler.stop();
    
    // 少し待機してスケジューラーが確実に停止するまで待つ
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    info!("Application shutdown complete");
    Ok(())
}

/// トレイメニューイベントを処理
async fn handle_tray_event(
    event: TrayMenuEvent, 
    system_tray: &mut SystemTray,
    config: &Config
) -> Result<bool> {
    match event {
        TrayMenuEvent::ToggleAutoStart => {
            let current_status = system_tray.get_autostart_status();
            let new_status = !current_status;
            
            match system_tray.set_autostart_status(new_status) {
                Ok(()) => {
                    info!("Auto-start {} {}", 
                          if new_status { "enabled" } else { "disabled" },
                          if new_status { "✓" } else { "✗" });
                }
                Err(e) => {
                    error!("Failed to toggle auto-start: {}", e);
                }
            }
            Ok(false)
        }

        TrayMenuEvent::OpenConfig => {
            match SystemTray::open_config_file() {
                Ok(()) => info!("Opened config file"),
                Err(e) => error!("Failed to open config file: {}", e),
            }
            Ok(false)
        }

        TrayMenuEvent::OpenLogsDir => {
            match SystemTray::open_logs_directory(&config.logging.directory) {
                Ok(()) => info!("Opened logs directory"),
                Err(e) => error!("Failed to open logs directory: {}", e),
            }
            Ok(false)
        }

        TrayMenuEvent::Exit => {
            info!("Exit requested from tray menu");
            Ok(true) // 終了フラグを返す
        }
    }
}
