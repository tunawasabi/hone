use std::sync::Arc;
use std::thread;

use serenity::http::Http;
use serenity::model::prelude::ChannelId;
use std::sync::mpsc::{sync_channel, RecvTimeoutError::*, SyncSender};
use std::time::Duration;

const MESSAGE_INTERVAL: Duration = Duration::from_millis(800);
const MESSAGE_NUMBER_THRESHOLD: usize = 10;
const DISCORD_MESSAGE_LENGTH_LIMIT: usize = 900;

pub struct LogSender {
    pub channel_id: ChannelId,
    sender: SyncSender<String>,
}

impl LogSender {
    pub fn new(channel_id: ChannelId, http: Arc<Http>) -> LogSender {
        let (sender, rx) = sync_channel::<String>(MESSAGE_NUMBER_THRESHOLD);

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
                        if LogSender::internal_say(&buf, &http, channel_id)
                            .await
                            .is_err()
                        {
                            break;
                        };

                        // reset buffer
                        buf.clear();
                    }
                }
            });
        });

        LogSender { sender, channel_id }
    }

    /// Send a message to the buffer.
    ///
    /// If the number of messages reach `LENGTH_THRESHOLD` or
    /// no message sent within `MESSAGE_INTERVAL`, send the discord thread the messages.
    pub fn say(&self, message: String) -> Result<(), ()> {
        self.sender.send(message).or(Err(()) as Result<(), ()>)
    }

    async fn internal_say(
        messages: &[String],
        http: &Http,
        thread: ChannelId,
    ) -> Result<serenity::model::prelude::Message, serenity::Error> {
        let messages = messages.concat();

        if messages.len() <= DISCORD_MESSAGE_LENGTH_LIMIT {
            thread.say(http, LogSender::wrap_codeblock(&messages)).await
        } else {
            let messages = &messages[..DISCORD_MESSAGE_LENGTH_LIMIT];
            thread.say(http, LogSender::wrap_codeblock(&format!("{messages}……\n\n出力が長いため、省略されました。Minecraftサーバ側のログを確認してください。"))).await
        }
    }

    fn wrap_codeblock(str: &str) -> String {
        format!("```\n{str}\n```")
    }
}
