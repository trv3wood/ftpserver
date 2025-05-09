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
        set_current_dir(&self.root)
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
                "LIST" | "NLST" => self.nlst(args).await,
                "PASV" => self.pasv(args).await,
                "RETR" => self.retr(args).await,
                "TYPE" => self.r#type(args).await,
                "STOR" => self.stor(args).await,
                "STRU" => self.stru(args).await,
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
        match dbg!(path.canonicalize()) {
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
                    self.set_pwd(path)
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
                    "/{}",
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
    async fn nlst(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);

        let path = self.working_dir.join(s);

        self.with_data_connection(|mut datasock| async move {
            // 获取目录列表
            let entries = Session::exec_list_dir(&path)?;
            // 写入数据
            datasock.write_all(entries.as_bytes()).await
        })
        .await
    }

    fn exec_list_dir(path: &Path) -> std::io::Result<String> {
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
        let file_path = self.working_dir.join(s);
        self.with_data_connection(|mut datasock| async move {
            if file_path.exists() {
                let mut file = tokio::fs::File::open(file_path).await?;
                io::copy(&mut file, &mut datasock).await?;
                Ok(())
            } else {
                Err(ErrorKind::NotFound.into())
            }
        })
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
        let file_path = self.working_dir.join(s);
        self.with_data_connection(|mut datasock| async move {
            let mut file = tokio::fs::File::create(file_path).await?;
            io::copy(&mut datasock, &mut file).await?;
            Ok(())
        })
        .await
    }

    fn set_pwd(&mut self, path: PathBuf) -> std::io::Result<()> {
        set_current_dir(&path)?;
        self.working_dir = path;
        Ok(())
    }

    async fn stru(&mut self, args: &str) -> std::io::Result<()> {
        match args {
            "F" => {
                self.send_response(FtpReplyCode::CommandOk, "Structure set to File.")
                    .await
            }
            "R" | "P" => {
                self.send_response(
                    FtpReplyCode::CommandNotImplementedForParameter,
                    "not supported",
                )
                .await
            }
            _ => {
                self.send_response(FtpReplyCode::SyntaxErrorParameters, "SyntaxErrorParameters")
                    .await
            }
        }
    }
    async fn with_data_connection<F, Fut>(&mut self, operation: F) -> std::io::Result<()>
    where
        F: FnOnce(TcpStream) -> Fut,
        Fut: Future<Output = std::io::Result<()>>,
    {
        // 取出数据监听器所有权
        let listener = match self.data_listener.take() {
            Some(l) => l,
            None => {
                self.send_response(
                    FtpReplyCode::ErrorOpeningDataConnection,
                    "Failed to open data connection",
                )
                .await?;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "No data listener",
                ));
            }
        };

        // 建立数据连接
        let (data_socket, _) = listener.accept().await?;

        // 发送准备就绪响应
        self.send_response(
            FtpReplyCode::FileStatusOkOpeningDataConnection,
            "Sending data",
        )
        .await?;

        // 执行实际操作
        let result = operation(data_socket).await;

        // 无论成功与否，发送完成响应
        self.send_response(FtpReplyCode::ClosingDataConnection, "Transfer complete")
            .await?;

        result
    }
}
