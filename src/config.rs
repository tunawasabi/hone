use serde_derive::Deserialize;
use serenity::all::{ChannelId, UserId};
use std::fs;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub client: ClientConfig,
    pub permission: PermissionConfig,
    pub server: ServerConfig,
    pub backup: Option<BackupConfig>,
}

/// Discordクライアントの設定
#[derive(Deserialize, Clone)]
pub struct ClientConfig {
    pub secret: String,
    pub show_public_ip: Option<bool>,
}

/// 権限の設定
#[derive(Deserialize, Clone)]
pub struct PermissionConfig {
    /// コマンドを送信できるチャンネル
    pub channel_id: ChannelId,

    /// コマンドを実行できるユーザ
    pub user_id: Vec<UserId>,
}

/// Minecraftサーバの設定
#[derive(Deserialize, Clone)]
pub struct ServerConfig {
    pub work_dir: String,
    #[cfg(target_os = "windows")]
    pub port: u16,
    pub jar_file: String,
    pub auto_stop: bool,
    pub memory: String,
}

/// バックアップ設定
#[derive(Deserialize, Clone)]
pub struct BackupConfig {
    pub output_dir: String,
}

impl Config {
    pub fn read_from(path: &str) -> Result<Config, String> {
        let config = match fs::read_to_string(path) {
            Ok(v) => v,
            Err(err) => return Err(format!("設定ファイルを開くことができませんでした: {}", err)),
        };

        match toml::from_str::<Config>(&config) {
            Ok(config) => Ok(config),
            Err(err) => Err(format!("設定に誤りがあります: {}", err)),
        }
    }
}
