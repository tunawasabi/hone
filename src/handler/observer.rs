use super::LogSessionGuildChannel;
use crate::{context::ConfigContext, server::PlayerNotifier, types::ServerMessage};
use serenity::{http::Http, model::prelude::ChannelId, prelude::Mutex};
use std::{
    sync::mpsc::{Receiver, Sender},
    sync::Arc,
    thread,
};

/// Observe the server's standard output and send messages to Discord.
pub fn observe(
    srv_msg_rx: Receiver<ServerMessage>,
    http: Arc<Http>,
    stdin: Arc<Mutex<Option<Sender<String>>>>,
    channel: ChannelId,
    log_thread: Arc<Mutex<Option<LogSessionGuildChannel>>>,
    player_notifier: Option<PlayerNotifier>,
) {
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            use ServerMessage::*;

            let config = ConfigContext::get();
            for v in srv_msg_rx {
                match v {
                    Exit => {
                        println!("サーバが停止しました。");

                        let mut log_thread = log_thread.lock().await;

                        if let Some(ref mut log_thread) = *log_thread {
                            log_thread.archive(&http).await.ok();
                        }

                        channel.say(&http, "終了しました").await.ok();
                    }
                    Done => {
                        channel
                            .say(
                                &http,
                                "サーバが起動しました！サーバログをスレッドから確認できます。",
                            )
                            .await
                            .ok();

                        if config.client.show_public_ip.unwrap_or(false) {
                            if let Some(ip) = public_ip::addr_v4().await {
                                channel
                                    .say(&http, format!("サーバアドレスは `{}` です。", ip))
                                    .await
                                    .ok();
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
                        channel
                            .say(&http, format!("エラーが発生しました:\n```{}\n```", e))
                            .await
                            .ok();
                    }
                }
            }
        });

        // FIXME: Windows限定機能の整理
        #[cfg(target_os = "windows")]
        {
            use crate::context::ConfigContext;

            let config = ConfigContext::get();
            crate::server::close_port(config.server.port);
        }

        let mut log_thread = log_thread.blocking_lock();
        *log_thread = None;
        let mut stdin = stdin.blocking_lock();
        *stdin = None;
    });
}
