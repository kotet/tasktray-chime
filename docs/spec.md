# Tasktray Chime — 仕様書

**概要**
- **目的**: Windows のタスクトレイ常駐アプリとして、cron 形式で指定した音声ファイルを秒単位の精度で再生する時報アプリ。
- **実装言語**: Rust（stable）
- **アプリ形態**: ユーザーレベルのスタンドアローン実行バイナリ（単一の `.exe`、設定ファイルは同フォルダに配置）
- **設定ファイル形式**: YAML（複数スケジュールを配列で定義）。存在しない場合はデフォルト設定のファイルを生成

---

## 技術スタック
- **トレイライブラリ**: `tray-icon`（安定、シンプル、クロスプラットフォーム対応）
- **音声再生ライブラリ**: `rodio`（高レベル API、WAV/MP3/OGG 対応、簡易再生制御、依存少）
- **cron ライブラリ**: `cron_parser`（秒単位精度、YAML cron 式を直接パース可能、tokio 連携容易）
- **ログライブラリ**: `tracing` + `tracing-subscriber` + `tracing-appender`（ファイル出力、ログレベル制御、ローテーション対応、非同期対応）
- **その他**: `serde_yaml`（YAML 読み込み）、`tokio`（非同期タイマー）

## 機能・振る舞い
- **タスクトレイ常駐**（アイコン + コンテキストメニュー）
- **スケジュール方式**: `type: cron` のみ。秒精度でスケジュール実行
- **音声再生**: ローカルファイルのみ（WAV/MP3/OGG）。`rodio` を使用
- **自動起動**: コンテキストメニューから切替可能（デフォルトオフ）
- **コンテキストメニュー操作**:
  - 自動起動切替
  - 設定ファイルを開く
  - ログディレクトリを開く
  - アプリ終了
- **ログ**: ファイルベースのみ。`tracing` 系でログレベル制御、ローテーションオプションあり

## YAML スキーマ例
```yaml
app:
  start_on_login: false

logging:
  level: "info"
  directory: "./logs"
  rotate: true
  max_files: 7

audio:
  global_volume: 80

schedules:
  - id: "hourly_chime"
    type: "cron"
    cron: "0 * * * *" # 毎時0分
    file: "./sounds/tick.mp3"
    enabled: true

behavior:
  retry_on_fail: 0 # 再生失敗時のリトライ回数（0でリトライなし）
  retry_delay_seconds: 5

```

## rodio 実装上の注意
- **事前ロード**: 秒精度を高めるため、再生前に音声ファイルをメモリにデコード
- **遅延補正**: 実機で呼び出し〜音出力までの遅延を測定し、必要なら補正
- **音量制御**: `rodio::Sink::set_volume` を使用
- **スレッド設計**: トレイのイベントループと `tokio` タスクはチャネルで連携、再生は `spawn_blocking` などで非同期実行
- **フォーマット対応**: MP3/OGG の feature や依存を確認して CI ビルド

## cron_parser 実装上の注意
- YAML の cron フィールドを文字列として読み込み、`cron_parser::parse` で Schedule オブジェクトに変換
- tokio タイマーで次回時刻まで `sleep_until` することで秒精度再生を実現
- 複数スケジュールが有効の場合は、次回時刻を比較して最も早いものを待機ターゲットとする

## tracing 実装上の注意
- `tracing_appender::rolling` を用いたログローテーション設定
- `tracing_subscriber::fmt` でフォーマット指定（時間、レベル、メッセージ）
- tokio の非同期タスクからも安全にログ出力可能

## 配布形式
- 単一 `.exe` のみ
- バイナリと `config.yaml` を同フォルダに配置
- 相対パスで音声ファイル管理（例: `./sounds/`）
