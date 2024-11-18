use super::log_sender::LogSessionGuildChannel;
use super::observer::observe;
use super::Handler;
use crate::server::{auto_stop_inspect, ServerBuilder};
use std::sync::Arc;

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

        let channel = self.config.permission.channel_id;

        // Minecraft サーバスレッド
        let Ok(server) = ServerBuilder::new()
            .jar_file(&self.config.server.jar_file)
            .work_dir(&self.config.server.work_dir)
            .memory(&self.config.server.memory)
            .build()
        else {
            channel
                .say(
                    &self.http,
                    "Minecraftサーバのプロセスを起動できませんでした",
                )
                .await
                .ok();
            return;
        };

        // サーバログを表示して、別スレッドに送信する
        let srv_msg_rx = server.logs();

        // Minecraftサーバへの標準入力 (stdin) を取得する
        let command_sender = server.stdin_sender();
        let mut stdin = self.thread_stdin.lock().await;
        *stdin = Some(command_sender.clone());

        // 自動停止システムを起動
        let player_notifier = if self.config.server.auto_stop {
            Some(auto_stop_inspect(command_sender, 180))
        } else {
            None
        };

        let http = Arc::clone(&self.http);
        let stdin = Arc::clone(&self.thread_stdin);
        let log_thread = Arc::clone(&self.log_thread);
        observe(
            srv_msg_rx,
            http,
            stdin,
            channel,
            log_thread,
            player_notifier,
        )
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
                    .send_message(format!(
                        "終了できませんでした。honeを再起動する必要があります。\n{}",
                        err
                    ))
                    .await
                    .ok();
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
