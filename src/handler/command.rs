use super::Handler;

pub fn parse_command(message: &str) -> Option<Vec<&str>> {
    if message.len() <= 1 || !message.starts_with("!") {
        return None;
    }

    let message = message[1..].split(' ');
    let args: Vec<&str> = message.collect();
    Some(args)
}

pub async fn mcstart(handler: &Handler) {
    unimplemented!()
}

/// Discordで送信されたコマンドをMinecraftサーバに送信します。
pub async fn send_command_to_server(handler: &Handler, args: Vec<&str>) {
    if args.len() == 0 {
        handler.send_message("引数を入力して下さい！").await;
        return;
    }

    let mut stdin = handler.thread_stdin.lock().await;
    if stdin.is_some() {
        stdin.as_mut().unwrap().send(args.join(" ")).unwrap();

        handler.send_message("コマンドを送信しました").await;

        let mut inputed = handler.command_inputed.lock().await;
        *inputed = true;
    } else {
        handler.send_message("起動していません！").await;
    }
}

pub async fn send_stop_to_server(handler: &Handler) {
    let mut stdin = handler.thread_stdin.lock().await;
    let mut inputed = handler.command_inputed.lock().await;

    if stdin.is_some() {
        stdin.as_mut().unwrap().send("stop".to_string()).unwrap();

        println!("stopping...");
        handler.send_message("終了しています……").await;

        *stdin = None;
        *inputed = false;
    } else {
        handler.send_message("起動していません！").await;
    }
}

pub async fn mcsvend(handler: &Handler) {
    handler.send_message("クライアントを終了しました。").await;
    std::process::exit(0);
}

#[cfg(test)]
mod test {
    use crate::handler::command::parse_command;

    #[test]
    fn parse_command_correctly() {
        let message = String::from("!a b c d e");
        let args = parse_command(&message).unwrap();

        assert_eq!(args, vec!["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn parse_command_failed_correctly() {
        // コマンドではないメッセーが送信された時
        assert!(parse_command("hello").is_none());

        // prefixが使用されているが1文字の時
        assert!(parse_command("!").is_none());
    }
}
