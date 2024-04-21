# Hone

🦴 Minecraft Server Manager on Discord 🦴

Minecraft サーバを Discord の BOT を通じて起動・停止・コマンドの送信ができるクライアントアプリケーションです。

## はじめる前に

### Discord Developer Portal

Discord Developer Portal にて BOT の設定とアクセストークンの取得が必要です。

- [Discord Developer Portal](https://discord.com/developers/applications)

### Privileged Gateway Intents

サーバで送信されるメッセージを取得するために、`Message Content Intent` 特権を有効にする必要があります。
アプリケーションの設定 > `BOT` > `Privileged Gateway Intents` から設定して下さい。

## 使い方

1. 設定ファイル `config.toml` を作成して、必要な設定を行ってください。
2. `config.toml` で設定した作業ディレクトリを作成して、そのディレクトリ内に Minecraft サーバ (`.jar` ファイル) を置いてください。
3. `MCSVHandlerDiscord.exe` を実行してください。CLI (PowerShell, コマンドプロンプトなど) からの実行がおすすめです。
4. 設定したチャンネルで `!mcstart` と入力するとサーバが開始します。
5. 設定したチャンネルで `!mcend` と入力するとサーバが停止します。
6. このアプリケーションを終了したい時は、`Ctrl+c` を入力もしくは設定したチャンネルで `!mcsvend` を入力してください。

### コマンド

起動中のサーバでコマンドを実行するには、`!mcc <コマンド名>` を入力して下さい。

```
!mcc say hello
```

## 設定ファイル

`config.example.toml` をコピーして、 `config.toml` を実行ファイルと同じディレクトリに置いてください。
