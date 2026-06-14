use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use squaregolf_connector::api;
use squaregolf_connector::config::{AppConfig, ConfigStore};
use squaregolf_connector::core::protocol::parser::parse_shot_ball_metrics;
use squaregolf_connector::core::{AppState, ConnectionStatus};
use tower::ServiceExt;

#[tokio::test]
async fn status_endpoint_reports_configured_api_port() {
    let cfg = AppConfig {
        api_port: 5177,
        ..Default::default()
    };
    let app = api::router(AppState::new(&cfg));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["apiPort"], 5177);
    assert_eq!(json["squarelaunch"]["port"], 2920);
}

#[tokio::test]
async fn openapi_document_is_served() {
    let app = api::router(AppState::new(&AppConfig::default()));

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api-docs/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["openapi"], "3.1.0");
    assert!(json["paths"]["/api/status"].is_object());
}

#[tokio::test]
async fn parsed_ball_metrics_can_update_api_status() {
    let state = AppState::new(&AppConfig::default());
    let metrics = parse_shot_ball_metrics(&[
        "11", "02", "37", "64", "00", "C8", "00", "2C", "01", "E8", "03", "F4", "01", "D0", "07",
        "B8", "0B",
    ])
    .unwrap();
    state.set_last_ball_metrics(metrics).await;
    let app = api::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["device"]["lastBallMetrics"]["speedMps"], 1.0);
    assert_eq!(json["device"]["lastBallMetrics"]["launchAngle"], 2.0);
    assert_eq!(json["device"]["lastBallMetrics"]["totalSpin"], 1000.0);
}

#[tokio::test]
async fn config_endpoint_updates_simulator_settings() {
    let app = api::router(AppState::new(&AppConfig::default()));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/config")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "gsproEnabled": true,
                        "gsproHost": "192.168.1.20",
                        "gsproPort": 921,
                        "infiniteTeesEnabled": true,
                        "infiniteTeesHost": "192.168.1.21",
                        "infiniteTeesPort": 999
                    }"#,
                ))
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
    assert_eq!(json["gspro"]["enabled"], true);
    assert_eq!(json["gspro"]["host"], "192.168.1.20");
    assert_eq!(json["gspro"]["port"], 921);
    assert_eq!(json["infiniteTees"]["enabled"], true);
    assert_eq!(json["infiniteTees"]["host"], "192.168.1.21");
    assert_eq!(json["infiniteTees"]["port"], 999);
}

#[tokio::test]
async fn config_endpoint_persists_updated_settings_when_store_is_configured() {
    let path = unique_temp_config_path("api");
    let store = ConfigStore::new(&path);
    let app = api::router_with_store(AppState::new(&AppConfig::default()), Some(store.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/config")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{
                        "gsproEnabled": true,
                        "gsproHost": "10.0.0.20",
                        "gsproPort": 1921,
                        "squarelaunchEnabled": true,
                        "squarelaunchWsHost": "10.0.0.21",
                        "squarelaunchWsPort": 2921
                    }"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let cfg = store
        .load()
        .expect("load persisted config")
        .expect("persisted config");
    assert!(cfg.gspro_enabled);
    assert_eq!(cfg.gspro_host, "10.0.0.20");
    assert_eq!(cfg.gspro_port, 1921);
    assert!(cfg.squarelaunch_enabled);
    assert_eq!(cfg.squarelaunch_ws_host.as_deref(), Some("10.0.0.21"));
    assert_eq!(cfg.squarelaunch_ws_port, 2921);

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn device_disconnect_endpoint_clears_connected_device_state() {
    let state = AppState::new(&AppConfig::default());
    state
        .update_device(|device| {
            device.connection_status = ConnectionStatus::Connected;
            device.device_name = Some("SquareGolf(1234)".to_string());
            device.battery_level = Some(87);
        })
        .await;
    let app = api::router(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/device/disconnect")
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
    assert_eq!(json["device"]["connectionStatus"], "disconnected");
    assert!(json["device"]["deviceName"].is_null());
    assert!(json["device"]["batteryLevel"].is_null());
}

fn unique_temp_config_path(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("squaregolf-api-{label}-{nanos}"))
        .join("config.json")
}
