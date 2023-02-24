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
                if let Err(_) = self.stdin.write_all(v.as_bytes()) {
                    break;
                };
            }
        });

        sender
    }
}
