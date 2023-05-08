use std::{io::Write, process::ChildStdin, sync::mpsc, thread};

pub struct StdinSender {
    stdin: ChildStdin,
}

impl StdinSender {
    pub fn new(stdin: ChildStdin) -> StdinSender {
        StdinSender { stdin }
    }

    pub fn listen(mut self) -> mpsc::Sender<String> {
        let (sender, receiver) = mpsc::channel::<String>();

        thread::spawn(move || {
            for v in receiver {
                // write_allでコマンドを実行させるために最後に改行を加える
                if self.stdin.write_all(format!("{}\n", v).as_bytes()).is_err() {
                    break;
                };
            }
        });

        sender
    }
}
