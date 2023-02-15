use std::fs;
use std::io;
use std::process::{Child, Command, Stdio};
use toml;

use crate::types::Config;

pub fn mcserver_new(jar_file: &str, work_dir: &str, memory: &str) -> io::Result<Child> {
    Command::new("cmd")
        .current_dir(work_dir)
        .args(["/C", "java"])
        .arg(format!("-Xmx{}", memory))
        .arg(format!("-Xms{}", memory))
        .arg("-jar")
        .arg(jar_file)
        .arg("nogui")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
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
