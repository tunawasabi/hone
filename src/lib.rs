pub mod types {
    use serde_derive::Deserialize;

    #[derive(Deserialize)]
    pub struct Config {
        pub client: Client,
        pub permission: Permission,
        pub server: Server,
    }

    #[derive(Deserialize)]
    pub struct Client {
        pub secret: String,
    }

    #[derive(Deserialize)]
    pub struct Permission {
        pub channel_id: u64,
        pub user_id: u64,
    }

    #[derive(Deserialize)]
    pub struct Server {
        pub work_dir: String,
        pub jar_file: String,
        pub memory: String,
    }
}

pub mod excecutor {
    use std::io;
    use std::process::{Child, Command, Stdio};

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
            .stderr(Stdio::piped())
            .spawn()
    }
}
