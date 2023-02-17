use crate::types::Config;
use chrono;
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::prelude::ChannelId;
use serenity::prelude::*;
use std::io::{BufRead, BufReader, Write};
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

enum ServerMessage {
    Done,
    Exit,
    Info(String),
    Error(String),
}

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

    async fn send(&self, message: String) {
        let channel = ChannelId(self.config.permission.channel_id);

        if let Err(e) = channel.say(&self.http, message).await {
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
    async fn send(message: String, http: &Http, channel: u64) -> Option<Message> {
        let channel = ChannelId(channel);

        match channel.say(http, message).await {
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
            let (tx2, rx2) = mpsc::channel::<ChildStdin>();
            let thread_tx2 = tx2.clone();

            // Minecraft サーバスレッド
            thread::spawn(move || {
                // Minecraft サーバを起動する
                let mut server_thread =
                    match crate::executor::mcserver_new(&jar_file, &work_dir, &memory) {
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
                    .send(server_thread.stdin.take().unwrap())
                    .unwrap();

                let mut bufread = BufReader::new(server_thread.stdout.as_mut().unwrap());
                // 出力
                let mut buf = String::new();
                loop {
                    if let Ok(lines) = bufread.read_line(&mut buf) {
                        if lines == 0 {
                            break;
                        }

                        // JVMからの出力をそのまま出力する。
                        // 改行コードが既に含まれているのでprint!マクロを使う
                        print!("[Minecraft] {}", buf);

                        // サーバの起動が完了したとき
                        if buf.contains("Done") {
                            thread_tx.send(ServerMessage::Done).unwrap();
                        }

                        // EULAへの同意が必要な時
                        if buf.contains("You need to agree") {
                            thread_tx
                                    .send(ServerMessage::Error(
                                        "サーバを開始するには、EULAに同意する必要があります。eula.txtを編集してください。"
                                            .to_string(),
                                    ))
                                    .unwrap();
                        }

                        // Minecraftサーバ終了を検知
                        if buf.contains("All dimensions are saved") {
                            break;
                        }

                        thread_tx.send(ServerMessage::Info(buf.clone())).unwrap();

                        buf.clear();
                    }
                }

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
                                    MessageSender::send("終了しました".to_string(), &http, channel)
                                        .await;
                                }
                                ServerMessage::Done => {
                                    let invoked_message = MessageSender::send(
                                        "サーバが起動しました！サーバログをスレッドから確認できます。".to_string(),
                                        &http,
                                        channel,
                                    )
                                    .await
                                    .unwrap();

                                    let thread = ChannelId(channel)
                                        .create_public_thread(&http, invoked_message, |v| {
                                            v.name(format!(
                                                "Minecraftサーバログ {}",
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
        }

        //コマンド入力
        if msg.content.starts_with("!mcc") {
            let stdin = self.thread_stdin.lock().await;

            match stdin.as_ref() {
                Some(mut v) => {
                    // 引数部分 (5文字目以降) を取り出す
                    let command = &msg.content[5..];

                    v.write_all(format!("{}\n", command).as_bytes()).unwrap();
                    self.send("コマンドを送信しました".to_string()).await;

                    let mut inputed = self.command_inputed.lock().await;
                    *inputed = true;
                }
                None => {
                    self.send("起動していません！".to_string()).await;
                }
            }

            return;
        }

        // サーバ停止コマンド
        if msg.content == "!mcend" {
            let mut stdin = self.thread_stdin.lock().await;
            let mut inputed = self.command_inputed.lock().await;
            let mut thread_id = self.thread_id.lock().await;

            match stdin.as_ref() {
                Some(mut v) => {
                    println!("stopping...");
                    self.send("終了しています……".to_string()).await;
                    v.write_all(b"stop\n").unwrap();

                    ChannelId(thread_id.unwrap())
                        .edit_thread(&self.http, |thread| thread.archived(true))
                        .await
                        .unwrap();

                    *stdin = None;
                    *inputed = false;
                    *thread_id = None;
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
