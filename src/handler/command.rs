use super::Handler;
use super::MessageSender;
use crate::executor;
use crate::types::ServerMessage;
use chrono;
use serenity::model::channel::Channel;
use serenity::model::prelude::ChannelId;
use std::process::ChildStdin;
use std::sync::{mpsc, Arc};
use std::thread;

// スレッド名の前につける稼働状況
const RUNNING_INDICATER: &str = "[🏃稼働中]";
const LOG_INDICATER: &str = "🗒️";

pub fn parse_command(message: &str) -> Option<Vec<&str>> {
    if message.len() <= 1 || !message.starts_with("!") {
        return None;
    }

    let message = message[1..].split(' ');
    let args: Vec<&str> = message.collect();
    Some(args)
}

pub async fn mcstart(handler: &Handler) {
    // 標準入力が存在するなら, 既に起動しているのでreturnする
    if handler.is_server_running().await {
        handler.send_message("すでに起動しています！").await;
        return;
    }

    handler.send_message("開始しています……").await;

    #[cfg(target_os = "windows")]
    executor::open_port(handler.config.server.port);

    let config = handler.config.clone();
    let (thread_tx, rx) = mpsc::channel::<ServerMessage>();
    let (thread_tx2, rx2) = mpsc::channel::<ChildStdin>();

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
            server_thread.stderr.take().unwrap(),
        );

        #[cfg(target_os = "windows")]
        executor::close_port(server_config.port);

        thread_tx.send(ServerMessage::Exit).unwrap();
    });

    // Minecraftサーバへの標準入力 (stdin) を取得する
    // stdinを取得するまで次に進まない
    let listner = executor::mcsv::StdinSender::new(rx2.recv().unwrap());
    let command_sender = listner.listen();
    let mut stdin = handler.thread_stdin.lock().await;
    *stdin = Some(command_sender.clone());

    // 自動停止システムを起動
    let tx3 = executor::auto_stop_inspect(command_sender, 120, handler.config.server.auto_stop);

    let http = Arc::clone(&handler.http);
    let channel = handler.config.permission.channel_id;
    let show_public_ip = handler.config.client.show_public_ip.unwrap_or(false);
    let stdin = Arc::clone(&handler.thread_stdin);
    let thread_id = Arc::clone(&handler.thread_id);

    let tokio_handle = tokio::runtime::Handle::current();

    // メッセージ処理を行うスレッド
    thread::spawn(move || {
        for v in rx {
            let http = Arc::clone(&http);
            let thread_id = Arc::clone(&thread_id);
            let tx3 = tx3.clone();

            tokio_handle.spawn(async move {
                match v {
                    ServerMessage::Exit => {
                        println!("サーバが停止しました。");

                        let thread_id = thread_id.lock().await;

                        if let Some(v) = *thread_id {
                            if let Ok(Channel::Guild(channel)) = &http.get_channel(v).await {
                                let name = channel.name();

                                channel
                                    .edit_thread(&http, |thread| {
                                        thread
                                            .name(name.replace(RUNNING_INDICATER, LOG_INDICATER))
                                            .archived(true)
                                    })
                                    .await
                                    .ok();
                            }
                        }

                        MessageSender::send("終了しました", &http, channel).await;
                    }
                    ServerMessage::Done => {
                        let invoked_message = MessageSender::send(
                            "サーバが起動しました！サーバログをスレッドから確認できます。",
                            &http,
                            channel,
                        )
                        .await
                        .unwrap();

                        if show_public_ip {
                            if let Some(ip) = public_ip::addr_v4().await {
                                MessageSender::send(
                                    format!("サーバアドレスは `{}` です。", ip),
                                    &http,
                                    channel,
                                )
                                .await;
                            } else {
                                println!("IPv4アドレスを取得できませんでした。");
                            }
                        }

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
                        if message.contains("joined the game") {
                            tx3.send(1).ok();
                        } else if message.contains("left the game") {
                            tx3.send(-1).ok();
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
                    }
                }
            });
        }
        let mut stdin = stdin.blocking_lock();
        *stdin = None;
    });
}

/// Discordで送信されたコマンドをMinecraftサーバに送信します。
pub async fn send_command_to_server(handler: &Handler, args: Vec<&str>) {
    if args.len() == 0 {
        handler.send_message("引数を入力して下さい！").await;
        return;
    }

    let mut stdin = handler.thread_stdin.lock().await;
    if stdin.is_some() {
        stdin.as_mut().unwrap().send(args.join(" ")).unwrap();
        handler.send_message("コマンドを送信しました").await;
    } else {
        handler.send_message("起動していません！").await;
    }
}

pub async fn send_stop_to_server(handler: &Handler) {
    let mut stdin = handler.thread_stdin.lock().await;

    if stdin.is_some() {
        stdin.as_mut().unwrap().send("stop".to_string()).unwrap();

        println!("stopping...");
        handler.send_message("終了しています……").await;

        *stdin = None;
    } else {
        handler.send_message("起動していません！").await;
    }
}

pub async fn mcsvend(handler: &Handler) {
    handler.send_message("クライアントを終了しました。").await;
    std::process::exit(0);
}

#[cfg(test)]
mod test {
    use crate::handler::command::parse_command;

    #[test]
    fn parse_command_correctly() {
        let message = String::from("!a b c d e");
        let args = parse_command(&message).unwrap();

        assert_eq!(args, vec!["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn parse_command_failed_correctly() {
        // コマンドではないメッセーが送信された時
        assert!(parse_command("hello").is_none());

        // prefixが使用されているが1文字の時
        assert!(parse_command("!").is_none());
    }
}
