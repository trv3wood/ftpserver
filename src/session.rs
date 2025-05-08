use std::{
    env::set_current_dir,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc},
};

use crate::message::{FtpMessage, FtpReplyCode};

pub struct Session {
    socket: TcpStream,
    logged: bool,
    root: PathBuf,
    working_dir: PathBuf,
    data_listener: Option<TcpListener>,
}
macro_rules! logged {
    ($session:ident) => {
        if !$session.logged {
            $session
                .send_response(FtpReplyCode::NotLoggedIn, "Not logged in")
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
            root: std::env::current_dir().unwrap(),
            working_dir: std::env::current_dir().unwrap(),
            data_listener: None,
        }
    }
    pub async fn run(
        &mut self,
        mut shutdown: broadcast::Receiver<()>,
        _close_complete: mpsc::Sender<()>,
    ) -> std::io::Result<()> {
        self.send_response(
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
            log::debug!("received command: {}", s);
            let (cmdtype, args) = match s.split_once(' ') {
                Some(cmd) => cmd,
                None => (s, ""),
            };
            match cmdtype.to_uppercase().as_str() {
                "USER" => self.user(args).await,
                "PASS" => self.pass(args).await,
                "ACCT" => self.acct(args).await,
                "CWD" => self.cwd(args).await,
                "PWD" => self.pwd(args).await,
                "LIST" => self.list(args).await,
                "PASV" => self.pasv(args).await,
                "RETR" => self.retr(args).await,
                "TYPE" => self.r#type(args).await,
                "STOR" => self.stor(args).await,
                "NOOP" => self.send_response(FtpReplyCode::CommandOk, "NOOP").await,
                "QUIT" => {
                    self.send_response(
                        FtpReplyCode::ServiceClosingControlConnection,
                        "Connection shutting down",
                    )
                    .await?;
                    break;
                }
                _ => {
                    self.send_response(FtpReplyCode::CommandNotImplemented, "CommandNotImplemented")
                        .await
                }
            }?
        }
        log::info!("Close Connection from {}", self.socket.peer_addr()?);
        Ok(())
    }

    pub async fn send_response(&mut self, code: FtpReplyCode, msg: &str) -> io::Result<()> {
        self.socket
            .write_all(&FtpMessage::new(code, msg).to_vec())
            .await
    }
    async fn user(&mut self, _s: &str) -> std::io::Result<()> {
        log::debug!("user: {}", _s);
        self.send_response(FtpReplyCode::UserNameOk, "user name ok. need password.")
            .await
    }
    async fn pass(&mut self, _s: &str) -> std::io::Result<()> {
        self.logged = true;
        self.send_response(FtpReplyCode::UserLoggedIn, "logged in.")
            .await
    }
    async fn acct(&mut self, _s: &str) -> std::io::Result<()> {
        self.send_response(
            FtpReplyCode::SyntaxErrorUnrecognizedCommand,
            "Unsupported command",
        )
        .await
    }
    async fn cwd(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        if s.is_empty() {
            self.send_response(FtpReplyCode::ActionNotTaken, "No path given")
                .await?;
            return Ok(());
        }
        let given_path = std::path::Path::new(&s);
        if !given_path.is_dir() {
            self.send_response(
                FtpReplyCode::ActionNotTaken,
                "The given resource is not a directory.",
            )
            .await?;
            return Ok(());
        }
        self.exec_cwd(given_path).await
    }

    async fn exec_cwd(&mut self, path: &Path) -> std::io::Result<()> {
        match path.canonicalize() {
            Ok(path) => {
                // 处理路径前缀
                #[cfg(target_os = "windows")]
                let path = path.to_str().unwrap().trim_start_matches(r"\\?\");
                #[cfg(target_os = "windows")]
                let path = PathBuf::from(path);

                log::debug!("canonicalize path: {:?}", &path);
                if path.starts_with(&self.root) {
                    self.send_response(
                        FtpReplyCode::FileActionCompleted,
                        &format!("Working directory changed to {}", path.display()),
                    )
                    .await?;
                    log::debug!("new working dir: {:?}", &path);
                    self.working_dir = path;
                    set_current_dir(&self.working_dir)?;
                    Ok(())
                } else {
                    self.send_response(FtpReplyCode::ActionNotTaken, "Path not in root")
                        .await
                }
            }
            Err(e) => {
                log::debug!("Error canonicalizing path: {}", e);
                self.send_response(FtpReplyCode::ActionNotTaken, "Failed ot change directory: The given resource does not exist or permission denied.").await
            }
        }
    }

    async fn pwd(&mut self, _s: &str) -> std::io::Result<()> {
        if !self.logged {
            self.send_response(FtpReplyCode::ActionNotTaken, "Not logged in")
                .await
        } else {
            self.send_response(
                FtpReplyCode::PathnameCreated,
                &format!(
                    "~/{}",
                    self.working_dir
                        .strip_prefix(self.root.as_path())
                        .unwrap()
                        .display()
                ),
            )
            .await
        }
    }
    async fn pasv(&mut self, _s: &str) -> std::io::Result<()> {
        logged!(self);
        let listener = TcpListener::bind(format!("{}:0", self.socket.local_addr()?.ip())).await?;
        let addr = listener.local_addr()?;
        self.data_listener = Some(listener);

        // 构造PASV响应
        let (ip, port) = (addr.ip(), addr.port());
        let (p1, p2) = (port >> 8, port & 0xFF);

        let ip = ip.to_string();
        let mut ip = ip.split('.');

        let response = format!(
            "Entering Passive Mode ({},{},{},{},{},{})",
            ip.next().unwrap(),
            ip.next().unwrap(),
            ip.next().unwrap(),
            ip.next().unwrap(),
            p1,
            p2
        );
        self.send_response(FtpReplyCode::EnteringPassiveMode, &response)
            .await
    }
    async fn list(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        if let Some(data_listener) = &self.data_listener {
            let (mut data_sock, _) = data_listener.accept().await?;
            let (_, mut writer) = data_sock.split();
            self.send_response(
                FtpReplyCode::FileStatusOkOpeningDataConnection,
                "Sending directory listing",
            )
            .await?;
            let ls = self.exec_list_dir(&self.working_dir.join(s))?;
            writer.write_all(ls.as_bytes()).await?;
            self.data_listener = None;
            return self
                .send_response(FtpReplyCode::ClosingDataConnection, "Transfer complete")
                .await;
        }
        Err(ErrorKind::HostUnreachable.into())
    }

    fn exec_list_dir(&self, path: &Path) -> std::io::Result<String> {
        let mut entries = String::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let mut file_name = entry.file_name().into_string().unwrap_or_default();
            file_name.push('\n');
            entries.push_str(&file_name);
        }
        Ok(entries)
    }
    async fn retr(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        if let Some(data_listener) = &self.data_listener {
            let (mut data_sock, _) = data_listener.accept().await?;
            let (_, mut writer) = data_sock.split();
            self.send_response(
                FtpReplyCode::FileStatusOkOpeningDataConnection,
                "Sending file",
            )
            .await?;
            let file_path = self.working_dir.join(s);
            if file_path.exists() {
                let mut file = tokio::fs::File::open(file_path).await?;
                let mut buf = vec![0; 1024];
                loop {
                    let n = file.read(&mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    writer.write_all(&buf[..n]).await?;
                }
                self.data_listener = None;
                return self
                    .send_response(FtpReplyCode::ClosingDataConnection, "Transfer complete")
                    .await;
            } else {
                return self
                    .send_response(FtpReplyCode::FileActionNotTaken, "File not found")
                    .await;
            }
        }
        self.send_response(
            FtpReplyCode::ErrorOpeningDataConnection,
            "Failed to open data connection",
        )
        .await
    }
    async fn r#type(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        match s.to_uppercase().as_str() {
            "A" => {
                self.send_response(FtpReplyCode::CommandOk, "Switching to ASCII mode")
                    .await
            }
            "I" => {
                self.send_response(FtpReplyCode::CommandOk, "Switching to Binary mode")
                    .await
            }
            _ => {
                self.send_response(FtpReplyCode::ActionNotTaken, "Unsupported type")
                    .await
            }
        }
    }

    async fn stor(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        if let Some(data_listener) = &self.data_listener {
            let (mut data_sock, _) = data_listener.accept().await?;
            let (mut reader, _) = data_sock.split();
            self.send_response(
                FtpReplyCode::FileStatusOkOpeningDataConnection,
                "Receiving file",
            )
            .await?;
            let file_path = self.working_dir.join(s);
            let mut file = tokio::fs::File::create(file_path).await?;
            let mut buf = vec![0; 1024];
            loop {
                let n = reader.read(&mut buf).await?;
                if n == 0 {
                    break;
                }
                file.write_all(&buf[..n]).await?;
            }
            self.data_listener = None;
            return self
                .send_response(FtpReplyCode::ClosingDataConnection, "Transfer complete")
                .await;
        }
        self.send_response(
            FtpReplyCode::ErrorOpeningDataConnection,
            "Failed to open data connection",
        )
        .await
    }
}
