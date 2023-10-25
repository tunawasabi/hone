use serenity::prelude::*;
use std::path::Path;
use std::process::exit;

mod config;
use config::Config;

mod executor;
mod handler;
use handler::Handler;

mod types;

pub async fn start() {
    let config = Config::read_from("config.toml").unwrap_or_else(|err| {
        println!("{}", err);
        exit(-1);
    });

    if !Path::new(&format!(
        "{}\\{}",
        config.server.work_dir, config.server.jar_file
    ))
    .exists()
    {
        let current = std::env::current_dir().unwrap();
        let current = current.to_str().unwrap();
        println!(
            "サーバが存在しません。{}\\{}\\{}に置いてください",
            current, config.server.work_dir, config.server.jar_file
        );
        exit(-1);
    }

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&config.client.secret, intents)
        .event_handler(Handler::new(config))
        .await
        .expect("Err creating client");

    if let Err(e) = client.start().await {
        println!("Client error: {:?}", e);
    }
}
