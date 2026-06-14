use squaregolf_connector::config::{AppConfig, ConfigStore};

#[test]
fn api_port_defaults_to_8080() {
    let cfg =
        AppConfig::from_iter_and_env(["squaregolf-connector"], std::iter::empty::<(&str, &str)>())
            .expect("default config");

    assert_eq!(cfg.api_port, 8080);
}

#[test]
fn api_port_can_be_set_by_cli() {
    let cfg = AppConfig::from_iter_and_env(
        ["squaregolf-connector", "--api-port", "5177"],
        [("SQUAREGOLF_API_PORT", "8081")],
    )
    .expect("cli config");

    assert_eq!(cfg.api_port, 5177);
}

#[test]
fn api_port_can_be_set_by_env_when_cli_omits_it() {
    let cfg =
        AppConfig::from_iter_and_env(["squaregolf-connector"], [("SQUAREGOLF_API_PORT", "9090")])
            .expect("env config");

    assert_eq!(cfg.api_port, 9090);
}

#[test]
fn invalid_api_port_is_rejected() {
    let err = AppConfig::from_iter_and_env(
        ["squaregolf-connector", "--api-port", "70000"],
        std::iter::empty::<(&str, &str)>(),
    )
    .expect_err("port should be invalid");

    assert!(err.contains("api port"));
}

#[test]
fn simulator_endpoints_can_be_set_by_cli() {
    let cfg = AppConfig::from_iter_and_env(
        [
            "squaregolf-connector",
            "--enable-gspro",
            "--gspro-host",
            "192.168.1.20",
            "--gspro-port",
            "921",
            "--enable-it",
            "--it-host",
            "192.168.1.21",
            "--it-port",
            "999",
        ],
        std::iter::empty::<(&str, &str)>(),
    )
    .expect("sim config");

    assert!(cfg.gspro_enabled);
    assert_eq!(cfg.gspro_host, "192.168.1.20");
    assert_eq!(cfg.gspro_port, 921);
    assert!(cfg.infinite_tees_enabled);
    assert_eq!(cfg.infinite_tees_host, "192.168.1.21");
    assert_eq!(cfg.infinite_tees_port, 999);
}

#[test]
fn config_store_loads_legacy_go_settings() {
    let path = unique_temp_config_path("legacy");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        r#"{
            "gsproIP": "192.168.1.30",
            "gsproPort": 1921,
            "gsproAutoConnect": true,
            "infiniteTeesIP": "192.168.1.31",
            "infiniteTeesPort": 1999,
            "infiniteTeesAutoConnect": true
        }"#,
    )
    .unwrap();

    let cfg = ConfigStore::new(&path)
        .load()
        .expect("load config")
        .expect("config exists");

    assert_eq!(cfg.gspro_host, "192.168.1.30");
    assert_eq!(cfg.gspro_port, 1921);
    assert!(cfg.gspro_enabled);
    assert_eq!(cfg.infinite_tees_host, "192.168.1.31");
    assert_eq!(cfg.infinite_tees_port, 1999);
    assert!(cfg.infinite_tees_enabled);

    let _ = std::fs::remove_file(path);
}

#[test]
fn cli_and_env_override_persisted_base_config() {
    let cfg = AppConfig::from_iter_env_and_base(
        [
            "squaregolf-connector",
            "--gspro-host",
            "10.0.0.10",
            "--enable-gspro",
        ],
        [("SQUAREGOLF_API_PORT", "9091")],
        AppConfig {
            gspro_host: "192.168.1.30".to_string(),
            gspro_port: 1921,
            infinite_tees_host: "192.168.1.31".to_string(),
            infinite_tees_enabled: true,
            ..Default::default()
        },
    )
    .expect("merged config");

    assert_eq!(cfg.api_port, 9091);
    assert_eq!(cfg.gspro_host, "10.0.0.10");
    assert_eq!(cfg.gspro_port, 1921);
    assert!(cfg.gspro_enabled);
    assert_eq!(cfg.infinite_tees_host, "192.168.1.31");
    assert!(cfg.infinite_tees_enabled);
}

fn unique_temp_config_path(label: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("squaregolf-config-{label}-{nanos}"))
        .join("config.json")
}
