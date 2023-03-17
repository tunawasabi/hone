use crate::types::ServerMessage;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::process::ChildStderr;
use std::process::ChildStdout;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;

pub mod mcsv;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use self::windows::*;

#[cfg(not(target_os = "windows"))]
mod not_windows;
#[cfg(not(target_os = "windows"))]
pub use self::not_windows::*;

mod auto_stop;
pub use auto_stop::*;

/// Minecraftサーバを起動します。
pub fn mcserver_new(jar_file: &str, work_dir: &str, memory: &str) -> io::Result<Child> {
    self::command_new("java")
        .current_dir(work_dir)
        .arg(format!("-Xmx{}", memory))
        .arg(format!("-Xms{}", memory))
        .arg("-jar")
        .arg(jar_file)
        .arg("nogui")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
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
