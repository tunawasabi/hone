use std::{
    sync::mpsc::{channel, RecvTimeoutError, SendError, Sender},
    thread, time,
};

type PlayerNotifierResult = Result<(), ()>;

/// Player joining/leaving notifier.
#[derive(Clone)]
pub struct PlayerNotifier(Sender<PlayerNotification>);
enum PlayerNotification {
    Join,
    Leave,
}

impl PlayerNotifier {
    fn notifier_err_from(res: Result<(), SendError<PlayerNotification>>) -> Result<(), ()> {
        if res.is_err() {
            return Err(());
        };

        Ok(())
    }

    /// Send a message that a player joined.
    pub fn join(&self) -> PlayerNotifierResult {
        PlayerNotifier::notifier_err_from(self.0.send(PlayerNotification::Join))
    }

    /// Senda  message that a player left.
    pub fn leave(&self) -> PlayerNotifierResult {
        PlayerNotifier::notifier_err_from(self.0.send(PlayerNotification::Leave))
    }
}

pub fn auto_stop_inspect(stdin: Sender<String>, sec: u64) -> PlayerNotifier {
    let (tx, rx) = channel();

    thread::spawn(move || {
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

                    match v {
                        PlayerNotification::Join => players += 1,
                        PlayerNotification::Leave => players -= 1,
                    };

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

    #[test]
    fn auto_stop_after_all_players_leaved() {
        let (tx, _) = mpsc::channel();
        let r = auto_stop_inspect(tx, 2);

        r.join().unwrap();
        std::thread::sleep(Duration::from_secs(3));
        r.leave().unwrap();
        std::thread::sleep(Duration::from_secs(3));
        assert!(r.join().is_err());
    }

    #[test]
    fn do_not_stop_when_player_is_joining() {
        let (tx, _) = mpsc::channel();
        let r = auto_stop_inspect(tx, 1);

        r.join().unwrap();
        std::thread::sleep(Duration::from_secs(2));
        assert!(r.join().is_ok());
    }

    #[test]
    fn auto_stop_when_timeouted_and_no_player() {
        let (tx, rx) = mpsc::channel();

        #[allow(unused_variables)]
        let counter = auto_stop_inspect(tx, 1);

        assert_eq!(rx.recv().unwrap(), "stop");
    }
}
