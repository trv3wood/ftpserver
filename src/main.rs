use std::error::Error;

use session::Session;
use tokio::net::TcpListener;

mod message;
mod session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default()
            .default_filter_or("debug")
    );
    let listener = TcpListener::bind("localhost:2121").await?;
    log::info!("Listening on {}", listener.local_addr()?);

    loop {
        let (socket, addr) = listener.accept().await?;
        log::info!("Accepted connection from {}", addr);
        tokio::spawn(async move { Session::new(socket).process().await });
    }
}
