use std::{sync::mpsc, thread, time};

pub fn auto_stop_inspect(
    stdin: mpsc::Sender<String>,
    sec: u64,
    is_enabled: bool,
) -> mpsc::Sender<i32> {
    let (tx, rx) = mpsc::channel();

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
                    mpsc::RecvTimeoutError::Timeout => {
                        if players == 0 {
                            println!("自動終了します……");
                            stdin.send("stop".to_string()).ok();
                            break;
                        }

                        if players < 0 {
                            players = 0
                        }
                    }
                    mpsc::RecvTimeoutError::Disconnected => {
                        println!("auto_stop_inspect_sender dropped");
                        break;
                    }
                },
            }
        }
    });

    tx
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::auto_stop_inspect;

    #[test]
    fn auto_stop_after_all_players_leaved() {
        let (tx, _) = mpsc::channel();
        let r = auto_stop_inspect(tx, 2, true);

        r.send(1).unwrap();
        std::thread::sleep(Duration::from_secs(3));
        r.send(-1).unwrap();
        std::thread::sleep(Duration::from_secs(3));
        assert!(r.send(0).is_err());
    }

    #[test]
    fn do_not_stop_when_player_is_joining() {
        let (tx, _) = mpsc::channel();
        let r = auto_stop_inspect(tx, 1, true);

        r.send(1).unwrap();
        std::thread::sleep(Duration::from_secs(2));
        assert!(r.send(0).is_ok());
    }

    #[test]
    fn channel_closed_when_auto_stop_disabled() {
        let (tx, _) = mpsc::channel();
        let r = auto_stop_inspect(tx, 120, false);

        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(r.send(0).is_err());
    }

    #[test]
    fn auto_stop_when_timeouted_and_no_player() {
        let (tx, rx) = mpsc::channel();

        #[allow(unused_variables)]
        let counter = auto_stop_inspect(tx, 1, true);

        assert_eq!(rx.recv().unwrap(), "stop");
    }
}
