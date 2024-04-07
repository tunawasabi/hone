use crate::types::ServerMessage;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::process::{Child, ChildStdin, Stdio};
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
    proc: Child,
    pub stdin: ChildStdin,
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

        let server = Server::mcserver_new(&jar_file, &work_dir, &memory)?;

        Ok(server)
    }
}

impl Server {
    /// Create a new Minecraft server process.
    fn mcserver_new(jar_file: &str, work_dir: &str, memory: &str) -> io::Result<Server> {
        let xmx = &format!("-Xmx{}", memory);
        let xms = &format!("-Xms{}", memory);

        let java_command = ["java", xmx, xms, "-jar", jar_file, "nogui"];
        let mut cmd = self::command_new(&java_command.join(" "));

        // `stdin`, `stdout`, `stderr` must be set to `piped` to read/write from/to the child process.
        cmd.current_dir(work_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child_proc = cmd.spawn()?;

        let stdin = child_proc.stdin.take().unwrap();

        Ok(Self {
            stdin,
            proc: child_proc,
        })
    }

    pub fn logs(&mut self) -> mpsc::Receiver<ServerMessage> {
        let (stdout_tx, rx) = mpsc::channel::<ServerMessage>();
        let stderr_tx = stdout_tx.clone();

        // 標準出力を監視する
        {
            let stdout = self.proc.stdout.take().unwrap();

            thread::spawn(move || {
                let mut bufread = BufReader::new(stdout);
                let mut buf = String::new();

                while let Ok(lines) = bufread.read_line(&mut buf) {
                    if lines == 0 {
                        break;
                    }

                    // JVMからの出力をそのまま出力する。
                    // 改行コードが既に含まれているのでprint!マクロを使う
                    print!("[Minecraft] {}", buf);

                    // サーバの起動が完了したとき
                    if buf.contains("Done") {
                        stdout_tx.send(ServerMessage::Done).ok();
                    }

                    // EULAへの同意が必要な時
                    if buf.contains("You need to agree") {
                        stdout_tx.send(ServerMessage::Error(
                "サーバを開始するには、EULAに同意する必要があります。eula.txtを編集してください。"
                    .to_string(),
            ))
            .ok();
                    }

                    // Minecraftサーバ終了を検知
                    if buf.contains("All dimensions are saved") {
                        break;
                    }

                    stdout_tx.send(ServerMessage::Info(buf.clone())).unwrap();
                    buf.clear();
                }

                stdout_tx.send(ServerMessage::Exit).ok();
            });
        }

        // 標準エラー出力を監視するスレッド
        {
            let stderr = self.proc.stderr.take().unwrap();

            thread::spawn(move || {
                let mut bufread = BufReader::new(stderr);
                let mut buf = String::new();

                while let Ok(v) = bufread.read_line(&mut buf) {
                    if v == 0 {
                        break;
                    }

                    print!("[Minecraft] {}", buf);
                    stderr_tx.send(ServerMessage::Error(buf.clone())).ok();

                    buf.clear();
                }
            });
        }

        rx
    }
}
