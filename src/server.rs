use crate::types::ServerMessage;
use std::{
    cell::Cell,
    io::{self, BufRead, BufReader},
    path::PathBuf,
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Stdio},
    sync::mpsc,
    thread,
};

mod stdin_sender;

mod create;
pub use create::*;

mod auto_stop;
pub use auto_stop::*;

pub struct ServerBuilder {
    jar_file: Option<PathBuf>,
    work_dir: Option<PathBuf>,
    memory: Option<String>,
}

pub struct Server {
    #[allow(dead_code)]
    proc: Child,
    stdin: Cell<Option<ChildStdin>>,
    stdout: Cell<Option<ChildStdout>>,
    stderr: Cell<Option<ChildStderr>>,
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            jar_file: None,
            work_dir: None,
            memory: None,
        }
    }

    pub fn jar_file(mut self, jar_file: PathBuf) -> Self {
        self.jar_file = Some(jar_file);
        self
    }

    pub fn work_dir(mut self, work_dir: PathBuf) -> Self {
        self.work_dir = Some(work_dir);
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
    fn mcserver_new(jar_file: &PathBuf, work_dir: &PathBuf, memory: &str) -> io::Result<Server> {
        let xmx = &format!("-Xmx{}", memory);
        let xms = &format!("-Xms{}", memory);

        let mut cmd = self::command_new();
        cmd.arg("java")
            .args([xmx, xms])
            .arg("-jar")
            .arg(jar_file)
            .arg("nogui")
            .current_dir(work_dir)
            // `stdin`, `stdout`, `stderr` must be set to `piped` to read/write from/to the child process.
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child_proc = cmd.spawn()?;

        // `stdin`, `stdout`, `stderr` are piped, so we can unwrap them safely.
        let stdin = Cell::new(Some(child_proc.stdin.take().unwrap()));
        let stdout = Cell::new(Some(child_proc.stdout.take().unwrap()));
        let stderr = Cell::new(Some(child_proc.stderr.take().unwrap()));

        Ok(Self {
            proc: child_proc,
            stdin,
            stdout,
            stderr,
        })
    }

    /// Get stdin sender.
    pub fn stdin_sender(&self) -> mpsc::Sender<String> {
        let stdin = self.stdin.replace(None).expect("stdin is not set");

        stdin_sender::StdinSender::new(stdin).listen()
    }

    /// Get the server logs. You can only call this method once.
    pub fn logs(&self) -> mpsc::Receiver<ServerMessage> {
        let (stdout_tx, rx) = mpsc::channel::<ServerMessage>();
        let stderr_tx = stdout_tx.clone();

        // 標準出力を監視する
        {
            let stdout = self.stdout.replace(None).expect("stdout is not set");

            thread::spawn(move || {
                let mut stdout_reader = BufReader::new(stdout);
                let mut buf = String::new();

                while let Ok(lines) = stdout_reader.read_line(&mut buf) {
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
            let stderr = self.stderr.replace(None).expect("stderr is not set");

            thread::spawn(move || {
                let mut stderr_reader = BufReader::new(stderr);
                let mut buf = String::new();

                while let Ok(v) = stderr_reader.read_line(&mut buf) {
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
