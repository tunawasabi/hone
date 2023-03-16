pub enum ServerMessage {
    Done,
    Exit,
    Info(String),
    Error(String),
}
