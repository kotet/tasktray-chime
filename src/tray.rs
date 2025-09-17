use anyhow::{Context, Result};
use tokio::sync::mpsc;
use tray_icon::{
    menu::{Menu, MenuItem, PredefinedMenuItem, MenuId},
    TrayIcon, TrayIconBuilder,
};
use tray_icon::menu::MenuEvent;
use std::path::Path;

pub struct SystemTray {
    tray_icon: TrayIcon,
    menu_event_receiver: mpsc::UnboundedReceiver<TrayMenuEvent>,
    // 固定メニューID
    toggle_autostart_id: MenuId,
    open_config_id: MenuId,
    open_logs_id: MenuId,
    exit_id: MenuId,
    // シャットダウン用チャンネル
    shutdown_tx: mpsc::UnboundedSender<()>,
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
        // 固定IDを作成
        let toggle_autostart_id = MenuId::new("toggle_autostart");
        let open_config_id = MenuId::new("open_config");
        let open_logs_id = MenuId::new("open_logs");
        let exit_id = MenuId::new("exit");
        
        // 自動起動の現在の状態を確認
        let autostart_enabled = Self::check_autostart_status();
        let autostart_text = if autostart_enabled {
            "自動起動を無効化 (現在: 有効)"
        } else {
            "自動起動を有効化 (現在: 無効)"
        };
        
        // 固定IDを使用してメニューアイテムを作成
        let toggle_autostart = MenuItem::with_id(toggle_autostart_id.clone(), autostart_text, true, None);
        let separator1 = PredefinedMenuItem::separator();
        let open_config = MenuItem::with_id(open_config_id.clone(), "設定ファイルを開く", true, None);
        let open_logs = MenuItem::with_id(open_logs_id.clone(), "ログディレクトリを開く", true, None);
        let separator2 = PredefinedMenuItem::separator();
        let exit = MenuItem::with_id(exit_id.clone(), "終了", true, None);

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
            .with_icon(Self::create_tray_icon())
            .build()
            .context("Failed to create tray icon")?;

        // メニューイベント処理用のチャンネル
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        
        // シャットダウン用チャンネル
        let (shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();

        // メニューイベントリスナーを設定
        let event_tx_clone = event_tx.clone();
        let toggle_autostart_id_clone = toggle_autostart_id.clone();
        let open_config_id_clone = open_config_id.clone();
        let open_logs_id_clone = open_logs_id.clone();
        let exit_id_clone = exit_id.clone();
        
        // メニューイベント処理用のタスクを起動
        tokio::spawn(async move {
            let menu_channel = MenuEvent::receiver();
            
            loop {
                // シャットダウンシグナルをチェック
                if shutdown_rx.try_recv().is_ok() {
                    tracing::debug!("Menu event listener received shutdown signal");
                    break;
                }
                
                // ノンブロッキングでメニューイベントをチェック
                match menu_channel.try_recv() {
                    Ok(event) => {
                        tracing::debug!("Received menu event: {:?}", event.id);
                        
                        let menu_event = if event.id == toggle_autostart_id_clone {
                            TrayMenuEvent::ToggleAutoStart
                        } else if event.id == open_config_id_clone {
                            TrayMenuEvent::OpenConfig
                        } else if event.id == open_logs_id_clone {
                            TrayMenuEvent::OpenLogsDir
                        } else if event.id == exit_id_clone {
                            TrayMenuEvent::Exit
                        } else {
                            tracing::warn!("Unknown menu item clicked: {:?}", event.id);
                            continue;
                        };

                        if let Err(_) = event_tx_clone.send(menu_event) {
                            tracing::warn!("Failed to send tray menu event - channel closed");
                            break;
                        }
                    }
                    Err(_) => {
                        // イベントがない場合は少し待機
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                }
            }
            tracing::debug!("Menu event listener task terminated");
        });

        Ok(Self {
            tray_icon,
            menu_event_receiver: event_rx,
            toggle_autostart_id,
            open_config_id,
            open_logs_id,
            exit_id,
            shutdown_tx,
        })
    }

    /// メニューを現在の自動起動状態に基づいて更新
    pub fn update_menu(&mut self) -> Result<()> {
        let autostart_enabled = Self::check_autostart_status();
        let autostart_text = if autostart_enabled {
            "自動起動を無効化 (現在: 有効)"
        } else {
            "自動起動を有効化 (現在: 無効)"
        };
        
        // 固定IDを使用して新しいメニューを作成
        let toggle_autostart = MenuItem::with_id(
            self.toggle_autostart_id.clone(), 
            autostart_text, 
            true, 
            None
        );
        let separator1 = PredefinedMenuItem::separator();
        let open_config = MenuItem::with_id(
            self.open_config_id.clone(),
            "設定ファイルを開く", 
            true, 
            None
        );
        let open_logs = MenuItem::with_id(
            self.open_logs_id.clone(),
            "ログディレクトリを開く", 
            true, 
            None
        );
        let separator2 = PredefinedMenuItem::separator();
        let exit = MenuItem::with_id(
            self.exit_id.clone(),
            "終了", 
            true, 
            None
        );

        let menu = Menu::with_items(&[
            &toggle_autostart,
            &separator1,
            &open_config,
            &open_logs,
            &separator2,
            &exit,
        ])
        .context("Failed to create updated tray menu")?;

        // メニューを更新
        self.tray_icon.set_menu(Some(Box::new(menu)));
        
        tracing::debug!("Updated tray menu with autostart status: {}", autostart_enabled);
        Ok(())
    }

    /// メニューイベントを受信
    pub async fn recv_menu_event(&mut self) -> Option<TrayMenuEvent> {
        self.menu_event_receiver.recv().await
    }
    
    /// システムトレイのシャットダウン処理
    pub fn shutdown(&self) {
        // メニューイベントリスナータスクにシャットダウンシグナルを送信
        let _ = self.shutdown_tx.send(());
        tracing::debug!("Sent shutdown signal to menu event listener");
    }

    /// タスクトレイアイコンを作成（組み込みアイコンファイル使用）
    fn create_tray_icon() -> tray_icon::Icon {
        // 組み込みアイコンデータを使用
        let icon_bytes = include_bytes!("../resources/icons/chime-icon-32.ico");
        
        // ICOファイルからアイコンを作成を試行
        match image::load_from_memory_with_format(icon_bytes, image::ImageFormat::Ico) {
            Ok(img) => {
                let rgba_img = img.to_rgba8();
                let (width, height) = rgba_img.dimensions();
                let rgba_data = rgba_img.into_raw();
                
                match tray_icon::Icon::from_rgba(rgba_data, width, height) {
                    Ok(icon) => {
                        tracing::debug!("Successfully loaded tray icon from embedded resource");
                        return icon;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create RGBA icon: {}. Using fallback.", e);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to decode embedded icon: {}. Using fallback.", e);
            }
        }

        // フォールバック: デフォルトアイコンを作成
        Self::create_fallback_icon()
    }

    /// フォールバックアイコンを作成（シンプルな鐘のような形状）
    fn create_fallback_icon() -> tray_icon::Icon {
        // 16x16のアイコンデータ（RGBA形式）
        // より明確な鐘の形状を作成
        let mut icon_data = vec![0u8; 1024]; // 16x16 * 4 (RGBA)
        
        // アイコンを描画する関数
        let mut set_pixel = |x: usize, y: usize, r: u8, g: u8, b: u8, a: u8| {
            if x < 16 && y < 16 {
                let idx = (y * 16 + x) * 4;
                if idx + 3 < icon_data.len() {
                    icon_data[idx] = r;     // Red
                    icon_data[idx + 1] = g; // Green  
                    icon_data[idx + 2] = b; // Blue
                    icon_data[idx + 3] = a; // Alpha
                }
            }
        };

        // 白い鐘の形状を描画
        for y in 0..16 {
            for x in 0..16 {
                let center_x = 8.0;
                let center_y = 8.0;
                let dx = x as f32 - center_x;
                let dy = y as f32 - center_y;
                let distance = (dx * dx + dy * dy).sqrt();
                
                // 鐘の外形
                if distance <= 6.0 && distance >= 3.0 {
                    set_pixel(x, y, 255, 255, 255, 255); // 白色
                } else if distance <= 7.0 && distance >= 6.0 {
                    set_pixel(x, y, 200, 200, 200, 180); // 薄い白
                }
                
                // 鐘の中心部
                if distance <= 1.5 {
                    set_pixel(x, y, 255, 200, 100, 255); // 薄い黄色
                }
            }
        }

        tray_icon::Icon::from_rgba(icon_data, 16, 16)
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to create icon from data: {}. Creating fallback.", e);
                // フォールバック: シンプルな白い四角
                let mut fallback_data = vec![0u8; 1024];
                for i in (0..1024).step_by(4) {
                    if i + 3 < fallback_data.len() {
                        fallback_data[i] = 255;     // Red
                        fallback_data[i + 1] = 255; // Green
                        fallback_data[i + 2] = 255; // Blue
                        fallback_data[i + 3] = 128; // Alpha
                    }
                }
                tray_icon::Icon::from_rgba(fallback_data, 16, 16)
                    .expect("Failed to create fallback icon")
            })
    }

    /// 自動起動の状態を設定
    pub fn set_autostart_status(&mut self, enabled: bool) -> Result<()> {
        let result = {
            #[cfg(target_os = "windows")]
            {
                self.set_windows_autostart_status(enabled)
            }
            
            #[cfg(not(target_os = "windows"))]
            {
                let _ = enabled; // 未使用変数警告を回避
                tracing::info!("Autostart setting not implemented for this platform");
                Ok(())
            }
        };

        // 状態変更後にメニューを更新
        if result.is_ok() {
            if let Err(e) = self.update_menu() {
                tracing::warn!("Failed to update menu after autostart status change: {}", e);
            }
        }

        result
    }

    #[cfg(target_os = "windows")]
    fn set_windows_autostart_status(&self, enabled: bool) -> Result<()> {
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

    /// 自動起動の現在状態を取得（静的メソッド）
    fn check_autostart_status() -> bool {
        #[cfg(target_os = "windows")]
        {
            Self::check_windows_autostart_status().unwrap_or(false)
        }
        
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    /// 自動起動の現在状態を取得
    pub fn get_autostart_status(&self) -> bool {
        Self::check_autostart_status()
    }

    #[cfg(target_os = "windows")]
    fn check_windows_autostart_status() -> Result<bool> {
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
        
        // 絶対パスに変換
        let absolute_path = config_path.canonicalize()
            .or_else(|_| {
                // canonicalize が失敗した場合（まだ存在しないファイル等）、手動で絶対パス化
                std::env::current_dir()
                    .map(|cwd| cwd.join(config_path))
                    .context("Failed to get current directory")
            })?;
        
        #[cfg(target_os = "windows")]
        {
            // Windows: notepad を直接実行
            std::process::Command::new("notepad.exe")
                .arg(&absolute_path)
                .spawn()
                .context("Failed to open config file with notepad.exe")?;
        }
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(&absolute_path)
                .spawn()
                .context("Failed to open config file")?;
        }
        
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&absolute_path)
                .spawn()
                .context("Failed to open config file")?;
        }

        tracing::info!("Opened config file: {:?}", absolute_path);
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
        
        // 絶対パスに変換
        let absolute_path = logs_path.canonicalize()
            .or_else(|_| {
                // canonicalize が失敗した場合（まだ存在しないパス等）、手動で絶対パス化
                std::env::current_dir()
                    .map(|cwd| cwd.join(logs_path))
                    .context("Failed to get current directory")
            })?;
        
        #[cfg(target_os = "windows")]
        {
            // Windows: explorer.exe を直接実行
            std::process::Command::new("explorer.exe")
                .arg(&absolute_path)
                .spawn()
                .context("Failed to open logs directory with explorer.exe")?;
        }
        
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(&absolute_path)
                .spawn()
                .context("Failed to open logs directory")?;
        }
        
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&absolute_path)
                .spawn()
                .context("Failed to open logs directory")?;
        }

        tracing::info!("Opened logs directory: {:?}", absolute_path);
        Ok(())
    }
}