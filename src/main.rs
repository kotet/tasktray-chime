#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod logging;
mod audio;
mod scheduler;
mod tray;

use anyhow::{Context, Result};
use std::sync::Arc;
use tracing::{info, error, warn};
use directories::ProjectDirs;

use config::Config;
use audio::AudioPlayer;
use scheduler::CronScheduler;
use tray::{SystemTray, TrayMenuEvent};

#[cfg(target_os = "windows")]
mod windows_utils {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    use std::mem;

    pub fn pump_messages_non_blocking() -> bool {
        unsafe {
            let mut msg: MSG = mem::zeroed();
            
            // 複数のメッセージを一度に処理（最大10件）
            let mut processed = 0;
            let max_messages = 10;
            
            while processed < max_messages && PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
                processed += 1;
            }
            
            processed > 0
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 設定ファイルを読み込み（存在しない場合は作成）
    let config_path = if let Some(proj_dirs) = ProjectDirs::from("com", "tasktray-chime", "tasktray-chime") {
        let config_dir = proj_dirs.config_dir();
        std::fs::create_dir_all(&config_dir).context("Failed to create config directory")?;
        config_dir.join("config.yaml")
    } else {
        // フォールバック: カレントディレクトリ
        std::path::PathBuf::from("config.yaml")
    };
    
    let config = Config::load_or_create_default(&config_path)
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

    // 音声プレイヤーを初期化
    let audio_player = Arc::new(
        AudioPlayer::new(&config.audio)
            .context("Failed to initialize audio player")?
    );

    // 音声ファイルを事前にロード
    for schedule in &config.schedules {
        if schedule.enabled {
            info!("Preloading audio file: {}", schedule.file);
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

    // 初期化後にメニューを更新して正確な自動起動状態を表示
    if let Err(e) = system_tray.update_menu() {
        warn!("Failed to update tray menu after initialization: {}", e);
    }

    // シャットダウンチャンネル
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
    
    // スケジュールイベント処理用のワーカータスク
    let shutdown_tx_clone = shutdown_tx;
    tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(event) = schedule_events.recv() => {
                    info!("Schedule '{}' executed at {}", 
                          event.schedule_id, 
                          event.triggered_at.format("%Y-%m-%d %H:%M:%S"));
                }
                
                _ = &mut shutdown_rx => {
                    info!("Schedule event handler received shutdown signal");
                    break;
                }
            }
        }
        info!("Schedule event handler task terminated");
    });

    info!("All systems initialized, entering main event loop");

    // メインイベントループ（トレイイベント処理に専念）
    loop {
        // Windows環境: 効率的なメッセージポンプ
        #[cfg(target_os = "windows")]
        {
            windows_utils::pump_messages_non_blocking();
        }

        // トレイメニューイベントを短いタイムアウトで処理
        if let Some(event) = system_tray.recv_menu_event_with_timeout(50).await {
            info!("Received tray menu event: {:?}", event);
            match handle_tray_event(event, &mut system_tray, &config).await {
                Ok(should_exit) => {
                    if should_exit {
                        info!("Exit requested from tray menu");
                        break;
                    }
                }
                Err(e) => error!("Error handling tray event: {}", e),
            }
            continue;
        }
        
        // Ctrl+C シグナルをチェック
        if tokio::select! {
            _ = tokio::signal::ctrl_c() => { true }
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(10)) => { false }
        } {
            info!("Received shutdown signal");
            break;
        }
        
        // 短いスリープでCPU使用率を抑制
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // 終了処理
    info!("Shutting down application");
    
    // ワーカータスクにシャットダウンシグナルを送信
    let _ = shutdown_tx_clone.send(());
    
    // システムトレイの終了処理
    system_tray.shutdown();
    
    // スケジューラーの終了処理
    scheduler.stop();
    
    // バックグラウンドタスクが終了するまで待機
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    
    info!("Application shutdown complete");
    
    // 確実にプロセスを終了
    std::process::exit(0);
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
            
            // システムトレイの終了処理
            system_tray.shutdown();
            
            // 確実にプロセスを終了
            std::process::exit(0);
        }
    }
}
