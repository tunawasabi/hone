use serde_derive::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub client: Client,
    pub permission: Permission,
    pub server: Server,
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
