# Tasktray Chime

Windows向けタスクトレイ常駐型時報アプリケーション

## 概要

指定した時間に音声で知らせるタスクトレイ常駐アプリケーションです。
cron形式のスケジュール設定により、柔軟な時間指定が可能です。

## 機能

- タスクトレイ常駐
- cron形式でのスケジュール設定
- 複数の音声ファイル対応
- 音量調整機能
- Windows自動起動設定
- YAML設定ファイル
- ログファイル出力

## 環境要件

- Dev Container環境（推奨）
- Rust（stable）
- Windows向けクロスコンパイル環境（mingw-w64）

## セットアップ

### Dev Container使用時

1. VS Codeでプロジェクトを開く
2. "Reopen in Container"を選択
3. 環境構築が自動で完了

### 手動セットアップ

```bash
# Rustターゲット追加
rustup target add x86_64-pc-windows-gnu

# 必要なツールのインストール（Linux/macOS）
sudo apt-get install mingw-w64 gcc-mingw-w64
# または
brew install mingw-w64
```

## ビルド

```bash
# 依存関係確認
make deps

# Linux向けビルド
make build               # デバッグビルド
make build-release       # リリースビルド

# Windows向けビルド
make build-windows         # デバッグビルド
make build-windows-release # リリースビルド
```

## パッケージ作成

```bash
# プラットフォーム別パッケージ
make package-linux     # Linux向け
make package-windows   # Windows向け
make package-all       # 全プラットフォーム向け
```

生成されたパッケージは `release/` ディレクトリに保存されます。

## 使用方法

### Linux環境

```bash
make run                # デバッグ版実行
make run-release        # リリース版実行
```

### Windows環境

1. `release/windows/` からファイルをWindowsマシンにコピー
2. `tasktray-chime.exe` を実行
3. タスクトレイアイコンを右クリックして設定

## コマンド一覧

```bash
make help               # ヘルプ表示
make deps               # 依存関係確認
make build              # Linux向けデバッグビルド
make build-release      # Linux向けリリースビルド
make build-windows      # Windows向けデバッグビルド
make build-windows-release # Windows向けリリースビルド
make test               # テスト実行
make clean              # ビルドファイルクリーンアップ
make check-config       # 設定ファイルチェック
make package-linux      # Linux向けパッケージ作成
make package-windows    # Windows向けパッケージ作成
make package-all        # 全プラットフォーム向けパッケージ作成
make format             # コードフォーマット
make clippy             # Clippy解析
make check-all          # 全チェック実行
make info               # プロジェクト情報表示
make logs               # ログファイル表示
make help-windows       # Windows向けヘルプ
```

## ファイル構成

```
tasktray-chime/
├── src/                    # ソースコード
│   ├── main.rs            # エントリーポイント
│   ├── config.rs          # 設定管理
│   ├── audio.rs           # 音声再生
│   ├── scheduler.rs       # スケジューラー
│   └── tray.rs           # タスクトレイUI
├── .devcontainer/         # Dev Container設定
├── audios/               # 音声ファイル（生成）
├── config.yaml           # 設定ファイル（実行ファイルと同じディレクトリ）
├── logs/                 # ログファイル（実行ファイルと同じディレクトリ配下）
├── release/              # ビルド成果物（生成）
├── Cargo.toml           # Rustプロジェクト設定
├── Makefile             # ビルド自動化
└── README.md            # このファイル
```

## 設定ファイルとログについて

### 設定ファイル (config.yaml)

- **配置場所**: 実行ファイル（tasktray-chime または tasktray-chime.exe）と同じディレクトリ
- **作成**: 初回起動時に設定ファイルが存在しない場合、自動的にデフォルト設定ファイルが作成されます

### ログファイル

- **配置場所**: 実行ファイルと同じディレクトリ配下の `logs` フォルダ
- **ファイル名**: `tasktray-chime.YYYY-MM-DD.log` (日別ローテーション)
- **設定**: `config.yaml` の `logging` セクションで設定変更可能

### 配置例

```
実行ディレクトリ/
├── tasktray-chime.exe     # Windows実行ファイル
├── config.yaml           # 設定ファイル
├── audios/               # 音声ファイルフォルダ
│   ├── chime.wav
│   └── bell.wav
└── logs/                 # ログフォルダ（自動作成）
    ├── tasktray-chime.2025-01-01.log
    └── tasktray-chime.2025-01-02.log
```

## トラブルシューティング

### 音声が再生されない

- オーディオデバイスが正しく設定されているか確認
- 音声ファイルが存在するか確認
- 音量設定を確認

### Windows向けビルドが失敗する

- Dev Container環境を使用することを推奨
- 手動環境の場合、mingw-w64が正しくインストールされているか確認

### タスクトレイアイコンが表示されない

- Windows環境でのみサポート
- システム設定でタスクトレイアイコン表示が有効になっているか確認

## ライセンス

このプロジェクトはMITライセンスの下で公開されています。