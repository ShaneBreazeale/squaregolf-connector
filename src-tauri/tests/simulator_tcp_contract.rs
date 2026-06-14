use squaregolf_connector::simulator::client::JsonTcpClient;
use squaregolf_connector::simulator::open_connect::ready_payload;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;

#[tokio::test]
async fn json_tcp_client_sends_newline_delimited_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut lines = BufReader::new(stream).lines();
        lines.next_line().await.unwrap().unwrap()
    });

    let mut client = JsonTcpClient::connect("127.0.0.1", addr.port())
        .await
        .expect("client connects");
    client
        .send_json(&ready_payload(true, 11))
        .await
        .expect("send payload");

    let line = server.await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(json["ShotNumber"], 11);
    assert_eq!(json["ShotDataOptions"]["LaunchMonitorIsReady"], true);
}
