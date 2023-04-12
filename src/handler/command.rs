use super::log_sender::LogSender;
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
        handler.send_message("すでに起動しています！").await.ok();
        return;
    }

    // サーバログを出力するスレッドを作成する
    {
        let start_message = handler.send_message("開始しています……").await.unwrap();

        let thread = start_message
            .channel_id
            .create_public_thread(&handler.http, start_message.id, |v| {
                v.name(format!(
                    "{} Minecraftサーバログ {}",
                    RUNNING_INDICATER,
                    chrono::Local::now().format("%Y/%m/%d %H:%M")
                ))
                .auto_archive_duration(60)
            })
            .await
            .unwrap();

        let mut thread_id = handler.log_thread.lock().await;
        *thread_id = Some(LogSender::new(thread.id, Arc::clone(&handler.http)));
    }

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
    let player_notifier = if handler.config.server.auto_stop {
        Some(executor::auto_stop_inspect(command_sender, 180))
    } else {
        None
    };

    let http = Arc::clone(&handler.http);
    let channel = ChannelId(handler.config.permission.channel_id);
    let show_public_ip = handler.config.client.show_public_ip.unwrap_or(false);
    let stdin = Arc::clone(&handler.thread_stdin);
    let log_thread = Arc::clone(&handler.log_thread);

    let tokio_handle = tokio::runtime::Handle::current();

    // メッセージ処理を行うスレッド
    thread::spawn(move || {
        for v in rx {
            let http = Arc::clone(&http);
            let log_thread = Arc::clone(&log_thread);
            let player_notifier = player_notifier.clone();

            tokio_handle.spawn(async move {
                match v {
                    ServerMessage::Exit => {
                        println!("サーバが停止しました。");

                        let log_thread = log_thread.lock().await;

                        if let Some(ref log_thread) = *log_thread {
                            if let Ok(Channel::Guild(channel)) =
                                log_thread.channel_id.to_channel(&http).await
                            {
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
                        MessageSender::send(
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

                        if let Some(player_notifier) = player_notifier {
                            player_notifier.start().unwrap();
                        }
                    }
                    ServerMessage::Info(message) => {
                        if let Some(player_notifier) = player_notifier {
                            if message.contains("joined the game") {
                                player_notifier.join().ok();
                            } else if message.contains("left the game") {
                                player_notifier.leave().ok();
                            }
                        }

                        // スレッドが設定されているなら、スレッドに送信する
                        let thread_id = log_thread.lock().await;
                        if let Some(ref v) = *thread_id {
                            v.say(message).ok();
                        }
                    }
                    ServerMessage::Error(e) => {
                        MessageSender::send(
                            format!("エラーが発生しました:\n```{}\n```", e),
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
        handler.send_message("引数を入力して下さい！").await.ok();
        return;
    }

    let mut stdin = handler.thread_stdin.lock().await;

    if let Some(stdin) = stdin.as_mut() {
        let res = stdin.send(args.join(" "));
        match res {
            Ok(_) => {
                handler.send_message("コマンドを送信しました").await.ok();
            }
            Err(err) => {
                handler
                    .send_message(format!("コマンドを送信できませんでした。\n{}", err))
                    .await
                    .ok();
            }
        };
    } else {
        handler.send_message("起動していません！").await.ok();
    }
}

pub async fn send_stop_to_server(handler: &Handler) {
    let mut stdin = handler.thread_stdin.lock().await;

    if let Some(stdin) = stdin.as_mut() {
        let res = stdin.send("stop".to_string());
        match res {
            Ok(_) => {
                println!("stopping...");
                handler.send_message("終了しています……").await.ok();
            }
            Err(err) => {
                handler
                    .send_message(format!("終了できませんでした。mcsv-handler-discordを再起動する必要があります。\n{}", err))
                    .await.ok();
            }
        };
    } else {
        handler.send_message("起動していません！").await.ok();
    }

    *stdin = None;
}

pub async fn mcsvend(handler: &Handler) {
    handler
        .send_message("クライアントを終了しました。")
        .await
        .ok();
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
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
