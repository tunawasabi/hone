use std::{
    sync::mpsc::{channel, RecvTimeoutError, SendError, Sender},
    thread, time,
};

type PlayerNotifierResult = Result<(), SendError<i32>>;

/// Player joining/leaving notifier.
#[derive(Clone)]
pub struct PlayerNotifier(Sender<i32>);

impl PlayerNotifier {
    /// Send a message that a player joined.
    pub fn join(&self) -> PlayerNotifierResult {
        self.0.send(1)
    }

    /// Senda  message that a player left.
    pub fn leave(&self) -> PlayerNotifierResult {
        self.0.send(-1)
    }
}

pub fn auto_stop_inspect(stdin: Sender<String>, sec: u64, is_enabled: bool) -> PlayerNotifier {
    let (tx, rx) = channel();

    thread::spawn(move || {
        if !is_enabled {
            return;
        }

        // まだサーバが起動完了していな時に
        // 初期人数を-1とする
        let mut players = -1i32;

        loop {
            match rx.recv_timeout(time::Duration::from_secs(sec)) {
                Ok(v) => {
                    // 初期人数が-1ならば、
                    // 0に修正する
                    if players < 0 {
                        players = 0
                    }

                    players += v;
                    println!("There is/are {} players", players)
                }
                Err(err) => match err {
                    RecvTimeoutError::Timeout => {
                        if players == 0 {
                            println!("自動終了します……");
                            stdin.send("stop".to_string()).ok();
                            break;
                        }

                        if players < 0 {
                            players = 0
                        }
                    }
                    RecvTimeoutError::Disconnected => {
                        break;
                    }
                },
            }
        }
    });

    PlayerNotifier(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    impl PlayerNotifier {
        /// Send a empty message. This doesn't modify the count,
        /// but it extends the duration of server running when there are no players.
        pub fn ping(&self) -> PlayerNotifierResult {
            self.0.send(0)
        }
    }

    #[test]
    fn auto_stop_after_all_players_leaved() {
        let (tx, _) = mpsc::channel();
        let r = auto_stop_inspect(tx, 2, true);

        r.join().unwrap();
        std::thread::sleep(Duration::from_secs(2));
        r.leave().unwrap();
        std::thread::sleep(Duration::from_secs(2));
        assert!(r.ping().is_err());
    }

    #[test]
    fn do_not_stop_when_player_is_joining() {
        let (tx, _) = mpsc::channel();
        let r = auto_stop_inspect(tx, 1, true);

        r.join().unwrap();
        std::thread::sleep(Duration::from_secs(2));
        assert!(r.ping().is_ok());
    }

    #[test]
    fn channel_closed_when_auto_stop_disabled() {
        let (tx, _) = mpsc::channel();
        let r = auto_stop_inspect(tx, 120, false);

        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(r.ping().is_err());
    }

    #[test]
    fn auto_stop_when_timeouted_and_no_player() {
        let (tx, rx) = mpsc::channel();

        #[allow(unused_variables)]
        let counter = auto_stop_inspect(tx, 1, true);

        assert_eq!(rx.recv().unwrap(), "stop");
    }
}
