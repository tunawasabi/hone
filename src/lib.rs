use context::ConfigContext;
use serenity::prelude::*;
use std::path::Path;
use std::process::exit;

mod config;
use config::Config;

mod handler;
mod server;
use handler::Handler;

mod context;
mod save;
mod types;

pub async fn start() {
    let Config {
        client: client_cfg,
        server: server_cfg,
        ..
    } = ConfigContext::get();

    let server_path = Path::new(&server_cfg.work_dir).join(&server_cfg.jar_file);
    if !server_path.exists() {
        let current = std::env::current_dir().unwrap();
        let current = current.to_str().unwrap();
        println!(
            "サーバが存在しません。{}に置いてください",
            Path::new(current).join(server_path).display()
        );
        exit(-1);
    }

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&client_cfg.secret, intents)
        .event_handler(Handler::new(ConfigContext::get().clone()))
        .await
        .expect("Err creating client");

    if let Err(e) = client.start().await {
        println!("Client error: {:?}", e);
    }
}
