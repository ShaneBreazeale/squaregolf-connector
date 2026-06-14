use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub struct JsonTcpClient {
    stream: TcpStream,
}

impl JsonTcpClient {
    pub async fn connect(host: &str, port: u16) -> Result<Self, String> {
        let addr = format!("{host}:{port}");
        let stream = TcpStream::connect(&addr)
            .await
            .map_err(|err| format!("connect {addr}: {err}"))?;
        Ok(Self { stream })
    }

    pub async fn send_json<T: Serialize>(&mut self, payload: &T) -> Result<(), String> {
        let mut data =
            serde_json::to_vec(payload).map_err(|err| format!("serialize payload: {err}"))?;
        data.push(b'\n');
        self.stream
            .write_all(&data)
            .await
            .map_err(|err| format!("send payload: {err}"))
    }
}
