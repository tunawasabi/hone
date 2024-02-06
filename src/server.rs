use crate::types::ServerMessage;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Stdio};
use std::sync::mpsc;
use std::thread;

pub mod mcsv;

mod create;
pub use create::*;

mod auto_stop;
pub use auto_stop::*;

pub struct ServerBuilder {
    jar_file: Option<String>,
    work_dir: Option<String>,
    memory: Option<String>,
}

pub struct Server {
    #[allow(dead_code)]
    process: Child,
    pub stdin: ChildStdin,
    pub stdout: ChildStdout,
    pub stderr: ChildStderr,
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            jar_file: None,
            work_dir: None,
            memory: None,
        }
    }

    pub fn jar_file(mut self, jar_file: &str) -> Self {
        self.jar_file = Some(jar_file.to_string());
        self
    }

    pub fn work_dir(mut self, work_dir: &str) -> Self {
        self.work_dir = Some(work_dir.to_string());
        self
    }

    pub fn memory(mut self, memory: &str) -> Self {
        self.memory = Some(memory.to_string());
        self
    }

    pub fn build(self) -> io::Result<Server> {
        let jar_file = self.jar_file.expect("jar_file is not set");
        let work_dir = self.work_dir.expect("work_dir is not set");
        let memory = self.memory.expect("memory is not set");

        let mut child_proc = mcserver_new(&jar_file, &work_dir, &memory)?;

        Ok(Server {
            stdin: child_proc.stdin.take().unwrap(),
            stdout: child_proc.stdout.take().unwrap(),
            stderr: child_proc.stderr.take().unwrap(),
            process: child_proc,
        })
    }
}

/// Minecraftサーバを起動します。
fn mcserver_new(jar_file: &str, work_dir: &str, memory: &str) -> io::Result<Child> {
    let xmx = &format!("-Xmx{}", memory);
    let xms = &format!("-Xms{}", memory);

    let java_command = ["java", xmx, xms, "-jar", jar_file, "nogui"];
    let mut cmd = self::command_new(&java_command.join(" "));

    cmd.current_dir(work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    cmd.spawn()
}

pub fn server_log_sender(
    sender: &mpsc::Sender<ServerMessage>,
    stdout: ChildStdout,
    stderr: ChildStderr,
) {
    let mut bufread = BufReader::new(stdout);
    let mut buf = String::new();

    // 標準エラー出力を監視するスレッド
    let err_sender = sender.clone();
    thread::spawn(move || {
        let mut bufread = BufReader::new(stderr);
        let mut buf = String::new();

        while let Ok(v) = bufread.read_line(&mut buf) {
            if v == 0 {
                break;
            }

            print!("[Minecraft] {}", buf);
            err_sender.send(ServerMessage::Error(buf.clone())).ok();

            buf.clear();
        }
    });

    // 標準出力を監視する
    while let Ok(lines) = bufread.read_line(&mut buf) {
        if lines == 0 {
            break;
        }

        // JVMからの出力をそのまま出力する。
        // 改行コードが既に含まれているのでprint!マクロを使う
        print!("[Minecraft] {}", buf);

        // サーバの起動が完了したとき
        if buf.contains("Done") {
            sender.send(ServerMessage::Done).ok();
        }

        // EULAへの同意が必要な時
        if buf.contains("You need to agree") {
            sender
                .send(ServerMessage::Error(
                    "サーバを開始するには、EULAに同意する必要があります。eula.txtを編集してください。"
                        .to_string(),
                ))
                .ok();
        }

        // Minecraftサーバ終了を検知
        if buf.contains("All dimensions are saved") {
            break;
        }

        sender.send(ServerMessage::Info(buf.clone())).unwrap();
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::mcserver_new;

    #[test]
    fn mcsv_stdio_must_piped() {
        let mcsv = mcserver_new("dummy", "./", "").unwrap();

        assert!(mcsv.stdout.is_some(), "stdout is not piped");
        assert!(mcsv.stderr.is_some(), "stderr is not piped");
        assert!(mcsv.stdin.is_some(), "stdin is not piped");
    }
}
