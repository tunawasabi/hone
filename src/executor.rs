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

mod util {
    pub fn port_rule_in_name(port: u16) -> String {
        format!("name=mcsv-handler-discord in {}", port)
    }

    pub fn port_rule_out_name(port: u16) -> String {
        format!("name=mcsv-handler-discord out {}", port)
    }
}

pub fn open_port(port: u16) {
    println!("ポートの開放");

    let localport_arg = format!("localport={}", port);

    Command::new("cmd")
        .args(["/C", "netsh"])
        .arg("advfirewall")
        .arg("firewall")
        .args(["add", "rule"])
        .arg(util::port_rule_in_name(port))
        .arg("dir=in")
        .arg("action=allow")
        .arg("protocol=TCP")
        .arg(localport_arg.clone())
        .status()
        .ok();

    Command::new("cmd")
        .args(["/C", "netsh"])
        .arg("advfirewall")
        .arg("firewall")
        .args(["add", "rule"])
        .arg(util::port_rule_out_name(port))
        .arg("dir=out")
        .arg("action=allow")
        .arg("protocol=TCP")
        .arg(localport_arg)
        .status()
        .ok();
}

pub fn close_port(port: u16) {
    println!("ポートの戸締り");

    Command::new("cmd")
        .args(["/C", "netsh"])
        .arg("advfirewall")
        .arg("firewall")
        .args(["delete", "rule"])
        .arg(util::port_rule_in_name(port))
        .status()
        .ok();

    Command::new("cmd")
        .args(["/C", "netsh"])
        .arg("advfirewall")
        .arg("firewall")
        .args(["delete", "rule"])
        .arg(util::port_rule_out_name(port))
        .status()
        .ok();
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
