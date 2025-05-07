use std::error::Error;

use session::Session;
use tokio::{net::TcpListener, sync::mpsc};

mod message;
mod session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));
    let listener = TcpListener::bind("0.0.0.0:2121").await?;
    log::info!("Listening on {}", listener.local_addr()?);
    let (send, mut recv) = mpsc::channel(1);
    let (shutdown_send, _) = tokio::sync::broadcast::channel(1);
    tokio::select! {
        // 主循环接受连接
        _ = async {
            loop {
                let (socket, _) = listener.accept().await?;
                log::info!("Accepted connection from {}", socket.peer_addr()?);

                let mut session = Session::new(socket);
                let shutdown_notify = shutdown_send.subscribe();
                let send = send.clone();

                tokio::spawn(async move {
                    if let Err(e) = session.run(shutdown_notify, send).await {
                        log::error!("Session error: {}", e);
                    }
                });
            }
            #[allow(unreachable_code)]
            Ok::<_, Box<dyn std::error::Error>>(())
        } => {}

        // 响应Ctrl-C信号
        _ = tokio::signal::ctrl_c() => {
            log::info!("Received shutdown signal, shutting down.");
        }
    }
    drop(shutdown_send);
    drop(send);
    let _ = recv.recv().await; // 等待所有会话完成
    Ok(())
}
