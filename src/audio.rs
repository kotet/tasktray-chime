use anyhow::{Context, Result};
use rodio::{Decoder, OutputStream, OutputStreamBuilder, Sink};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::config::AudioConfig;

pub struct AudioPlayer {
    _stream: Arc<OutputStream>,
    preloaded_sounds: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    global_volume: Arc<Mutex<f32>>,
}

impl AudioPlayer {
    pub fn new(config: &AudioConfig) -> Result<Self> {
        let global_volume = (config.global_volume as f32) / 100.0;

        // OutputStreamBuilder を使用
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|e| anyhow::anyhow!("Failed to open default audio stream: {}", e))?;

        Ok(Self {
            _stream: Arc::new(stream),
            preloaded_sounds: Arc::new(Mutex::new(HashMap::new())),
            global_volume: Arc::new(Mutex::new(global_volume)),
        })
    }

    /// 音声ファイルを事前にメモリにロード
    pub fn preload_sound<P: AsRef<Path>>(&self, file_path: P) -> Result<()> {
        let path = file_path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        if !path.exists() {
            return Err(anyhow::anyhow!("Audio file not found: {:?}", path));
        }

        let audio_data = std::fs::read(path)
            .with_context(|| format!("Failed to read audio file: {:?}", path))?;

        // デコードテストを実行して有効な音声ファイルかチェック
        let cursor = std::io::Cursor::new(audio_data.clone());
        let _decoder = Decoder::new(cursor)
            .with_context(|| format!("Failed to decode audio file: {:?}", path))?;

        let mut preloaded = self.preloaded_sounds.lock().unwrap();
        preloaded.insert(path_str.clone(), audio_data);

        tracing::info!("Preloaded audio file: {:?}", path);
        Ok(())
    }

    /// 音声を非同期で再生（ブロッキングしない）
    pub async fn play_sound<P: AsRef<Path>>(&self, file_path: P) -> Result<()> {
        let path = file_path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        tracing::debug!("Attempting to play sound: {:?}", path);

        // 事前ロードされた音声データを取得
        let audio_data = {
            let preloaded = self.preloaded_sounds.lock().unwrap();
            preloaded.get(&path_str).cloned()
        };

        let audio_data = match audio_data {
            Some(data) => data,
            None => {
                // 事前ロードされていない場合はファイルから読み込み
                tracing::warn!("Audio file not preloaded, loading from disk: {:?}", path);
                std::fs::read(path)
                    .with_context(|| format!("Failed to read audio file: {:?}", path))?
            }
        };

        // 非同期タスクで再生実行
        let global_volume = *self.global_volume.lock().unwrap();
        let path_for_log = path_str.clone();
        let stream_ref = self._stream.clone();

        tokio::task::spawn_blocking(move || -> Result<()> {
            tracing::debug!("Starting audio playback task for: {}", path_for_log);
            
            // 既存のストリームを使用
            let sink = Sink::connect_new(&stream_ref.mixer());
            
            // デコーダーを作成
            let cursor = std::io::Cursor::new(audio_data);
            let decoder = Decoder::new(cursor)
                .with_context(|| format!("Failed to decode audio: {}", path_for_log))?;

            tracing::debug!("Setting volume to {} for: {}", global_volume, path_for_log);
            sink.set_volume(global_volume);
            
            tracing::debug!("Starting audio stream for: {}", path_for_log);
            sink.append(decoder);

            // 再生完了まで待機
            tracing::debug!("Waiting for audio completion: {}", path_for_log);
            sink.sleep_until_end();
            
            tracing::info!("Successfully completed audio playback: {}", path_for_log);
            Ok(())
        })
        .await
        .context("Audio playback task failed")
        .and_then(|result| result)
    }

    /// 音声を同期的に再生（完了まで待機）
    pub fn play_sound_blocking<P: AsRef<Path>>(&self, file_path: P) -> Result<()> {
        let path = file_path.as_ref();
        let path_str = path.to_string_lossy().to_string();

        tracing::debug!("Playing sound (blocking): {:?}", path);

        let audio_data = {
            let preloaded = self.preloaded_sounds.lock().unwrap();
            preloaded.get(&path_str).cloned()
        };

        let audio_data = match audio_data {
            Some(data) => data,
            None => {
                std::fs::read(path)
                    .with_context(|| format!("Failed to read audio file: {:?}", path))?
            }
        };

        // 既存のストリームを使用
        let sink = Sink::connect_new(&self._stream.mixer());

        let cursor = std::io::Cursor::new(audio_data);
        let decoder = Decoder::new(cursor)
            .with_context(|| format!("Failed to decode audio: {}", path_str))?;

        let global_volume = *self.global_volume.lock().unwrap();
        sink.set_volume(global_volume);
        sink.append(decoder);

        // 再生完了まで待機
        sink.sleep_until_end();
        
        tracing::debug!("Finished playing sound (blocking): {}", path_str);
        Ok(())
    }

    /// グローバル音量を設定（0-100）
    pub fn set_global_volume(&self, volume: u8) {
        let volume_f32 = (volume.min(100) as f32) / 100.0;
        *self.global_volume.lock().unwrap() = volume_f32;
        tracing::info!("Global volume set to: {}% ({:.2})", volume, volume_f32);
    }

    /// 現在のグローバル音量を取得
    pub fn get_global_volume(&self) -> u8 {
        let volume_f32 = *self.global_volume.lock().unwrap();
        (volume_f32 * 100.0) as u8
    }

    /// 事前ロードされた音声ファイルのリストを取得
    pub fn get_preloaded_sounds(&self) -> Vec<String> {
        let preloaded = self.preloaded_sounds.lock().unwrap();
        preloaded.keys().cloned().collect()
    }

    /// 事前ロードされた音声を削除
    pub fn unload_sound<P: AsRef<Path>>(&self, file_path: P) {
        let path_str = file_path.as_ref().to_string_lossy().to_string();
        let mut preloaded = self.preloaded_sounds.lock().unwrap();
        if preloaded.remove(&path_str).is_some() {
            tracing::info!("Unloaded audio file: {}", path_str);
        }
    }

    /// 全ての事前ロードされた音声を削除
    pub fn clear_preloaded_sounds(&self) {
        let mut preloaded = self.preloaded_sounds.lock().unwrap();
        let count = preloaded.len();
        preloaded.clear();
        tracing::info!("Cleared {} preloaded audio files", count);
    }
}

/// 音声ファイルの形式をチェック
pub fn validate_audio_file<P: AsRef<Path>>(file_path: P) -> Result<()> {
    let path = file_path.as_ref();
    
    if !path.exists() {
        return Err(anyhow::anyhow!("Audio file does not exist: {:?}", path));
    }

    let file = File::open(path)
        .with_context(|| format!("Failed to open audio file: {:?}", path))?;
    let buffered = BufReader::new(file);

    let _decoder = Decoder::new(buffered)
        .with_context(|| format!("Unsupported audio format: {:?}", path))?;

    tracing::debug!("Audio file validation successful: {:?}", path);
    Ok(())
}