#[tokio::main]
async fn main() {
    let splash_text = "🦴 Hone **********";

    println!("{}", splash_text);
    hone::start().await;
}
