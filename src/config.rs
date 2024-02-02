use serde_derive::Deserialize;
use std::fs;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub client: Client,
    pub permission: Permission,
    pub server: Server,
    pub backup: Option<Backup>,
}

/// Discordクライアントの設定
#[derive(Deserialize, Clone)]
pub struct Client {
    pub secret: String,
    pub show_public_ip: Option<bool>,
}

/// 権限の設定
#[derive(Deserialize, Clone)]
pub struct Permission {
    /// コマンドを送信できるチャンネル
    pub channel_id: u64,

    /// コマンドを実行できるユーザ
    pub user_id: Vec<u64>,
}

/// Minecraftサーバの設定
#[derive(Deserialize, Clone)]
pub struct Server {
    pub work_dir: String,
    pub port: u16,
    pub jar_file: String,
    pub auto_stop: bool,
    pub memory: String,
}

/// バックアップ設定
#[derive(Deserialize, Clone)]
pub struct Backup {
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
