use std::sync::Arc;
use std::thread;

use serenity::http::Http;
use serenity::model::prelude::ChannelId;
use std::sync::mpsc::{sync_channel, RecvTimeoutError::*, SyncSender};
use std::time::Duration;

const MESSAGE_INTERVAL: u64 = 1;
const LENGTH_THRESHOLD: usize = 5;
pub struct LogSender {
    pub channel_id: ChannelId,
    sender: SyncSender<String>,
}

impl LogSender {
    pub fn new(channel_id: ChannelId, http: Arc<Http>) -> LogSender {
        let (sender, rx) = sync_channel(LENGTH_THRESHOLD);

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let mut buf: Vec<String> = Vec::new();

            rt.block_on(async {
                loop {
                    match rx.recv_timeout(Duration::from_secs(MESSAGE_INTERVAL)) {
                        Ok(v) => {
                            buf.push(v);

                            if buf.len() >= LENGTH_THRESHOLD {
                                LogSender::internal_say(&mut buf, &http, channel_id)
                                    .await
                                    .ok();
                            }
                        }
                        Err(err) => match err {
                            Timeout => {
                                if !buf.is_empty() {
                                    LogSender::internal_say(&mut buf, &http, channel_id)
                                        .await
                                        .ok();
                                }
                            }
                            Disconnected => break,
                        },
                    };
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
        messages: &mut Vec<String>,
        http: &Http,
        thread: ChannelId,
    ) -> Result<serenity::model::prelude::Message, serenity::Error> {
        let res = thread.say(http, messages.concat()).await;
        messages.clear();

        res
    }
}
