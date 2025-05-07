use std::path::PathBuf;

use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{broadcast, mpsc},
};

use crate::message::{FtpMessage, FtpReplyCode};

pub struct Session {
    socket: TcpStream,
    logged: bool,
    root: PathBuf,
    working_dir: PathBuf,
}
macro_rules! logged {
    ($session:ident) => {
        if !$session.logged {
            Session::send_response(
                $session.socket_mut(),
                FtpReplyCode::NotLoggedIn,
                "Not logged in",
            )
            .await?;
            return Ok(());
        }
    };
}

impl Session {
    pub fn new(socket: TcpStream) -> Self {
        Self {
            socket,
            logged: false,
            #[cfg(target_os = "linux")]
            root: PathBuf::from("/var/ftp"),
            #[cfg(target_os = "linux")]
            working_dir: PathBuf::from("/var/ftp"),
            #[cfg(target_os = "windows")]
            root: PathBuf::from("C:\\ftp"),
            #[cfg(target_os = "windows")]
            working_dir: PathBuf::from("C:\\ftp"),
        }
    }
    pub async fn run(
        &mut self,
        mut shutdown: broadcast::Receiver<()>,
        _close_complete: mpsc::Sender<()>,
    ) -> std::io::Result<()> {
        Session::send_response(
            self.socket_mut(),
            FtpReplyCode::ServiceReadyForNewUser,
            "Service ready for new user",
        )
        .await?;
        tokio::select! {
            res = self.process() => {
                if let Err(e) = res {
                    log::error!("Error processing command: {}", e);
                }
            }
            _ = shutdown.recv() => {
                log::info!("Received shutdown signal, closing session.");
            }
        }
        Ok(())
    }
    async fn process(&mut self) -> std::io::Result<()> {
        let mut buf = vec![0; 128];
        while let Ok(n) = self.socket.read(&mut buf).await {
            if n == 0 {
                break;
            }
            let s = String::from_utf8(buf[..n].to_vec()).unwrap();
            let s = s.trim_end();
            let (cmdtype, args) = match s.split_once(' ') {
                Some(cmd) => cmd,
                None => (s, ""),
            };
            log::debug!("cmdtype {} args {}", cmdtype, args);
            match cmdtype.to_uppercase().as_str() {
                "USER" => self.user(args).await,
                "PASS" => self.pass(args).await,
                "ACCT" => self.acct(args).await,
                "CWD" => self.cwd(args).await,
                _ => {
                    Session::send_response(
                        self.socket_mut(),
                        FtpReplyCode::CommandNotImplemented,
                        "CommandNotImplemented",
                    )
                    .await
                }
            }?
        }
        println!("Close Connection from {:?}", self.socket.peer_addr());
        Ok(())
    }

    fn socket_mut(&mut self) -> &mut TcpStream {
        &mut self.socket
    }

    async fn send_response(
        socket: &mut TcpStream,
        code: FtpReplyCode,
        msg: &str,
    ) -> io::Result<()> {
        socket.write_all(&FtpMessage::new(code, msg).to_vec()).await
    }
    async fn user(&mut self, _s: &str) -> std::io::Result<()> {
        log::debug!("user: {}", _s);
        Session::send_response(
            self.socket_mut(),
            FtpReplyCode::UserNameOk,
            "user name ok. need password.",
        )
        .await
    }
    async fn pass(&mut self, _s: &str) -> std::io::Result<()> {
        self.logged = true;
        Session::send_response(self.socket_mut(), FtpReplyCode::UserLoggedIn, "logged in.").await
    }
    async fn acct(&mut self, _s: &str) -> std::io::Result<()> {
        Session::send_response(
            self.socket_mut(),
            FtpReplyCode::SyntaxErrorUnrecognizedCommand,
            "Unsupported command",
        )
        .await
    }
    async fn cwd(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        if s.is_empty() {
            Session::send_response(
                self.socket_mut(),
                FtpReplyCode::ActionNotTaken,
                "No path given",
            )
            .await?
        }
        let new_working_dir = std::path::Path::new(&s);
        if !new_working_dir.is_dir() {
            Session::send_response(
                self.socket_mut(),
                FtpReplyCode::ActionNotTaken,
                "The given resource is not a directory.",
            )
            .await?
        }
        let new_working_dir = self.root.join(new_working_dir);
        match new_working_dir.canonicalize() {
            Ok(path) => {
                if path.starts_with(&self.root) {
                    Session::send_response(
                        self.socket_mut(),
                        FtpReplyCode::FileActionCompleted,
                        &format!("Working directory changed to {}", path.display()),
                    )
                    .await?;
                    self.working_dir = path;
                    log::debug!("new working dir: {:?}", new_working_dir);
                } else {
                    Session::send_response(
                        self.socket_mut(),
                        FtpReplyCode::ActionNotTaken,
                        "Path not in root",
                    )
                    .await?
                }
            }
            Err(e) => {
                log::debug!("Error canonicalizing path: {}", e);
                Session::send_response(self.socket_mut(), FtpReplyCode::ActionNotTaken, "Failed ot change directory: The given resource does not exist or permission denied.").await?
            }
        }
        Ok(())
    }
}
