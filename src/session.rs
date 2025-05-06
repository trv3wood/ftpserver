use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub struct Session {
    socket: TcpStream,
}

impl Session {
    pub fn new(socket: TcpStream) -> Self {
        Self { socket }
    }
    // echo
    pub async fn process(&mut self) -> std::io::Result<()> {
        let mut buf = vec![0; 128];
        let (mut rd, mut wr) = self.socket.split();
        let bytes = rd.read(&mut buf).await?;
        if bytes == 0 {
            return Ok(());
        }
        wr.write(&buf[..bytes]).await?;
        Ok(())
    }
}
