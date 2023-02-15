use colored::*;
use mcsv_handler_discord::executor::{mcserver_new, read_config};
use mcsv_handler_discord::print_mclog;
use mcsv_handler_discord::types::{Config, ServerMessage};
use serenity::async_trait;
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::prelude::ChannelId;
use serenity::prelude::*;
use std::io::{BufRead, BufReader, Write};
use std::process::{exit, ChildStdin};
use std::sync::{mpsc, Arc};
use std::thread;
use tokio::runtime::Handle;

struct Handler {
    config: Config,
    http: Arc<Http>,
    thread_stdin: Arc<Mutex<Option<ChildStdin>>>,
}

struct MessageSender;

impl Handler {
    fn new(config: Config) -> Handler {
        let stdin = Arc::new(Mutex::new(Option::<ChildStdin>::None));
        let http = Arc::new(Http::new(&config.client.secret));
        Handler {
            config,
            http,
            thread_stdin: stdin,
        }
    }

    async fn send(&self, message: String) {
        let channel = ChannelId(self.config.permission.channel_id);

        if let Err(e) = channel.say(&self.http, message).await {
            println!("{}", e);
        }
    }
}

impl MessageSender {
    async fn send(message: String, http: &Http, channel: u64) {
        let channel = ChannelId(channel);

        if let Err(e) = channel.say(http, message).await {
            println!("{}", e);
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _: Context, msg: Message) {
        if msg.author.id != self.config.permission.user_id {
            return;
        }

        if msg.channel_id != self.config.permission.channel_id {
            return;
        }

        // サーバ起動コマンド
        if msg.content == "!mcstart" {
            // 標準入力が存在するなら, 既に起動しているのでreturnする
            if let Some(_) = *(self.thread_stdin.lock().await) {
                self.send("すでに起動しています！".to_string()).await;
                return;
            }

            self.send("開始しています……".to_string()).await;

            let memory = self.config.server.memory.clone();
            let jar_file = self.config.server.jar_file.clone();
            let work_dir = self.config.server.work_dir.clone();
            let (thread_tx, rx) = mpsc::channel::<ServerMessage>();
            let (thread_tx2, rx2) = mpsc::channel::<ChildStdin>();

            // Minecraft サーバスレッド
            thread::spawn(move || {
                // Minecraft サーバを起動する
                let mut server_thread = match mcserver_new(&jar_file, &work_dir, &memory) {
                    Ok(child) => child,
                    Err(err) => {
                        thread_tx
                            .send(ServerMessage::Error(format!(
                                "Minecraftサーバのプロセスを起動できませんでした: {}",
                                err
                            )))
                            .unwrap();
                        return;
                    }
                };

                let mut bufread = BufReader::new(server_thread.stdout.as_mut().unwrap());
                let mut bufread_err = BufReader::new(server_thread.stderr.as_mut().unwrap());

                thread_tx2
                    .send(server_thread.stdin.take().unwrap())
                    .unwrap();

                // 出力
                let mut buf = String::new();
                let mut buf_err = String::new();

                loop {
                    let mut flag = (false, false);

                    if let Ok(lines) = bufread.read_line(&mut buf) {
                        if lines == 0 {
                            flag.0 = true;
                        } else {
                            // JVMからの出力をそのまま出力する。
                            // 改行コードが既に含まれているのでprint!マクロを使う
                            print_mclog!("{}", buf);

                            if buf.contains("Done") {
                                thread_tx.send(ServerMessage::Done).unwrap();
                            }

                            // Minecraftサーバ終了を検知
                            if buf.contains("All dimensions are saved") {
                                thread_tx.send(ServerMessage::Exit).unwrap();
                                break;
                            }

                            buf.clear();
                        }
                    } else if let Ok(n) = bufread_err.read_line(&mut buf_err) {
                        if n > 0 {
                            print!("{} {}", "[  ERROR  ]".red().bold(), buf_err);
                            thread_tx
                                .send(ServerMessage::Error(buf_err.clone()))
                                .unwrap();
                            buf_err.clear();
                        } else {
                            flag.1 = true
                        }
                    }

                    if flag.0 && flag.1 {
                        break;
                    }
                }

                thread_tx.send(ServerMessage::Exit).unwrap();
            });

            // Minecraftサーバへの標準入力 (stdin) を取得する
            // stdinを取得するまで次に進まない
            let mut stdin = self.thread_stdin.lock().await;
            *stdin = Some(rx2.recv().unwrap());

            let http = Arc::clone(&self.http);
            let channel = self.config.permission.channel_id;
            let stdin = Arc::clone(&self.thread_stdin);

            let tokio_handle = Handle::current();

            // メッセージ処理を行うスレッド
            thread::spawn(move || {
                for v in rx {
                    let http = Arc::clone(&http);
                    let stdin = Arc::clone(&stdin);
                    tokio_handle.spawn(async move {
                        match v {
                            ServerMessage::Exit => {
                                println!("サーバが停止しました。");
                                MessageSender::send("終了しました".to_string(), &http, channel)
                                    .await;
                            }
                            ServerMessage::Done => {
                                MessageSender::send(
                                    "起動完了！接続できます。".to_string(),
                                    &http,
                                    channel,
                                )
                                .await;
                            }
                            ServerMessage::Error(e) => {
                                MessageSender::send(
                                    format!(" エラーが発生しました:\n```{}\n```", e),
                                    &http,
                                    channel,
                                )
                                .await;
                                let mut stdin = stdin.lock().await;
                                *stdin = None;
                            }
                        }
                    });
                }
            });
        }

        // サーバ停止コマンド
        if msg.content == "!mcend" {
            let mut stdin = self.thread_stdin.lock().await;

            match stdin.as_ref() {
                Some(mut v) => {
                    println!("stopping...");
                    self.send("終了しています……".to_string()).await;
                    v.write_all(b"stop\n").unwrap();
                    *stdin = None;
                }
                None => {
                    self.send("起動していません！".to_string()).await;
                }
            }

            return;
        }

        // クライアント停止コマンド
        if msg.content == "!mcsvend" {
            self.send("クライアントを終了しました。".to_string()).await;
            exit(0);
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("Discordに接続しました: {}", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    let config = read_config().unwrap_or_else(|err| {
        println!("{}", err);
        exit(-1);
    });

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
