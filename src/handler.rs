use crate::executor;
use crate::types::Config;
use crate::types::ServerMessage;
use chrono;
use serenity::http::Http;
use serenity::model::channel::Channel;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::prelude::ChannelId;
use serenity::prelude::*;
use std::io::Write;
use std::process::{exit, ChildStdin};
use std::sync::{mpsc, Arc};
use std::thread;

type ArcMutex<T> = Arc<Mutex<T>>;

pub struct Handler {
    config: Config,
    http: Arc<Http>,
    thread_stdin: ArcMutex<Option<ChildStdin>>,
    command_inputed: ArcMutex<bool>,
    thread_id: ArcMutex<Option<u64>>,
}

// スレッド名の前につける稼働状況
const RUNNING_INDICATER: &str = "[🏃稼働中]";
const LOG_INDICATER: &str = "🗒️";

impl Handler {
    pub fn new(config: Config) -> Handler {
        let stdin = Arc::new(Mutex::new(Option::<ChildStdin>::None));
        let http = Arc::new(Http::new(&config.client.secret));
        Handler {
            config,
            http,
            thread_stdin: stdin,
            command_inputed: Arc::new(Mutex::new(false)),
            thread_id: Arc::new(Mutex::new(None)),
        }
    }

    async fn send(&self, message: impl AsRef<str>) {
        let channel = ChannelId(self.config.permission.channel_id);

        if let Err(e) = channel.say(&self.http, message.as_ref()).await {
            println!("{}", e);
        }
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
    async fn send(message: impl AsRef<str>, http: &Http, channel: u64) -> Option<Message> {
        let channel = ChannelId(channel);

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
        if !self.is_allowed_user(*msg.author.id.as_u64()) {
            return;
        }

        if !self.is_allowed_channel(*msg.channel_id.as_u64()) {
            return;
        }

        if msg.content.len() <= 1 && !msg.content.starts_with("!") {
            return;
        }

        let mut message = msg.content[1..].split(' ');
        let command = message.next().unwrap();
        let args: Vec<&str> = message.collect();

        // サーバ起動コマンド
        if command == "mcstart" {
            // 標準入力が存在するなら, 既に起動しているのでreturnする
            if let Some(_) = *(self.thread_stdin.lock().await) {
                self.send("すでに起動しています！").await;
                return;
            }

            self.send("開始しています……".to_string()).await;

            executor::open_port(self.config.server.port);

            let config = self.config.clone();
            let (thread_tx, rx) = mpsc::channel::<ServerMessage>();
            let (tx2, rx2) = mpsc::channel::<ChildStdin>();
            let thread_tx2 = tx2.clone();

            // Minecraft サーバスレッド
            thread::spawn(move || {
                let server_config = config.server;

                // Minecraft サーバを起動する
                let mut server_thread = match executor::mcserver_new(
                    &server_config.jar_file,
                    &server_config.work_dir,
                    &server_config.memory,
                ) {
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

                thread_tx2
                    .send(server_thread.stdin.take().unwrap()) // stdinは必ず存在するのでunwrapしてもよい
                    .unwrap();

                // サーバログを表示して、別スレッドに送信する
                crate::executor::server_log_sender(
                    &thread_tx,
                    server_thread.stdout.take().unwrap(), // stdoutは必ず存在するのでunwrapしてもよい
                );

                executor::close_port(server_config.port);
                thread_tx.send(ServerMessage::Exit).unwrap();
            });

            // Minecraftサーバへの標準入力 (stdin) を取得する
            // stdinを取得するまで次に進まない
            let mut stdin = self.thread_stdin.lock().await;
            *stdin = Some(rx2.recv().unwrap());

            {
                let http = Arc::clone(&self.http);
                let channel = self.config.permission.channel_id;
                let stdin = Arc::clone(&self.thread_stdin);
                let inputed = Arc::clone(&self.command_inputed);
                let thread_id = Arc::clone(&self.thread_id);

                let tokio_handle = tokio::runtime::Handle::current();

                // メッセージ処理を行うスレッド
                thread::spawn(move || {
                    for v in rx {
                        let http = Arc::clone(&http);
                        let stdin = Arc::clone(&stdin);
                        let inputed = Arc::clone(&inputed);
                        let thread_id = Arc::clone(&thread_id);

                        tokio_handle.spawn(async move {
                            match v {
                                ServerMessage::Exit => {
                                    println!("サーバが停止しました。");
                                    let mut stdin = stdin.lock().await;
                                    *stdin = None;
                                    MessageSender::send("終了しました", &http, channel)
                                        .await;
                                }
                                ServerMessage::Done => {
                                    let invoked_message = MessageSender::send(
                                        "サーバが起動しました！サーバログをスレッドから確認できます。",
                                        &http,
                                        channel,
                                    )
                                    .await
                                    .unwrap();

                                    let thread = ChannelId(channel)
                                        .create_public_thread(&http, invoked_message, |v| {
                                            v.name(format!(
                                                "{} Minecraftサーバログ {}",
                                                RUNNING_INDICATER,
                                                chrono::Local::now().format("%Y/%m/%d %H:%M")
                                            ))
                                            .auto_archive_duration(60)
                                        })
                                        .await
                                        .unwrap();

                                    let mut thread_id = thread_id.lock().await;
                                    *thread_id = Some(thread.id.0);
                                }
                                ServerMessage::Info(message) => {
                                    // ユーザからコマンドの入力があった時のみ返信する
                                    let mut inputed = inputed.lock().await;
                                    if *inputed {
                                        MessageSender::send(
                                            format!("```{}\n```", message),
                                            &http,
                                            channel,
                                        )
                                        .await;

                                        *inputed = false;
                                    }

                                    // スレッドが設定されているなら、スレッドに送信する
                                    let thread_id = thread_id.lock().await;
                                        if let Some(v) = *thread_id {
                                        MessageSender::send(message, &http, v).await;
                                    }
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
            return;
        }

        //コマンド入力
        if command == "mcc" {
            if args.len() == 0 {
                self.send("引数を入力して下さい！").await;
                return;
            }

            let stdin = self.thread_stdin.lock().await;

            match stdin.as_ref() {
                Some(mut v) => {
                    v.write_all(format!("{}\n", args.join(" ")).as_bytes())
                        .unwrap();
                    self.send("コマンドを送信しました").await;

                    let mut inputed = self.command_inputed.lock().await;
                    *inputed = true;
                }
                None => {
                    self.send("起動していません！").await;
                }
            }

            return;
        }

        // サーバ停止コマンド
        if command == "mcend" {
            let mut stdin = self.thread_stdin.lock().await;
            let mut inputed = self.command_inputed.lock().await;
            let mut thread_id = self.thread_id.lock().await;

            match stdin.as_ref() {
                Some(mut v) => {
                    println!("stopping...");
                    self.send("終了しています……").await;
                    v.write_all(b"stop\n").unwrap();

                    if let Ok(Channel::Guild(channel)) =
                        &self.http.get_channel(thread_id.unwrap()).await
                    {
                        let name = channel.name();

                        channel
                            .edit_thread(&self.http, |thread| {
                                thread
                                    .name(name.replace(RUNNING_INDICATER, LOG_INDICATER))
                                    .archived(true)
                            })
                            .await
                            .ok();
                    }

                    *stdin = None;
                    *inputed = false;
                    *thread_id = None;
                }
                None => {
                    self.send("起動していません！").await;
                }
            }

            return;
        }

        // クライアント停止コマンド
        if command == "mcsvend" {
            self.send("クライアントを終了しました。").await;
            exit(0);
        }

        self.send("存在しないコマンドです。").await;
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("Discordに接続しました: {}", ready.user.name);
    }
}
