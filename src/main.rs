use std::error::Error;

use session::Session;
use tokio::net::TcpListener;

mod message;
mod session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("localhost:2121").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move { Session::new(socket).process().await });
    }
}
