use std::{
    sync::mpsc::{channel, RecvTimeoutError::*, SendError, Sender},
    thread, time,
};

type PlayerNotifierResult = Result<(), ()>;

/// Player joining/leaving notifier.
#[derive(Clone)]
pub struct PlayerNotifier(Sender<PlayerNotification>);
enum PlayerNotification {
    Join,
    Leave,
    Start,
}

impl PlayerNotifier {
    fn notifier_err_from(res: Result<(), SendError<PlayerNotification>>) -> Result<(), ()> {
        if res.is_err() {
            return Err(());
        };

        Ok(())
    }

    /// Increment the player count.
    pub fn join(&self) -> PlayerNotifierResult {
        PlayerNotifier::notifier_err_from(self.0.send(PlayerNotification::Join))
    }

    /// Decrement the player count.
    pub fn leave(&self) -> PlayerNotifierResult {
        PlayerNotifier::notifier_err_from(self.0.send(PlayerNotification::Leave))
    }

    /// Start watching player joining/leaving.
    pub fn start(&self) -> PlayerNotifierResult {
        PlayerNotifier::notifier_err_from(self.0.send(PlayerNotification::Start))
    }
}

pub fn auto_stop_inspect(stdin: Sender<String>, sec: u64) -> PlayerNotifier {
    let (tx, rx) = channel();

    thread::spawn(move || {
        let mut watching = false;
        let mut players = 0i32;

        loop {
            match rx.recv_timeout(time::Duration::from_secs(sec)) {
                Ok(v) => {
                    // メッセージが送信された時点でサーバは開始されていると判断する
                    watching = true;

                    match v {
                        PlayerNotification::Join => players += 1,
                        PlayerNotification::Leave => players -= 1,
                        _ => {}
                    };

                    println!("There is/are {} players", players)
                }
                Err(err) => match err {
                    Timeout => {
                        if watching && players == 0 {
                            println!("自動終了します……");
                            stdin.send("stop".to_string()).ok();
                            break;
                        }
                    }
                    Disconnected => {
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
        counter.start().unwrap();

        assert_eq!(rx.recv().unwrap(), "stop");
    }

    #[test]
    fn not_stop_when_watching_disabled() {
        let (tx, _) = mpsc::channel();

        #[allow(unused_variables)]
        let counter = auto_stop_inspect(tx, 1);
        thread::sleep(Duration::from_secs(2));

        assert!(counter.join().is_ok());
    }
}
