FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

# 必要なパッケージのインストール
RUN apt-get update \
    && apt-get install -y curl gnupg \
    && rm -rf /var/lib/apt/lists/*

USER vscode

# Node.jsのインストール（npmのため）
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.3/install.sh | bash

SHELL ["/bin/bash", "-c"]

RUN source "$HOME/.nvm/nvm.sh" \
    && nvm install 22 \
    && nvm use 22 \
    && nvm alias default 22 \
    && echo 'source "$HOME/.nvm/nvm.sh"' >> $HOME/.bashrc


RUN source "$HOME/.nvm/nvm.sh" && npm install -g @google/gemini-cli
