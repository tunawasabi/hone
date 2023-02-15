use serde_derive::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub client: Client,
    pub permission: Permission,
    pub server: Server,
}

#[derive(Deserialize)]
pub struct Client {
    pub secret: String,
}

#[derive(Deserialize)]
pub struct Permission {
    pub channel_id: u64,
    pub user_id: Vec<u64>,
}

#[derive(Deserialize)]
pub struct Server {
    pub work_dir: String,
    pub jar_file: String,
    pub memory: String,
}
