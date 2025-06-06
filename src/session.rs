use std::path::Path;

use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    process::Command,
    sync::{broadcast, mpsc},
};

use crate::{message::*, mydbg, path::PathHandler};

pub struct Session {
    socket: TcpStream,
    logged: bool,
    // root: PathBuf,
    // working_dir: PathBuf,
    path_handler: PathHandler,
    data_listener: Option<TcpListener>,
    data_port: Option<TcpStream>,
    rename_from_path: Option<std::path::PathBuf>,
}
macro_rules! logged {
    ($session:ident) => {
        if !$session.logged {
            $session
                .send_response(crate::message::NOT_LOGGED_IN, "Not logged in")
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
            path_handler: PathHandler::new(std::env::current_dir().unwrap()),
            data_listener: None,
            data_port: None,
            rename_from_path: None,
        }
    }
    pub async fn run(
        &mut self,
        mut shutdown: broadcast::Receiver<()>,
        _close_complete: mpsc::Sender<()>,
    ) -> std::io::Result<()> {
        self.send_response(SERVICE_READY_FOR_NEW_USER, "Service ready for new user")
            .await?;
        tokio::select! {
            res = self.process() => {
                if let Err(e) = res {
                    let _ = self.send_response(ACTION_ABORTED_LOCAL_ERROR, "Connection aborted").await;
                    log::error!("Session error: {}", e);
                }
            }
            _ = shutdown.recv() => {
                log::info!("Received shutdown signal, closing session.");
            }
        }
        Ok(())
    }
    async fn process(&mut self) -> std::io::Result<()> {
        let mut buf = vec![0; 512];
        while let Ok(n) = self.socket.read(&mut buf).await {
            if n == 0 {
                break;
            }
            let s = String::from_utf8_lossy(&buf[..n]);
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
                "PWD" | "XPWD" => self.pwd(args).await,
                "NLST" => self.nlst(args).await,
                "LIST" => self.list(args).await,
                "PASV" => self.pasv(args).await,
                "RETR" => self.retr(args).await,
                "TYPE" => self.r#type(args).await,
                "STOR" => self.stor(args).await,
                "STRU" => self.stru(args).await,
                "DELE" => self.dele(args).await,
                "RMD" => self.rmd(args).await,
                "MKD" => self.mkd(args).await,
                "RNFR" => self.rnfr(args).await,
                "RNTO" => self.rnto(args).await,
                "OPTS" => self.opts(args).await,
                "PORT" => self.port(args).await,
                "NOOP" => self.send_response(COMMAND_OK, "NOOP").await,
                "QUIT" => {
                    self.send_response(
                        SERVICE_CLOSING_CONTROL_CONNECTION,
                        "Connection shutting down",
                    )
                    .await?;
                    break;
                }
                _ => {
                    self.send_response(COMMAND_NOT_IMPLEMENTED, "CommandNotImplemented")
                        .await
                }
            }?
        }
        log::info!("Close Connection from {}", self.socket.peer_addr()?);
        Ok(())
    }

    pub async fn send_response(
        &mut self,
        code: impl AsRef<str>,
        msg: impl AsRef<str>,
    ) -> io::Result<()> {
        let response = format!("{} {}\r\n", code.as_ref(), msg.as_ref());
        log::debug!("Sending response: {}", response);
        self.socket.write_all(response.as_bytes()).await?;
        self.socket.flush().await?;
        Ok(())
    }
    async fn user(&mut self, _s: &str) -> std::io::Result<()> {
        log::debug!("user: {}", _s);
        self.send_response(USER_NAME_OK, "user name ok. need password.")
            .await
    }
    async fn pass(&mut self, _s: &str) -> std::io::Result<()> {
        self.logged = true;
        self.send_response(USER_LOGGED_IN, "logged in.").await
    }
    async fn acct(&mut self, _s: &str) -> std::io::Result<()> {
        self.send_response(SYNTAX_ERROR_UNRECOGNIZED_COMMAND, "Unsupported command")
            .await
    }
    async fn cwd(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        if s.is_empty() {
            self.send_response(ACTION_NOT_TAKEN, "No path given")
                .await?;
            return Ok(());
        }
        match self.path_handler.cd(s) {
            Ok(_) => {
                let pwd = self.path_handler.get_pwd();
                self.send_response(
                    FILE_ACTION_COMPLETED,
                    format!("Changed directory to {}", pwd.display()),
                )
                .await
            }
            Err(e) => {
                log::debug!("Error changing directory: {}", e);
                self.send_response(ACTION_NOT_TAKEN, e.to_string()).await
            }
        }
    }

    async fn pwd(&mut self, _s: &str) -> std::io::Result<()> {
        let pwd = self.path_handler.get_pwd();
        self.send_response(PATHNAME_CREATED, pwd.to_string_lossy())
            .await
    }
    async fn pasv(&mut self, _s: &str) -> std::io::Result<()> {
        logged!(self);
        let listener = TcpListener::bind(format!("{}:0", self.socket.local_addr()?.ip())).await?;
        let addr = listener.local_addr()?;
        self.data_listener = Some(listener);
        self.data_port = None;

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
        self.send_response(ENTERING_PASSIVE_MODE, response).await
    }
    async fn nlst(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);

        let path = self.path_handler.to_server_path(s)?;

        self.with_data_connection(|mut datasock| async move {
            // 获取目录列表
            let entries = Session::exec_list_dir_name(&path)?;
            // 写入数据
            datasock.write_all(entries.as_bytes()).await
        })
        .await
    }

    fn exec_list_dir_name(path: &Path) -> std::io::Result<String> {
        let mut entries = String::new();
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    entries.push_str(&format!("{}/\n", entry.file_name().to_string_lossy()));
                } else if file_type.is_file() {
                    entries.push_str(&format!("{}\n", entry.file_name().to_string_lossy()));
                } else {
                    entries.push_str(&format!(
                        "{} (symlink)\n",
                        entry.file_name().to_string_lossy()
                    ));
                }
            } else {
                entries.push_str(&format!(
                    "{} (error reading type)\n",
                    entry.file_name().to_string_lossy()
                ));
            }
        }
        Ok(if entries.is_empty() {
            "No files found.\n".to_string()
        } else {
            entries
        })
    }

    async fn list(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        let path = self.path_handler.to_server_path(s)?;
        self.with_data_connection(|mut datasock| async move {
            #[cfg(not(target_os = "windows"))]
            let dirlist = Command::new("ls")
                .arg("-all")
                .arg(path)
                .output()
                .await?
                .stdout;
            #[cfg(target_os = "windows")]
            let dirlist = Command::new("cmd")
                .arg("/C")
                .arg(format!("dir {}", path.display()))
                .output()
                .await?
                .stdout;
            datasock.write_all(&dirlist).await
        })
        .await
    }

    async fn retr(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        let file_path = self.path_handler.to_server_path(s)?;
        log::debug!("Retrieving file: {:?}", &file_path);
        if !file_path.is_file() {
            return self.send_response(ACTION_NOT_TAKEN, "Not a file").await;
        }
        if !file_path.exists() {
            return self.send_response(ACTION_NOT_TAKEN, "File not found").await;
        }
        self.with_data_connection(|mut datasock| async move {
            let mut file = tokio::fs::File::open(file_path).await?;
            io::copy(&mut file, &mut datasock).await?;
            Ok(())
        })
        .await
    }
    async fn r#type(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        match s.to_uppercase().as_str() {
            "A" => {
                self.send_response(COMMAND_OK, "Switching to ASCII mode")
                    .await
            }
            "I" => {
                self.send_response(COMMAND_OK, "Switching to Binary mode")
                    .await
            }
            _ => {
                self.send_response(ACTION_NOT_TAKEN, "Unsupported type")
                    .await
            }
        }
    }

    async fn stor(&mut self, s: &str) -> std::io::Result<()> {
        logged!(self);
        let file_path = self.path_handler.non_canonicalized_path(s)?;
        self.with_data_connection(|mut datasock| async move {
            let mut file = tokio::fs::File::create(file_path).await?;
            io::copy(&mut datasock, &mut file).await?;
            Ok(())
        })
        .await
    }

    async fn stru(&mut self, args: &str) -> std::io::Result<()> {
        match args {
            "F" => {
                self.send_response(COMMAND_OK, "Structure set to File.")
                    .await
            }
            "R" | "P" => {
                self.send_response(COMMAND_NOT_IMPLEMENTED_FOR_PARAMETER, "not supported")
                    .await
            }
            _ => {
                self.send_response(SYNTAX_ERROR_PARAMETERS, "SyntaxErrorParameters")
                    .await
            }
        }
    }
    async fn get_data_socket(&mut self) -> std::io::Result<TcpStream> {
        if let Some(socket) = self.data_port.take() {
            return Ok(socket);
        }
        if let Some(listener) = self.data_listener.take() {
            let (data_socket, _) = listener.accept().await?;
            return Ok(data_socket);
        }
        self.send_response(
            ERROR_OPENING_DATA_CONNECTION,
            "Failed to open data connection",
        )
        .await?;
        Err(std::io::Error::new(
            std::io::ErrorKind::NotConnected,
            "No data connection available",
        ))
    }
    async fn with_data_connection<F, Fut>(&mut self, operation: F) -> std::io::Result<()>
    where
        F: FnOnce(TcpStream) -> Fut,
        Fut: Future<Output = std::io::Result<()>>,
    {
        // 获取数据连接所有权
        let data_socket = self.get_data_socket().await?;

        // 发送准备就绪响应
        self.send_response(
            FILE_STATUS_OK_OPENING_DATA_CONNECTION,
            "Opening Data Connection",
        )
        .await?;

        // 执行实际操作
        let result = operation(data_socket).await;

        // 无论成功与否，发送完成响应
        self.send_response(CLOSING_DATA_CONNECTION, "Transfer complete")
            .await?;

        result
    }
    async fn dele(&mut self, args: &str) -> std::io::Result<()> {
        logged!(self);
        let args = self.path_handler.to_server_path(args)?;
        if let Err(e) = tokio::fs::remove_file(args).await {
            self.send_response(ACTION_NOT_TAKEN, &e.to_string()).await
        } else {
            self.send_response(FILE_ACTION_COMPLETED, "deleted").await
        }
    }
    async fn rmd(&mut self, args: &str) -> std::io::Result<()> {
        logged!(self);
        let path = self.path_handler.to_server_path(args)?;
        if !path.is_dir() {
            return self
                .send_response(ACTION_NOT_TAKEN, "Not a directory")
                .await;
        }
        if let Err(e) = tokio::fs::remove_dir_all(path).await {
            self.send_response(ACTION_NOT_TAKEN, &e.to_string()).await
        } else {
            self.send_response(FILE_ACTION_COMPLETED, "deleted").await
        }
    }
    async fn mkd(&mut self, args: &str) -> std::io::Result<()> {
        logged!(self);
        let path = match self.path_handler.non_canonicalized_path(args) {
            Ok(path) => path,
            Err(e) => {
                return self.send_response(ACTION_NOT_TAKEN, &e.to_string()).await;
            }
        };
        if let Err(e) = tokio::fs::create_dir(path).await {
            self.send_response(ACTION_NOT_TAKEN, &e.to_string()).await
        } else {
            self.send_response(PATHNAME_CREATED, "directory created")
                .await
        }
    }
    async fn rnfr(&mut self, args: &str) -> std::io::Result<()> {
        logged!(self);
        let args = self.path_handler.to_server_path(args)?;
        if std::fs::exists(&args)? {
            self.rename_from_path = Some(args);
            self.send_response(FILE_ACTION_NEEDS_FURTHER_INFO, "Enter target name")
                .await
        } else {
            self.send_response(ACTION_NOT_TAKEN, "file not exist").await
        }
    }

    async fn rnto(&mut self, args: &str) -> std::io::Result<()> {
        logged!(self);
        let rename_from = match self.rename_from_path.take() {
            Some(path) => path,
            None => {
                return self
                    .send_response(COMMANDS_BAD_SEQUENCE, "Please specify target file first")
                    .await;
            }
        };
        let mut rename_to = self.path_handler.non_canonicalized_path(args)?;
        mydbg!((&rename_from, &rename_to));
        match (rename_from.is_dir(), rename_to.is_dir()) {
            (false, true) => {
                // 文件->路径
                let filename = rename_from.file_name().unwrap();
                rename_to.push(filename);
            }
            _ => {} // 同为文件或路径
        }
        match std::fs::rename(rename_from, rename_to) {
            Ok(()) => self.send_response(FILE_ACTION_COMPLETED, "Ok").await,
            Err(e) => self.send_response(ACTION_NOT_TAKEN, &e.to_string()).await,
        }
    }

    async fn opts(&mut self, args: &str) -> std::io::Result<()> {
        match args {
            "UTF8 ON" => self.send_response(COMMAND_OK, "UTF8 mode enabled").await,
            _ => {
                self.send_response(SYNTAX_ERROR_PARAMETERS, "Unsupported OPTS command")
                    .await
            }
        }
    }
    async fn port(&mut self, args: &str) -> std::io::Result<()> {
        logged!(self);
        let parts: Vec<&str> = args.split(',').collect();
        if parts.len() != 6 {
            return self
                .send_response(SYNTAX_ERROR_PARAMETERS, "Invalid PORT command")
                .await;
        }
        let ip = parts[..4].join(".");
        let p1: u16 = parts[4].parse().unwrap_or(0);
        let p2: u16 = parts[5].parse().unwrap_or(0);
        let port = (p1 << 8) | p2;

        let addr = format!("{}:{}", ip, port);
        self.data_port = Some(TcpStream::connect(addr).await?);
        log::debug!("Connected to data port: {:?}", &self.data_port);

        // 发送准备就绪响应
        self.send_response(FILE_STATUS_OK_OPENING_DATA_CONNECTION, "File Status Ok")
            .await?;

        // 设置数据监听器为None，表示使用PORT模式
        self.data_listener = None;

        // 执行实际操作
        Ok(())
    }
}
