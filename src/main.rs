use std::{env::set_current_dir, error::Error};

use tokio::net::TcpListener;

mod message;
mod server;
mod session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Some(specified_dir) = std::env::args().nth(1) {
        set_current_dir(specified_dir)?;
    } else {
        #[cfg(target_os = "linux")]
        set_current_dir("/var/ftp")?; // 设置当前目录为/var/ftp
        #[cfg(target_os = "windows")]
        set_current_dir("C:\\ftp")?; // 设置当前目录为C:\ftp
    }
    #[cfg(debug_assertions)]
    let env = env_logger::Env::default().filter_or("RUST_LOG", "debug");

    #[cfg(not(debug_assertions))]
    let env = env_logger::Env::default().filter_or("RUST_LOG", "info");

    env_logger::init_from_env(env);
    let listener = TcpListener::bind("0.0.0.0:2121").await?;
    log::info!("Listening on {}", listener.local_addr()?);
    let mut server = server::Server::new(listener);
    server.run().await
}
