use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::{Context, Result};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    pub logging: LoggingConfig,
    pub audio: AudioConfig,
    pub schedules: Vec<Schedule>,
    pub behavior: BehaviorConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    pub level: String,
    pub directory: String,
    pub rotate: bool,
    pub max_files: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AudioConfig {
    pub global_volume: u8,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Schedule {
    pub id: String,
    #[serde(rename = "type")]
    pub schedule_type: String,
    pub cron: String,
    pub file: String,
    pub enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BehaviorConfig {
    pub retry_on_fail: u32,
    pub retry_delay_seconds: u64,
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {:?}", path.as_ref()))?;
        
        let config: Config = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {:?}", path.as_ref()))?;
        
        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = serde_yaml::to_string(self)
            .context("Failed to serialize config to YAML")?;
        
        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {:?}", path.as_ref()))?;
        
        Ok(())
    }

    /// デフォルトの設定を作成
    pub fn default() -> Self {
        // 実行ファイルと同じディレクトリ配下のlogsディレクトリをデフォルトとする
        let default_log_dir = if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                exe_dir.join("logs").to_string_lossy().to_string()
            } else {
                "./logs".to_string()
            }
        } else {
            // フォールバック: カレントディレクトリ
            "./logs".to_string()
        };

        Self {
            logging: LoggingConfig {
                level: "info".to_string(),
                directory: default_log_dir,
                rotate: true,
                max_files: 7,
            },
            audio: AudioConfig {
                global_volume: 80,
            },
            schedules: vec![
                Schedule {
                    id: "hourly_chime".to_string(),
                    schedule_type: "cron".to_string(),
                    cron: "0 * * * *".to_string(), // 毎時0分
                    file: "./audios/chime.wav".to_string(),
                    enabled: true,
                }
            ],
            behavior: BehaviorConfig {
                retry_on_fail: 0,
                retry_delay_seconds: 5,
            },
        }
    }

    /// 設定ファイルをロードし、存在しない場合はデフォルト設定を作成
    pub fn load_or_create_default<P: AsRef<Path>>(path: P) -> Result<Self> {
        if path.as_ref().exists() {
            Self::load_from_file(&path)
        } else {
            let config = Self::default();
            config.save_to_file(&path)
                .context("Failed to create default config file")?;
            tracing::info!("Created default config file at {:?}", path.as_ref());
            Ok(config)
        }
    }
}