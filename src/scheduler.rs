use anyhow::{Context, Result};
use chrono::{DateTime, Local, Utc};
use cron::Schedule as CronSchedule;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{sleep_until, Instant, Duration};
use crate::config::Schedule;
use crate::audio::AudioPlayer;

#[derive(Debug, Clone)]
pub struct ScheduleEvent {
    pub schedule_id: String,
    pub file_path: String,
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
    pub fn remove_schedule(&mut self, schedule_id: &str) {
        if self.schedules.remove(schedule_id).is_some() {
            tracing::info!("Removed schedule: {}", schedule_id);
        }
    }

    /// スケジュールを有効/無効に設定
    pub fn set_schedule_enabled(&mut self, schedule_id: &str, enabled: bool) -> Result<()> {
        if let Some(schedule) = self.schedules.get_mut(schedule_id) {
            schedule.enabled = enabled;
            tracing::info!("Schedule {} set to: {}", schedule_id, if enabled { "enabled" } else { "disabled" });
            Ok(())
        } else {
            Err(anyhow::anyhow!("Schedule not found: {}", schedule_id))
        }
    }

    /// 全てのスケジュールをクリア
    pub fn clear_schedules(&mut self) {
        let count = self.schedules.len();
        self.schedules.clear();
        tracing::info!("Cleared {} schedules", count);
    }

    /// 現在のスケジュール一覧を取得
    pub fn get_schedules(&self) -> Vec<Schedule> {
        self.schedules.values().cloned().collect()
    }

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

        // 有効なスケジュールをチェック
        for schedule in schedules.values() {
            if !schedule.enabled {
                continue;
            }

            match Self::get_next_run_time(&schedule.cron, &now) {
                Ok(next_time) => {
                    // 次回実行時間が現在時刻から1秒以内の場合、実行
                    let time_diff = next_time.signed_duration_since(now);
                    if time_diff.num_seconds() <= 1 && time_diff.num_seconds() >= 0 {
                        tracing::info!(
                            "Triggering schedule '{}' at {} (cron: {})",
                            schedule.id,
                            now.format("%Y-%m-%d %H:%M:%S"),
                            schedule.cron
                        );

                        // 音声再生
                        let audio_player_clone = audio_player.clone();
                        let file_path = schedule.file.clone();
                        let schedule_id = schedule.id.clone();
                        
                        tokio::spawn(async move {
                            if let Err(e) = audio_player_clone.play_sound(&file_path).await {
                                tracing::error!("Failed to play sound for schedule '{}': {}", schedule_id, e);
                            }
                        });

                        // イベント送信
                        let event = ScheduleEvent {
                            schedule_id: schedule.id.clone(),
                            file_path: schedule.file.clone(),
                            triggered_at: now,
                        };
                        
                        if let Err(e) = event_tx.send(event) {
                            tracing::warn!("Failed to send schedule event: {}", e);
                        }
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

        // 次回実行時間まで待機
        if let Some(next_time) = next_run_time {
            let now_instant = Instant::now();
            let wait_duration = next_time.signed_duration_since(Local::now());
            
            if wait_duration.num_milliseconds() > 0 {
                let wait_std_duration = Duration::from_millis(wait_duration.num_milliseconds() as u64);
                let wake_time = now_instant + wait_std_duration;
                
                tracing::debug!(
                    "Next schedule at {}, waiting {} ms",
                    next_time.format("%Y-%m-%d %H:%M:%S"),
                    wait_duration.num_milliseconds()
                );
                
                sleep_until(wake_time).await;
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
        
        let from_utc = from.with_timezone(&Utc);
        
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

    /// 指定したcron式の次回3回の実行時間を表示（デバッグ用）
    pub fn preview_next_runs(cron_expr: &str, count: usize) -> Result<Vec<DateTime<Local>>> {
        let schedule = CronSchedule::from_str(cron_expr)
            .map_err(|e| anyhow::anyhow!("Invalid cron expression: {}", e))?;
        
        let mut results = Vec::new();
        let current = Utc::now();
        
        for next in schedule.after(&current).take(count) {
            results.push(next.with_timezone(&Local));
        }
        
        Ok(results)
    }
}