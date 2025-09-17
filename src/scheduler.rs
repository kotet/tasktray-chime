use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use cron::Schedule as CronSchedule;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;
use crate::config::Schedule;
use crate::audio::AudioPlayer;

#[derive(Debug, Clone)]
pub struct ScheduleEvent {
    pub schedule_id: String,
    pub triggered_at: DateTime<Local>,
}

pub struct CronScheduler {
    schedules: HashMap<String, Schedule>,
    audio_player: Arc<AudioPlayer>,
    event_sender: Option<mpsc::UnboundedSender<ScheduleEvent>>,
    shutdown_sender: Option<tokio::sync::oneshot::Sender<()>>,
}

impl CronScheduler {
    pub fn new(audio_player: Arc<AudioPlayer>) -> Self {
        Self {
            schedules: HashMap::new(),
            audio_player,
            event_sender: None,
            shutdown_sender: None,
        }
    }

    /// スケジュールを追加/更新
    pub fn add_schedule(&mut self, schedule: Schedule) -> Result<()> {
        // cron式の妥当性をチェック
        Self::validate_cron_expression(&schedule.cron)?;
        
        tracing::info!("Adding schedule: {} with cron: {}", schedule.id, schedule.cron);
        self.schedules.insert(schedule.id.clone(), schedule);
        Ok(())
    }

    /// スケジュールを削除
    /// スケジューラーを開始
    pub async fn start(&mut self) -> Result<mpsc::UnboundedReceiver<ScheduleEvent>> {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        self.event_sender = Some(event_tx.clone());
        self.shutdown_sender = Some(shutdown_tx);

        let schedules = self.schedules.clone();
        let audio_player = self.audio_player.clone();

        tokio::spawn(async move {
            tracing::info!("Cron scheduler started with {} schedules", schedules.len());
            
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        tracing::info!("Cron scheduler shutdown requested");
                        break;
                    }
                    _ = Self::run_scheduler_cycle(&schedules, &audio_player, &event_tx) => {
                        // スケジューラーサイクル完了後、短い間隔で再チェック
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                }
            }
            
            tracing::info!("Cron scheduler stopped");
        });

        Ok(event_rx)
    }

    /// スケジューラーを停止
    pub fn stop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_sender.take() {
            let _ = shutdown_tx.send(());
            tracing::info!("Cron scheduler stop signal sent");
        }
    }

    /// スケジューラーサイクルを実行
    async fn run_scheduler_cycle(
        schedules: &HashMap<String, Schedule>,
        audio_player: &Arc<AudioPlayer>,
        event_tx: &mpsc::UnboundedSender<ScheduleEvent>,
    ) {
        let now = Local::now();
        let mut next_run_time: Option<DateTime<Local>> = None;
        let mut schedules_to_execute = Vec::new();

        // 有効なスケジュールをチェックし、次回実行時間を計算
        for schedule in schedules.values() {
            if !schedule.enabled {
                continue;
            }

            match Self::get_next_run_time(&schedule.cron, &now) {
                Ok(next_time) => {
                    // 実行すべきスケジュールかどうかチェック（1分の余裕を持って判定）
                    let time_diff = next_time.signed_duration_since(now);
                    
                    // 次回実行時間が現在時刻から5秒以内なら実行対象とする
                    if time_diff.num_seconds() <= 5 && time_diff.num_seconds() >= -5 {
                        tracing::info!(
                            "Schedule '{}' ready for execution at {} (cron: {}), time diff: {} seconds",
                            schedule.id,
                            next_time.format("%Y-%m-%d %H:%M:%S"),
                            schedule.cron,
                            time_diff.num_seconds()
                        );
                        schedules_to_execute.push(schedule.clone());
                    }

                    // 次回実行時間を更新
                    match next_run_time {
                        None => next_run_time = Some(next_time),
                        Some(current_next) => {
                            if next_time < current_next {
                                next_run_time = Some(next_time);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to calculate next run time for schedule '{}': {}", schedule.id, e);
                }
            }
        }

        // 実行対象のスケジュールを実行
        for schedule in schedules_to_execute {
            let now_exec = Local::now();
            tracing::info!(
                "Executing schedule '{}' at {} (cron: {})",
                schedule.id,
                now_exec.format("%Y-%m-%d %H:%M:%S"),
                schedule.cron
            );

            // 音声再生
            let audio_player_clone = audio_player.clone();
            let file_path = schedule.file.clone();
            let schedule_id = schedule.id.clone();
            
            tokio::spawn(async move {
                tracing::info!("Starting audio playback for schedule '{}': {}", schedule_id, file_path);
                match audio_player_clone.play_sound(&file_path).await {
                    Ok(()) => {
                        tracing::info!("Successfully completed audio playback for schedule '{}'", schedule_id);
                    }
                    Err(e) => {
                        tracing::error!("Failed to play sound for schedule '{}': {}", schedule_id, e);
                    }
                }
            });

            // イベント送信
            let event = ScheduleEvent {
                schedule_id: schedule.id.clone(),
                triggered_at: now_exec,
            };
            
            if let Err(e) = event_tx.send(event) {
                tracing::warn!("Failed to send schedule event: {}", e);
            }
        }

        // 次回実行時間まで待機
        if let Some(next_time) = next_run_time {
            let current_time = Local::now();
            let wait_duration = next_time.signed_duration_since(current_time);
            
            if wait_duration.num_milliseconds() > 100 {  // 100ms以上の場合のみ待機
                let wait_ms = wait_duration.num_milliseconds().max(1000) as u64;  // 最小1秒待機
                
                tracing::debug!(
                    "Next schedule at {}, waiting {} ms",
                    next_time.format("%Y-%m-%d %H:%M:%S"),
                    wait_ms
                );
                
                tokio::time::sleep(Duration::from_millis(wait_ms)).await;
            } else {
                // 短時間待機
                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        } else {
            // アクティブなスケジュールがない場合は1秒待機
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    /// 次回実行時間を計算
    fn get_next_run_time(cron_expr: &str, from: &DateTime<Local>) -> Result<DateTime<Local>> {
        let schedule = CronSchedule::from_str(cron_expr)
            .map_err(|e| anyhow::anyhow!("Invalid cron expression '{}': {}", cron_expr, e))?;
        
        // 1秒前から検索開始して、現在時刻付近の実行時間をより正確に捕捉
        let search_from = from.clone() - chrono::Duration::seconds(1);
        let from_utc = search_from.with_timezone(&Utc);
        
        // cronクレートは次回実行時間をUTCで返すため、ローカル時間に変換
        if let Some(next_utc) = schedule.after(&from_utc).next() {
            Ok(next_utc.with_timezone(&Local))
        } else {
            Err(anyhow::anyhow!("No future execution time found for cron expression: {}", cron_expr))
        }
    }

    /// cron式の妥当性を検証
    fn validate_cron_expression(cron_expr: &str) -> Result<()> {
        CronSchedule::from_str(cron_expr)
            .map_err(|e| anyhow::anyhow!("Invalid cron expression '{}': {}", cron_expr, e))?;
        Ok(())
    }
}