use tokio::{
    net::TcpListener,
    sync::{broadcast, mpsc},
};

use crate::{message::FtpReplyCode, session::Session};

pub struct Server {
    ctrl_socket: TcpListener,
}

impl Server {
    pub fn new(listener: TcpListener) -> Self {
        Self {
            ctrl_socket: listener,
        }
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (send, mut recv) = mpsc::channel(1);
        let (shutdown_send, _) = broadcast::channel(1);

        tokio::select! {
            // 主循环接受连接
            _ = async {
                loop {
                    let (socket, addr) = self.ctrl_socket.accept().await?;
                    log::info!("Accepted connection from {}", addr);

                    let mut session = Session::new(socket);
                    let shutdown_notify = shutdown_send.subscribe();
                    let send = send.clone();

                    tokio::spawn(async move {
                        if let Err(e) = session.run(shutdown_notify, send).await {
                            log::error!("Session error: {}", e);
                            let _ = session.send_response(FtpReplyCode::ActionAbortedLocalError, "Connection aborted").await;
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
}
