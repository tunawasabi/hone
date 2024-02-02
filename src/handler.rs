use self::command::*;
use crate::config::Config;
use crate::save::backup::save_backup;
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::prelude::ChannelId;
use serenity::model::prelude::ChannelType;
use serenity::prelude::*;
use std::process::exit;
use std::sync::{mpsc, Arc};

mod command;
mod log_sender;
use log_sender::*;

type ArcMutex<T> = Arc<Mutex<T>>;

pub struct Handler {
    config: Config,
    http: Arc<Http>,
    thread_stdin: ArcMutex<Option<mpsc::Sender<String>>>,
    log_thread: ArcMutex<Option<LogSender>>,
}

impl Handler {
    pub fn new(config: Config) -> Handler {
        let stdin = Arc::new(Mutex::new(None));
        let http = Arc::new(Http::new(&config.client.secret));
        Handler {
            config,
            http,
            thread_stdin: stdin,
            log_thread: Arc::new(Mutex::new(None)),
        }
    }

    async fn send_message(&self, message: impl AsRef<str>) -> Result<Message, SerenityError> {
        let channel = ChannelId(self.config.permission.channel_id);
        channel.say(&self.http, message.as_ref()).await
    }

    async fn is_server_running(&self) -> bool {
        self.thread_stdin.lock().await.is_some()
    }

    #[inline]
    fn is_allowed_user(&self, id: u64) -> bool {
        self.config.permission.user_id.contains(&id)
    }

    #[inline]
    fn is_allowed_channel(&self, id: u64) -> bool {
        id == self.config.permission.channel_id
    }
}

struct MessageSender;
impl MessageSender {
    async fn send(message: impl AsRef<str>, http: &Http, channel: ChannelId) -> Option<Message> {
        match channel.say(http, message.as_ref()).await {
            Ok(msg) => Some(msg),
            Err(e) => {
                println!("{}", e);
                None
            }
        }
    }
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn message(&self, _: Context, msg: Message) {
        if !self.is_allowed_user(*msg.author.id.as_u64())
            || !self.is_allowed_channel(*msg.channel_id.as_u64())
        {
            return;
        }

        let Some(args) = parse_command(&msg.content) else {
            return;
        };
        let command = args[0];
        let args = args[1..].to_vec();

        match command {
            // サーバ開始
            "mcstart" => mcstart(self).await,
            // コマンド送信
            "mcc" => send_command_to_server(self, args).await,
            // サーバ停止
            "mcend" => send_stop_to_server(self).await,
            // クライアント停止
            "mcsvend" => mcsvend(self).await,
            // バックアップ
            "mcbackup" => save_backup(self.config.backup.clone(), self.config.server.clone()),
            _ => {
                self.send_message("存在しないコマンドです。").await.ok();
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        let Ok(channel) = ctx
            .http
            .get_channel(self.config.permission.channel_id)
            .await
        else {
            println!("設定で指定されているチャンネルが見つかりません。permisson.channel_id の値を修正してください。");
            println!("* BOTがチャンネルのあるサーバに参加しているか確認してください。");
            exit(-1);
        };

        let Some(channel) = channel.guild() else {
            println!("プライベートチャンネル、チャンネルカテゴリーを管理用チャンネルに指定することはできません。permisson.channel_id の値を修正してください。");
            exit(-1);
        };

        // テキストチャンネルであることを確認
        if ChannelType::Text != channel.kind {
            println!("ボイスチャンネルやスレッド、フォーラムなどを管理用チャンネルに指定することはできません。テキストチャンネルを指定してください。");
            exit(-1);
        }

        println!("Discordに接続しました。");
        println!("BOTの名前: {}", ready.user.tag());
        println!(
            "管理チャンネル: {} (in {})",
            channel.name(),
            channel
                .guild_id
                .to_partial_guild(ctx.http)
                .await
                .unwrap()
                .name
        );
    }
}
