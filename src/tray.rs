use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::sync::mpsc;
use tray_icon::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};
use tray_icon::menu::MenuEvent;
use std::path::Path;

pub struct SystemTray {
    _tray_icon: TrayIcon,
    menu_event_receiver: mpsc::UnboundedReceiver<TrayMenuEvent>,
}

#[derive(Debug, Clone)]
pub enum TrayMenuEvent {
    ToggleAutoStart,
    OpenConfig,
    OpenLogsDir,
    Exit,
}

impl SystemTray {
    pub fn new() -> Result<Self> {
        // メニューアイテムを作成
        let toggle_autostart = MenuItem::new("自動起動切替", true, None);
        let separator1 = PredefinedMenuItem::separator();
        let open_config = MenuItem::new("設定ファイルを開く", true, None);
        let open_logs = MenuItem::new("ログディレクトリを開く", true, None);
        let separator2 = PredefinedMenuItem::separator();
        let exit = MenuItem::new("終了", true, None);

        // コンテキストメニューを構築
        let menu = Menu::with_items(&[
            &toggle_autostart,
            &separator1,
            &open_config,
            &open_logs,
            &separator2,
            &exit,
        ])
        .context("Failed to create tray menu")?;

        // トレイアイコンを作成
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Tasktray Chime - 時報アプリ")
            .with_icon(Self::create_default_icon())
            .build()
            .context("Failed to create tray icon")?;

        // メニューイベント処理用のチャンネル
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // メニューイベントリスナーを設定
        let event_tx_clone = event_tx.clone();
        let toggle_autostart_id = toggle_autostart.id().0.clone();
        let open_config_id = open_config.id().0.clone();
        let open_logs_id = open_logs.id().0.clone();
        let exit_id = exit.id().0.clone();
        
        std::thread::spawn(move || {
            let menu_channel = MenuEvent::receiver();
            
            while let Ok(event) = menu_channel.recv() {
                let menu_event = match event.id.0.as_str() {
                    id if id == toggle_autostart_id => TrayMenuEvent::ToggleAutoStart,
                    id if id == open_config_id => TrayMenuEvent::OpenConfig,
                    id if id == open_logs_id => TrayMenuEvent::OpenLogsDir,
                    id if id == exit_id => TrayMenuEvent::Exit,
                    _ => continue,
                };

                if let Err(_) = event_tx_clone.send(menu_event) {
                    tracing::warn!("Failed to send tray menu event");
                    break;
                }
            }
        });

        Ok(Self {
            _tray_icon: tray_icon,
            menu_event_receiver: event_rx,
        })
    }

    /// メニューイベントを受信
    pub async fn recv_menu_event(&mut self) -> Option<TrayMenuEvent> {
        self.menu_event_receiver.recv().await
    }

    /// メニューイベントを非ブロッキングで受信
    pub fn try_recv_menu_event(&mut self) -> Option<TrayMenuEvent> {
        self.menu_event_receiver.try_recv().ok()
    }

    /// デフォルトのアイコンを作成（シンプルなドット）
    fn create_default_icon() -> tray_icon::Icon {
        // 16x16のアイコンデータ（RGBA形式）
        let icon_data = vec![
            // 透明な背景に白い円
            0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     255, 255, 255, 128,
            255, 255, 255, 192, 255, 255, 255, 128, 0, 0, 0, 0,     0, 0, 0, 0,
            0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
            0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
            
            0, 0, 0, 0,     0, 0, 0, 0,     255, 255, 255, 128, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 128, 0, 0, 0, 0,
            0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
            0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
            
            0, 0, 0, 0,     255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 128,
            0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
            0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
            
            255, 255, 255, 128, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
            255, 255, 255, 128, 0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
            0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,     0, 0, 0, 0,
        ];

        // 必要なサイズまでパディング（16x16 = 256要素必要）
        let mut full_icon_data = Vec::with_capacity(256);
        for i in 0..256 {
            if i < icon_data.len() {
                full_icon_data.push(icon_data[i]);
            } else {
                full_icon_data.push(0);
            }
        }

        tray_icon::Icon::from_rgba(full_icon_data, 16, 16)
            .unwrap_or_else(|_| {
                // フォールバック: 完全に透明なアイコン
                let transparent_data = vec![0; 64]; // 16x16 RGBA
                tray_icon::Icon::from_rgba(transparent_data, 16, 16)
                    .expect("Failed to create fallback icon")
            })
    }

    /// 自動起動の状態を設定
    pub fn set_autostart_status(&mut self, _enabled: bool) -> Result<()> {
        // TODO: Windowsレジストリへの自動開始設定を実装
        tracing::info!("Autostart status update requested (not implemented)");
        Ok(())
    }

    /// 自動起動の現在状態を取得
    pub fn get_autostart_status(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            self.get_windows_autostart_status().unwrap_or(false)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    #[cfg(target_os = "windows")]
    fn set_windows_autostart(&self, enabled: bool) -> Result<()> {
        use std::process::Command;
        
        let exe_path = std::env::current_exe()
            .context("Failed to get current executable path")?;
        
        if enabled {
            let output = Command::new("reg")
                .args(&[
                    "add",
                    "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v", "TasktrayChime",
                    "/t", "REG_SZ",
                    "/d", &exe_path.to_string_lossy(),
                    "/f"
                ])
                .output()
                .context("Failed to execute reg command for adding autostart")?;
            
            if !output.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to enable autostart: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            
            tracing::info!("Autostart enabled");
        } else {
            let output = Command::new("reg")
                .args(&[
                    "delete",
                    "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                    "/v", "TasktrayChime",
                    "/f"
                ])
                .output()
                .context("Failed to execute reg command for removing autostart")?;
            
            // 削除は失敗してもよい（エントリが存在しない場合）
            if output.status.success() {
                tracing::info!("Autostart disabled");
            } else {
                tracing::info!("Autostart was not enabled");
            }
        }

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn get_windows_autostart_status(&self) -> Result<bool> {
        use std::process::Command;
        
        let output = Command::new("reg")
            .args(&[
                "query",
                "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v", "TasktrayChime"
            ])
            .output()
            .context("Failed to query registry for autostart status")?;
        
        Ok(output.status.success())
    }

    /// 設定ファイルを開く
    pub fn open_config_file() -> Result<()> {
        let config_path = Path::new("config.yaml");
        
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(&["/C", "start", "notepad", &config_path.to_string_lossy()])
                .spawn()
                .context("Failed to open config file")?;
        }
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(config_path)
                .spawn()
                .context("Failed to open config file")?;
        }
        
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(config_path)
                .spawn()
                .context("Failed to open config file")?;
        }

        tracing::info!("Opened config file: {:?}", config_path);
        Ok(())
    }

    /// ログディレクトリを開く
    pub fn open_logs_directory(logs_dir: &str) -> Result<()> {
        let logs_path = Path::new(logs_dir);
        
        // ディレクトリが存在しない場合は作成
        if !logs_path.exists() {
            std::fs::create_dir_all(logs_path)
                .context("Failed to create logs directory")?;
        }
        
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(&["/C", "start", "explorer", &logs_path.to_string_lossy()])
                .spawn()
                .context("Failed to open logs directory")?;
        }
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(logs_path)
                .spawn()
                .context("Failed to open logs directory")?;
        }
        
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(logs_path)
                .spawn()
                .context("Failed to open logs directory")?;
        }

        tracing::info!("Opened logs directory: {:?}", logs_path);
        Ok(())
    }
}