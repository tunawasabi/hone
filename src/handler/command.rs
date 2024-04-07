use super::log_sender::LogSender;
use super::Handler;
use super::MessageSender;
use crate::server::{auto_stop_inspect, mcsv, ServerBuilder};
use crate::types::ServerMessage;
use serenity::builder::CreateThread;
use serenity::builder::EditThread;
use serenity::model::channel::Channel;
use serenity::model::prelude::ChannelId;
use std::sync::Arc;
use std::thread;

// スレッド名の前につける稼働状況
const RUNNING_INDICATER: &str = "[🏃稼働中]";
const LOG_INDICATER: &str = "🗒️";

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
            let start_message = self.send_message("開始しています……").await.unwrap();

            let log_thread_name = format!(
                "{RUNNING_INDICATER} Minecraftサーバログ {}",
                chrono::Local::now().format("%Y/%m/%d %H:%M")
            );
            let log_thread_builder = CreateThread::new(log_thread_name)
                .auto_archive_duration(serenity::all::AutoArchiveDuration::OneHour);

            let log_thread = start_message
                .channel_id
                .create_thread_from_message(&self.http, start_message.id, log_thread_builder)
                .await
                .unwrap();

            let mut thread_id = self.log_thread.lock().await;
            *thread_id = Some(LogSender::new(log_thread.id, Arc::clone(&self.http)));
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

                            if let Some(ref log_thread) = *log_thread {
                                if let Ok(Channel::Guild(mut channel)) =
                                    log_thread.channel_id.to_channel(&http).await
                                {
                                    let name = channel.name();
                                    let edit_thread_builder = EditThread::new()
                                        .name(name.replace(RUNNING_INDICATER, LOG_INDICATER))
                                        .archived(true);

                                    channel.edit_thread(&http, edit_thread_builder).await.ok();
                                }
                            }

                            *log_thread = None;
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
