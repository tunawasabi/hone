use serde_derive::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub client: Client,
    pub permission: Permission,
    pub server: Server,
}

#[derive(Deserialize, Clone)]
pub struct Client {
    pub secret: String,
}

#[derive(Deserialize, Clone)]
pub struct Permission {
    pub channel_id: u64,
    pub user_id: Vec<u64>,
}

#[derive(Deserialize, Clone)]
pub struct Server {
    pub work_dir: String,
    pub port: u16,
    pub jar_file: String,
    pub memory: String,
}

pub enum ServerMessage {
    Done,
    Exit,
    Info(String),
    Error(String),
}
