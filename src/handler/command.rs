use super::log_sender::LogSessionGuildChannel;
use super::Handler;
use super::MessageSender;
use crate::server::{auto_stop_inspect, mcsv, ServerBuilder};
use crate::types::ServerMessage;
use serenity::model::prelude::ChannelId;
use std::sync::Arc;
use std::thread;

pub fn parse_command(message: &str) -> Option<Vec<&str>> {
    if message.len() <= 1 || !message.starts_with('!') {
        return None;
    }

    let message = message[1..].split(' ');
    let args: Vec<&str> = message.collect();
    Some(args)
}

impl Handler {
    pub async fn mcstart(&self) {
        // 標準入力が存在するなら, 既に起動しているのでreturnする
        if self.is_server_running().await {
            self.send_message("すでに起動しています！").await.ok();
            return;
        }

        // Create a thread to output server logs
        {
            let start_msg = self.send_message("開始しています……").await.unwrap();

            let mut log_thread = self.log_thread.lock().await;
            *log_thread =
                Some(LogSessionGuildChannel::new(start_msg, Arc::clone(&self.http)).await);
        }

        // FIXME: Windows限定機能の整理
        #[cfg(target_os = "windows")]
        let port = self.config.server.port;
        #[cfg(target_os = "windows")]
        crate::server::open_port(port);

        let channel = ChannelId::new(self.config.permission.channel_id);

        // Minecraft サーバスレッド
        let Ok(server) = ServerBuilder::new()
            .jar_file(&self.config.server.jar_file)
            .work_dir(&self.config.server.work_dir)
            .memory(&self.config.server.memory)
            .build()
        else {
            MessageSender::send(
                "Minecraftサーバのプロセスを起動できませんでした",
                &self.http,
                channel,
            )
            .await;
            return;
        };

        // サーバログを表示して、別スレッドに送信する
        let (server, srv_msg_rx) = {
            let mut server = server;
            let srv_msg_rx = server.logs();
            (server, srv_msg_rx)
        };

        // Minecraftサーバへの標準入力 (stdin) を取得する
        let listner = mcsv::StdinSender::new(server.stdin);
        let command_sender = listner.listen();
        let mut stdin = self.thread_stdin.lock().await;
        *stdin = Some(command_sender.clone());

        // 自動停止システムを起動
        let player_notifier = if self.config.server.auto_stop {
            Some(auto_stop_inspect(command_sender, 180))
        } else {
            None
        };

        let http = Arc::clone(&self.http);
        let show_public_ip = self.config.client.show_public_ip.unwrap_or(false);
        let stdin = Arc::clone(&self.thread_stdin);
        let log_thread = Arc::clone(&self.log_thread);

        // メッセージ処理を行うスレッド
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                use ServerMessage::*;

                for v in srv_msg_rx {
                    match v {
                        Exit => {
                            println!("サーバが停止しました。");

                            let mut log_thread = log_thread.lock().await;

                            if let Some(ref mut log_thread) = *log_thread {
                                log_thread.archive(&http).await.ok();
                            }

                            MessageSender::send("終了しました", &http, channel).await;
                        }
                        Done => {
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

                            if let Some(ref player_notifier) = player_notifier {
                                player_notifier.start().unwrap();
                            }
                        }
                        Info(message) => {
                            if let Some(ref player_notifier) = player_notifier {
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
                        Error(e) => {
                            MessageSender::send(
                                format!("エラーが発生しました:\n```{}\n```", e),
                                &http,
                                channel,
                            )
                            .await;
                        }
                    }
                }
            });

            // FIXME: Windows限定機能の整理
            #[cfg(target_os = "windows")]
            crate::server::close_port(port);

            let mut log_thread = log_thread.blocking_lock();
            *log_thread = None;
            let mut stdin = stdin.blocking_lock();
            *stdin = None;
        });
    }
}

/// Discordで送信されたコマンドをMinecraftサーバに送信します。
pub async fn send_command_to_server(handler: &Handler, args: Vec<&str>) {
    if args.is_empty() {
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
