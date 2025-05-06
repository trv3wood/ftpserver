use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::message::{FtpMessage, FtpReplyCode};

pub struct Session {
    socket: TcpStream,
}

impl Session {
    pub fn new(socket: TcpStream) -> Self {
        Self { socket }
    }
    pub async fn process(&mut self) -> std::io::Result<()> {
        let mut buf = vec![0; 128];
        while let Ok(n) = self.socket.read(&mut buf).await {
            if n == 0 {
                break;
            }
            let s = String::from_utf8(buf[..n].to_ascii_uppercase()).unwrap();
            let s = s.trim_end();
            let (cmdtype, args) = match s.split_once(' ') {
                Some(cmd) => cmd,
                None => (s, ""),
            };
            dbg!(cmdtype, args);
            match cmdtype {
                "USER" => self.user(args).await,
                "PASS" => self.pass(args).await,
                "ACCT" => self.acct(args).await,
                _ => {
                    Session::send_response(
                        self.socket_mut(),
                        FtpReplyCode::CommandNotImplemented,
                        "CommandNotImplemented",
                    )
                    .await?
                }
            }
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
    async fn user(&mut self, _s: &str) {
        if let Err(e) = Session::send_response(
            self.socket_mut(),
            FtpReplyCode::UserNameOk,
            "user name ok. need password.",
        )
        .await
        {
            eprintln!("{e}");
        }
    }
    async fn pass(&mut self, _s: &str) {
        if let Err(e) =
            Session::send_response(self.socket_mut(), FtpReplyCode::UserLoggedIn, "logged in.")
                .await
        {
            eprintln!("{e}");
        }
    }
    pub async fn acct(&mut self, _s: &str) {
        if let Err(e) =
            Session::send_response(self.socket_mut(), FtpReplyCode::UserLoggedIn, "logged in.")
                .await
        {
            eprintln!("{e}");
        }
    }
    pub async fn cwd(&mut self, s: &str) {

    }
}
