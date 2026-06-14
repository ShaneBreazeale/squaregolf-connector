use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use squaregolf_connector::api;
use squaregolf_connector::config::AppConfig;
use squaregolf_connector::core::protocol::parser::parse_shot_ball_metrics;
use squaregolf_connector::core::{AppState, ConnectionStatus};
use squaregolf_connector::simulator::runtime::SimulatorRuntime;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::TcpListener;
use tower::ServiceExt;

#[tokio::test]
async fn runtime_connects_and_sends_gspro_ball_payload() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut lines = BufReader::new(stream).lines();
        lines.next_line().await.unwrap().unwrap()
    });
    let cfg = AppConfig {
        gspro_enabled: true,
        gspro_port: addr.port(),
        ..Default::default()
    };
    let state = AppState::new(&cfg);
    let runtime = SimulatorRuntime::new(state.clone());

    runtime.connect_gspro().await.expect("connect gspro");
    let metrics = parse_shot_ball_metrics(&[
        "11", "02", "37", "64", "00", "C8", "00", "2C", "01", "E8", "03", "F4", "01", "D0", "07",
        "B8", "0B",
    ])
    .unwrap();
    runtime
        .send_ball_metrics_to_connected(&metrics)
        .await
        .expect("send metrics");

    let line = server.await.unwrap();
    let json: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(json["ShotNumber"], 1);
    assert_eq!(json["ShotDataOptions"]["ContainsBallData"], true);

    let status = state.status().await;
    assert!(matches!(
        status.gspro.connection_status,
        ConnectionStatus::Connected
    ));
    assert_eq!(status.gspro.last_shot_number, Some(1));
}

#[tokio::test]
async fn runtime_connection_error_updates_status() {
    let cfg = AppConfig {
        gspro_enabled: true,
        gspro_port: 9,
        ..Default::default()
    };
    let state = AppState::new(&cfg);
    let runtime = SimulatorRuntime::new(state.clone());

    let err = runtime
        .connect_gspro()
        .await
        .expect_err("port 9 should fail");

    assert!(err.contains("connect"));
    let status = state.status().await;
    assert!(matches!(
        status.gspro.connection_status,
        ConnectionStatus::Error
    ));
    assert!(status.gspro.last_error.is_some());
}

#[tokio::test]
async fn api_connect_endpoint_updates_gspro_status() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let _server = tokio::spawn(async move {
        let _ = listener.accept().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    });
    let cfg = AppConfig {
        gspro_enabled: true,
        gspro_port: addr.port(),
        ..Default::default()
    };
    let state = AppState::new(&cfg);
    let runtime = SimulatorRuntime::new(state.clone());
    let app = api::router_with_simulators(state, runtime);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/gspro/connect")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["gspro"]["connectionStatus"], "connected");
}
