use crate::types::ServerMessage;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::process::ChildStderr;
use std::process::ChildStdout;
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time;
use toml;

use crate::types::Config;

pub mod mcsv;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use self::windows::*;

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

pub fn read_config() -> Result<Config, String> {
    let config = match fs::read_to_string("config.toml") {
        Ok(v) => v,
        Err(err) => return Err(format!("設定ファイルを開くことができませんでした: {}", err)),
    };

    match toml::from_str::<Config>(&config) {
        Ok(config) => Ok(config),
        Err(err) => Err(format!("設定に誤りがあります: {}", err)),
    }
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
            err_sender.send(ServerMessage::Error(buf.clone())).unwrap();

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
            sender.send(ServerMessage::Done).unwrap();
        }

        // EULAへの同意が必要な時
        if buf.contains("You need to agree") {
            sender
                                    .send(ServerMessage::Error(
                                        "サーバを開始するには、EULAに同意する必要があります。eula.txtを編集してください。"
                                            .to_string(),
                                    ))
                                    .unwrap();
        }

        // Minecraftサーバ終了を検知
        if buf.contains("All dimensions are saved") {
            break;
        }

        sender.send(ServerMessage::Info(buf.clone())).unwrap();
        buf.clear();
    }
}

pub fn auto_stop_inspect(stdin: mpsc::Sender<String>) -> mpsc::Sender<i32> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let mut players = 0i32;

        loop {
            match rx.recv_timeout(time::Duration::from_secs(30)) {
                Ok(v) => {
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
    use super::mcserver_new;

    #[test]
    fn mcsv_stdio_must_piped() {
        let mcsv = mcserver_new("dummy", "./", "").unwrap();

        assert!(mcsv.stdout.is_some(), "stdout is not piped");
        assert!(mcsv.stderr.is_some(), "stderr is not piped");
        assert!(mcsv.stdin.is_some(), "stdin is not piped");
    }
}
