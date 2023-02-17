# mcsv-handler-discord
Minecraft Server Manager at Discord v0.2.0
MinecraftサーバをDiscordのBOTを通じて起動・停止・コマンドの送信ができるクライアントアプリケーションです。

## はじめる前に
### Discord Developer Portal
Discord Developer PortalにてBOTの設定とアクセストークンの取得が必要です。
- [Discord Developer Portal](https://discord.com/developers/applications)

### Privileged Gateway Intents
サーバで送信されるメッセージを取得するために、`Message Content Intent` 特権を有効にする必要があります。
アプリケーションの設定 > `BOT` > `Privileged Gateway Intents` から設定して下さい。

## 使い方
1. 設定ファイル `config.toml` を作成して、必要な設定を行ってください。
2. `config.toml` で設定した作業ディレクトリを作成して、そのディレクトリ内にMinecraftサーバ (`.jar` ファイル) を置いてください。
2. `MCSVHandlerDiscord.exe` を実行してください。CLI (PowerShell, コマンドプロンプトなど) からの実行がおすすめです。
3. 設定したチャンネルで `!mcstart` と入力するとサーバが開始します。
4. 設定したチャンネルで `!mcend` と入力するとサーバが停止します。
5. このアプリケーションを終了したい時は、`Ctrl+c` を入力もしくは設定したチャンネルで `!mcsvend` を入力してください。

### コマンド
起動中のサーバでコマンドを実行するには、`!mcc <コマンド名>` を入力して下さい。
```
!mcc say hello
```

## 設定ファイル
`config.toml` を実行ファイルと同じディレクトリに置いてください。

テンプレート:
```toml
# mcsv-handler-discord 設定
# v 0.2.0

[client] # クライアント設定

secret = "TOKEN"
# Discord Botのシークレットを設定します
# https://discord.com/developers/applications でトークンを取得してください。

[permission] # 権限設定

# BOTが動作するチャンネルのidを数値で指定します。
#
# 例
# ----
# channel_id = 12345678987654321
channel_id = 12345678987654321

# BOTを操作できるユーザのidを数値の配列で指定します。
#
# user_id = [数字, 数字, 数字]
user_id = [12345678987654321]

[server] # Minecraftサーバの設定

# サーバが入っているディレクトリを
# 相対パスで指定します。
work_dir = "srv"

# サーバが入っているjarファイルのファイル名を
# 拡張子付きで指定します。
jar_file = "server.jar"

# サーバのメモリ使用量を指定します。
# 数値の後にMまたはGを指定します。
#
# 例: 2GB
# ----
# memory = "2G"
memory = "2G"

```
