use std::sync::Arc;
use std::thread;

use serenity::all::{CreateThread, EditThread, GuildChannel, Message};
use serenity::http::Http;
use serenity::model::prelude::ChannelId;
use serenity::Result;
use std::sync::mpsc::{sync_channel, RecvTimeoutError::*, SyncSender};
use std::time::Duration;

const MESSAGE_INTERVAL: Duration = Duration::from_millis(800);
const MESSAGE_NUMBER_THRESHOLD: usize = 10;
const DISCORD_MESSAGE_LENGTH_LIMIT: usize = 900;

// スレッド名の前につける稼働状況
const RUNNING_INDICATOR: &str = "[🏃稼働中]";
const LOG_INDICATOR: &str = "🗒️";

pub struct LogSessionGuildChannel {
    channel: GuildChannel,
    sender: SyncSender<String>,
}

impl LogSessionGuildChannel {
    pub async fn new(start_msg: Message, http: Arc<Http>) -> Self {
        let log_thread_name = format!(
            "{RUNNING_INDICATOR} Minecraftサーバログ {}",
            chrono::Local::now().format("%Y/%m/%d %H:%M")
        );
        let log_thread_builder = CreateThread::new(log_thread_name)
            .auto_archive_duration(serenity::all::AutoArchiveDuration::OneHour);
        let log_thread = start_msg
            .channel_id
            .create_thread_from_message(&http, start_msg.id, log_thread_builder)
            .await
            .unwrap();

        let (sender, rx) = sync_channel::<String>(MESSAGE_NUMBER_THRESHOLD);
        let channel_id = log_thread.id;

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let mut buf: Vec<String> = Vec::new();
                loop {
                    let mut send_flag = false;

                    match rx.recv_timeout(MESSAGE_INTERVAL) {
                        Ok(v) => {
                            buf.push(v);

                            if buf.len() >= MESSAGE_NUMBER_THRESHOLD {
                                send_flag = true;
                            }
                        }
                        Err(err) => match err {
                            Timeout => {
                                if !buf.is_empty() {
                                    send_flag = true;
                                }
                            }
                            Disconnected => break,
                        },
                    };

                    if send_flag {
                        if Self::internal_say(&buf, &http, channel_id).await.is_err() {
                            break;
                        };

                        // reset buffer
                        buf.clear();
                    }
                }
            });
        });

        Self {
            sender,
            channel: log_thread,
        }
    }

    /// Send a message to the buffer.
    ///
    /// If the number of messages reach `LENGTH_THRESHOLD` or
    /// no message sent within `MESSAGE_INTERVAL`, send the discord thread the messages.
    pub fn say(&self, message: String) -> Result<(), ()> {
        self.sender.send(message).or(Err(()) as Result<(), ()>)
    }

    pub async fn archive(&mut self, http: &Http) -> Result<()> {
        let name = self.channel.name();
        let edit_thread_builder = EditThread::new()
            .name(name.replace(RUNNING_INDICATOR, LOG_INDICATOR))
            .archived(true);

        self.channel.edit_thread(&http, edit_thread_builder).await
    }

    async fn internal_say(
        messages: &[String],
        http: &Http,
        thread: ChannelId,
    ) -> Result<serenity::model::prelude::Message, serenity::Error> {
        let messages = messages.concat();

        if messages.len() <= DISCORD_MESSAGE_LENGTH_LIMIT {
            thread.say(http, Self::wrap_codeblock(&messages)).await
        } else {
            let messages = &messages[..DISCORD_MESSAGE_LENGTH_LIMIT];
            thread.say(http, Self::wrap_codeblock(&format!("{messages}……\n\n出力が長いため、省略されました。Minecraftサーバ側のログを確認してください。"))).await
        }
    }

    fn wrap_codeblock(str: &str) -> String {
        format!("```\n{str}\n```")
    }
}
