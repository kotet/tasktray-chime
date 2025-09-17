# Tasktray Chime - Makefile
# Windows向けクロスコンパイル対応 (環境構築は devcontainer で自動実行)

# プロジェクト名
PROJECT_NAME := tasktray-chime
BINARY_NAME := tasktray-chime

# Windows向けターゲット
WINDOWS_TARGET := x86_64-pc-windows-gnu

# ディレクトリ
AUDIO_DIR := audios
CONFIG_FILE := config.yaml
RELEASE_DIR := release

# デフォルトターゲット
.DEFAULT_GOAL := help

.PHONY: help
help: ## ヘルプを表示
	@echo "Tasktray Chime - Makefile"
	@echo ""
	@echo "環境構築: devcontainer で自動実行"
	@echo ""
	@echo "利用可能なコマンド:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}'

.PHONY: deps
deps: ## 依存関係の確認
	@echo "=== 依存関係の確認 ==="
	cargo check
	@echo "依存関係OK"

.PHONY: build
build: ## Linux向けデバッグビルド
	@echo "=== Linux向けデバッグビルド ==="
	cargo build

.PHONY: build-release
build-release: ## Linux向けリリースビルド
	@echo "=== Linux向けリリースビルド ==="
	cargo build --release

.PHONY: build-windows
build-windows: ## Windows向けデバッグビルド
	@echo "=== Windows向けデバッグビルド ==="
	cargo build --target $(WINDOWS_TARGET)

.PHONY: build-windows-release
build-windows-release: ## Windows向けリリースビルド
	@echo "=== Windows向けリリースビルド ==="
	cargo build --release --target $(WINDOWS_TARGET)

.PHONY: test
test: ## テストを実行
	@echo "=== テスト実行 ==="
	cargo test

.PHONY: clean
clean: ## ビルドファイルをクリーンアップ
	@echo "=== クリーンアップ ==="
	cargo clean
	rm -rf $(RELEASE_DIR)
	rm -f generate_test_audio
	rm -f logs/*.log
	@echo "クリーンアップ完了"

.PHONY: check-config
check-config: ## 設定ファイルの妥当性をチェック
	@echo "=== 設定ファイルチェック ==="
	@if [ ! -f $(CONFIG_FILE) ]; then \
		echo "設定ファイルが見つかりません: $(CONFIG_FILE)"; \
		echo "デフォルト設定ファイルを作成してください"; \
		exit 1; \
	fi
	@echo "設定ファイルOK: $(CONFIG_FILE)"

.PHONY: check-audio
check-audio: ## 音声ファイルの存在を確認
	@echo "=== 音声ファイルチェック ==="
	@if [ ! -d $(AUDIO_DIR) ]; then \
		echo "音声ディレクトリが見つかりません: $(AUDIO_DIR)"; \
		exit 1; \
	fi
	@if [ ! -f $(AUDIO_DIR)/chime.wav ]; then \
		echo "音声ファイルが見つかりません: $(AUDIO_DIR)/chime.wav"; \
		exit 1; \
	fi
	@if [ ! -f $(AUDIO_DIR)/bell.wav ]; then \
		echo "音声ファイルが見つかりません: $(AUDIO_DIR)/bell.wav"; \
		exit 1; \
	fi
	@echo "音声ファイルOK: $(AUDIO_DIR)/"

.PHONY: run
run: build check-audio check-config ## Linux環境でアプリケーションを実行
	@echo "=== アプリケーション実行 (Linux) ==="
	@echo "注意: dev container環境では音声デバイスが利用できないため、エラーが発生する可能性があります"
	./target/debug/$(BINARY_NAME)

.PHONY: run-release
run-release: build-release check-audio check-config ## Linux環境でリリース版を実行
	@echo "=== リリース版実行 (Linux) ==="
	./target/release/$(BINARY_NAME)

.PHONY: package-linux
package-linux: build-release check-audio ## Linux向けパッケージを作成
	@echo "=== Linux向けパッケージ作成 ==="
	mkdir -p $(RELEASE_DIR)/linux
	cp target/release/$(BINARY_NAME) $(RELEASE_DIR)/linux/
	cp $(CONFIG_FILE) $(RELEASE_DIR)/linux/
	cp -r $(AUDIO_DIR) $(RELEASE_DIR)/linux/
	cp README.md $(RELEASE_DIR)/linux/ 2>/dev/null || echo "README.md not found, skipping"
	@echo "Linux パッケージ作成完了: $(RELEASE_DIR)/linux/"

.PHONY: package-windows
package-windows: build-windows-release check-audio ## Windows向けパッケージを作成
	@echo "=== Windows向けパッケージ作成 ==="
	mkdir -p $(RELEASE_DIR)/windows
	cp target/$(WINDOWS_TARGET)/release/$(BINARY_NAME).exe $(RELEASE_DIR)/windows/
	cp $(CONFIG_FILE) $(RELEASE_DIR)/windows/
	cp -r $(AUDIO_DIR) $(RELEASE_DIR)/windows/
	cp README.md $(RELEASE_DIR)/windows/ 2>/dev/null || echo "README.md not found, skipping"
	@echo "Windows パッケージ作成完了: $(RELEASE_DIR)/windows/"

.PHONY: package-all
package-all: package-linux package-windows ## 全プラットフォーム向けパッケージを作成
	@echo "=== 全プラットフォーム向けパッケージ作成完了 ==="
	@echo "Linux:   $(RELEASE_DIR)/linux/"
	@echo "Windows: $(RELEASE_DIR)/windows/"

.PHONY: format
format: ## コードをフォーマット
	@echo "=== コードフォーマット ==="
	cargo fmt

.PHONY: clippy
clippy: ## Clippyによるコード解析
	@echo "=== Clippy解析 ==="
	cargo clippy -- -D warnings

.PHONY: check-all
check-all: format clippy test ## 全チェックを実行
	@echo "=== 全チェック完了 ==="

.PHONY: info
info: ## プロジェクト情報を表示
	@echo "=== プロジェクト情報 ==="
	@echo "プロジェクト名: $(PROJECT_NAME)"
	@echo "バイナリ名:     $(BINARY_NAME)"
	@echo "Windowsターゲット: $(WINDOWS_TARGET)"
	@echo "設定ファイル:   $(CONFIG_FILE)"
	@echo "音声ディレクトリ: $(AUDIO_DIR)"
	@echo ""
	@echo "=== 現在の状態 ==="
	@echo -n "Rustターゲット $(WINDOWS_TARGET): "
	@if rustup target list --installed | grep -q $(WINDOWS_TARGET); then \
		echo "インストール済み"; \
	else \
		echo "未インストール (devcontainer再構築でインストールされます)"; \
	fi
	@echo -n "設定ファイル: "
	@if [ -f $(CONFIG_FILE) ]; then \
		echo "存在"; \
	else \
		echo "未作成"; \
	fi
	@echo -n "音声ファイル: "
	@if [ -d $(AUDIO_DIR) ] && [ -f $(AUDIO_DIR)/chime.wav ] && [ -f $(AUDIO_DIR)/bell.wav ]; then \
		echo "存在"; \
	else \
		echo "不完全 ($(AUDIO_DIR)/chime.wav, $(AUDIO_DIR)/bell.wav が必要です)"; \
	fi

.PHONY: logs
logs: ## ログファイルを表示
	@echo "=== ログファイル ==="
	@if [ -d logs ] && [ -n "$$(find logs -name "*.log" -type f 2>/dev/null)" ]; then \
		echo "最新のログファイル:"; \
		find logs -name "*.log" -type f -exec ls -la {} \; | sort -k9; \
		echo ""; \
		echo "最新ログの内容:"; \
		find logs -name "*.log" -type f -exec ls -t {} \; | head -1 | xargs tail -20; \
	else \
		echo "ログファイルが見つかりません"; \
	fi

# Windows開発者向けのヘルプ
.PHONY: help-windows
help-windows: ## Windows開発者向けヘルプ
	@echo "=== Windows開発者向けガイド ==="
	@echo ""
	@echo "環境構築は devcontainer で自動実行されます"
	@echo ""
	@echo "1. Windows向けビルド:"
	@echo "   make build-windows         # デバッグビルド"
	@echo "   make build-windows-release # リリースビルド"
	@echo ""
	@echo "2. パッケージ作成:"
	@echo "   make package-windows       # Windows向けパッケージ"
	@echo "   make package-all          # 全プラットフォーム向け"
	@echo ""
	@echo "3. 生成されるファイル:"
	@echo "   target/x86_64-pc-windows-gnu/release/$(BINARY_NAME).exe"
	@echo "   release/windows/$(BINARY_NAME).exe"
	@echo ""
	@echo "注意事項:"
	@echo "- Windows実行ファイルはLinux環境では実行できません"
	@echo "- 実際のテストは Windows 環境で行ってください"
	@echo "- 音声再生にはWindows上でのオーディオデバイスが必要です"