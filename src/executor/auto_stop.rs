use std::{sync::mpsc, thread, time};

pub fn auto_stop_inspect(stdin: mpsc::Sender<String>, is_enabled: bool) -> mpsc::Sender<i32> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        if !is_enabled {
            return;
        }

        // まだサーバが起動完了していな時に
        // 初期人数を-1とする
        let mut players = -1i32;

        loop {
            match rx.recv_timeout(time::Duration::from_secs(120)) {
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
                            stdin.send("stop\n".to_string()).ok();
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
