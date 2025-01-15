#[cfg(target_os = "windows")]
pub use self::windows::*;

#[cfg(not(target_os = "windows"))]
pub use self::not_windows::*;

#[cfg(target_os = "windows")]
mod windows {
    use std::process::Command;

    pub fn command_new() -> Command {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C");
        cmd
    }

    fn firewall_process_new() -> Command {
        let mut cmd = command_new();
        cmd.arg("netsh").arg("advfirewall").arg("firewall");
        cmd
    }

    fn rule_name_arg(port: u16) -> String {
        format!("name=mcsv-handler-discord-{}", port)
    }

    pub fn open_port(port: u16) {
        println!("ポートの開放");

        let add_port_rule = |port: u16| {
            let mut cmd = firewall_process_new();
            cmd.args(["add", "rule"])
                .arg(rule_name_arg(port))
                .arg("action=allow")
                .arg("protocol=TCP")
                .arg(format!("localport={}", port));
            cmd
        };

        add_port_rule(port).arg("dir=in").status().ok();
        add_port_rule(port).arg("dir=out").status().ok();
    }

    pub fn close_port(port: u16) {
        println!("ポートの戸締り");

        let delete_rule = || {
            let mut cmd = firewall_process_new();
            cmd.args(["delete", "rule"]).arg(rule_name_arg(port));
            cmd
        };

        delete_rule().status().ok();
    }
}

#[cfg(not(target_os = "windows"))]
mod not_windows {
    use std::process::Command;

    pub fn command_new() -> Command {
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        cmd
    }
}
