use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

pub const DEFAULT_API_PORT: u16 = 8080;
pub const DEFAULT_SQUARELAUNCH_WS_PORT: u16 = 2920;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub api_port: u16,
    pub gspro_host: String,
    pub gspro_port: u16,
    pub gspro_enabled: bool,
    pub infinite_tees_host: String,
    pub infinite_tees_port: u16,
    pub infinite_tees_enabled: bool,
    pub squarelaunch_ws_host: Option<String>,
    pub squarelaunch_ws_port: u16,
    pub squarelaunch_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            api_port: DEFAULT_API_PORT,
            gspro_host: "127.0.0.1".to_string(),
            gspro_port: 921,
            gspro_enabled: false,
            infinite_tees_host: "127.0.0.1".to_string(),
            infinite_tees_port: 999,
            infinite_tees_enabled: false,
            squarelaunch_ws_host: None,
            squarelaunch_ws_port: DEFAULT_SQUARELAUNCH_WS_PORT,
            squarelaunch_enabled: false,
        }
    }
}

impl AppConfig {
    pub fn from_env() -> Result<Self, String> {
        let config = ConfigStore::default().load()?.unwrap_or_default();
        Self::from_iter_env_and_base(std::env::args(), std::env::vars(), config)
    }

    pub fn from_iter_and_env<I, S, E, K, V>(args: I, env: E) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        E: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        Self::from_iter_env_and_base(args, env, Self::default())
    }

    pub fn from_iter_env_and_base<I, S, E, K, V>(
        args: I,
        env: E,
        mut config: Self,
    ) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
        E: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let env_pairs: Vec<(String, String)> = env
            .into_iter()
            .map(|(key, value)| (key.as_ref().to_string(), value.as_ref().to_string()))
            .collect();

        if let Some(port) = env_value(&env_pairs, "SQUAREGOLF_API_PORT") {
            config.api_port = parse_port(port, "api port")?;
        }
        if let Some(host) = env_value(&env_pairs, "SQUARELAUNCH_WS_HOST") {
            let host = host.trim();
            if !host.is_empty() {
                config.squarelaunch_ws_host = Some(host.to_string());
            }
        }
        if let Some(port) = env_value(&env_pairs, "SQUARELAUNCH_WS_PORT") {
            config.squarelaunch_ws_port = parse_port(port, "SquareLaunch websocket port")?;
        }
        if let Some(enabled) = env_value(&env_pairs, "SQUARELAUNCH_WS") {
            config.squarelaunch_enabled = env_truthy(enabled);
        }
        if let Some(host) = env_value(&env_pairs, "GSPRO_HOST") {
            config.gspro_host = host.trim().to_string();
        }
        if let Some(port) = env_value(&env_pairs, "GSPRO_PORT") {
            config.gspro_port = parse_port(port, "GSPro port")?;
        }
        if let Some(enabled) = env_value(&env_pairs, "GSPRO_ENABLED") {
            config.gspro_enabled = env_truthy(enabled);
        }
        if let Some(host) = env_value(&env_pairs, "INFINITE_TEES_HOST") {
            config.infinite_tees_host = host.trim().to_string();
        }
        if let Some(port) = env_value(&env_pairs, "INFINITE_TEES_PORT") {
            config.infinite_tees_port = parse_port(port, "Infinite Tees port")?;
        }
        if let Some(enabled) = env_value(&env_pairs, "INFINITE_TEES_ENABLED") {
            config.infinite_tees_enabled = env_truthy(enabled);
        }

        let mut args = args.into_iter().skip(1).peekable();
        while let Some(arg) = args.next() {
            match arg.as_ref() {
                "--api-port" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--api-port requires a value".to_string())?;
                    config.api_port = parse_port(value.as_ref(), "api port")?;
                }
                "--squarelaunch-ws-host" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--squarelaunch-ws-host requires a value".to_string())?;
                    let trimmed = value.as_ref().trim();
                    config.squarelaunch_ws_host =
                        (!trimmed.is_empty()).then(|| trimmed.to_string());
                }
                "--squarelaunch-ws-port" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--squarelaunch-ws-port requires a value".to_string())?;
                    config.squarelaunch_ws_port =
                        parse_port(value.as_ref(), "SquareLaunch websocket port")?;
                }
                "--enable-squarelaunch-ws" => {
                    config.squarelaunch_enabled = true;
                }
                "--disable-squarelaunch-ws" => {
                    config.squarelaunch_enabled = false;
                }
                "--gspro-host" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--gspro-host requires a value".to_string())?;
                    config.gspro_host = value.as_ref().trim().to_string();
                }
                "--gspro-port" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--gspro-port requires a value".to_string())?;
                    config.gspro_port = parse_port(value.as_ref(), "GSPro port")?;
                }
                "--enable-gspro" => {
                    config.gspro_enabled = true;
                }
                "--disable-gspro" => {
                    config.gspro_enabled = false;
                }
                "--it-host" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--it-host requires a value".to_string())?;
                    config.infinite_tees_host = value.as_ref().trim().to_string();
                }
                "--it-port" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--it-port requires a value".to_string())?;
                    config.infinite_tees_port = parse_port(value.as_ref(), "Infinite Tees port")?;
                }
                "--enable-it" => {
                    config.infinite_tees_enabled = true;
                }
                "--disable-it" => {
                    config.infinite_tees_enabled = false;
                }
                unknown if unknown.starts_with("--") => {
                    return Err(format!("unknown argument {unknown}"));
                }
                _ => {}
            }
        }

        Ok(config)
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl Default for ConfigStore {
    fn default() -> Self {
        Self::new(default_config_path())
    }
}

impl ConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<Option<AppConfig>, String> {
        let data = match std::fs::read_to_string(&self.path) {
            Ok(data) => data,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(format!("read config {}: {err}", self.path.display())),
        };
        let value: Value = serde_json::from_str(&data)
            .map_err(|err| format!("parse config {}: {err}", self.path.display()))?;
        Ok(Some(config_from_json_value(&value)?))
    }

    pub fn save(&self, config: &AppConfig) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create config directory {}: {err}", parent.display()))?;
        }
        let data = serde_json::to_string_pretty(config)
            .map_err(|err| format!("serialize config {}: {err}", self.path.display()))?;
        std::fs::write(&self.path, data)
            .map_err(|err| format!("write config {}: {err}", self.path.display()))
    }
}

fn default_config_path() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".squaregolf-connector").join("config.json")
}

fn config_from_json_value(value: &Value) -> Result<AppConfig, String> {
    let mut config = AppConfig::default();

    if let Some(port) = number_field(value, &["apiPort"]) {
        config.api_port = port_to_u16(port, "apiPort")?;
    }
    if let Some(host) = string_field(value, &["gsproHost", "gsproIP"]) {
        config.gspro_host = host;
    }
    if let Some(port) = number_field(value, &["gsproPort"]) {
        config.gspro_port = port_to_u16(port, "gsproPort")?;
    }
    if let Some(enabled) = bool_field(value, &["gsproEnabled", "gsproAutoConnect"]) {
        config.gspro_enabled = enabled;
    }
    if let Some(host) = string_field(value, &["infiniteTeesHost", "infiniteTeesIP"]) {
        config.infinite_tees_host = host;
    }
    if let Some(port) = number_field(value, &["infiniteTeesPort"]) {
        config.infinite_tees_port = port_to_u16(port, "infiniteTeesPort")?;
    }
    if let Some(enabled) = bool_field(value, &["infiniteTeesEnabled", "infiniteTeesAutoConnect"]) {
        config.infinite_tees_enabled = enabled;
    }
    if let Some(host) = string_field(value, &["squarelaunchWsHost", "squarelaunchWSHost"]) {
        config.squarelaunch_ws_host = Some(host);
    }
    if let Some(port) = number_field(value, &["squarelaunchWsPort", "squarelaunchWSPort"]) {
        config.squarelaunch_ws_port = port_to_u16(port, "squarelaunchWsPort")?;
    }
    if let Some(enabled) = bool_field(value, &["squarelaunchEnabled"]) {
        config.squarelaunch_enabled = enabled;
    }

    Ok(config)
}

fn string_field(value: &Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        value
            .get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|candidate| !candidate.is_empty())
            .map(ToOwned::to_owned)
    })
}

fn number_field(value: &Value, names: &[&str]) -> Option<u64> {
    names
        .iter()
        .find_map(|name| value.get(name).and_then(Value::as_u64))
}

fn bool_field(value: &Value, names: &[&str]) -> Option<bool> {
    names
        .iter()
        .find_map(|name| value.get(name).and_then(Value::as_bool))
}

fn port_to_u16(value: u64, label: &str) -> Result<u16, String> {
    if value == 0 || value > u16::MAX as u64 {
        return Err(format!("{label} must be from 1 to 65535"));
    }
    Ok(value as u16)
}

fn env_value<'a>(env: &'a [(String, String)], key: &str) -> Option<&'a str> {
    env.iter()
        .find(|(candidate, _)| candidate == key)
        .map(|(_, value)| value.as_str())
}

fn env_truthy(value: &str) -> bool {
    !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "" | "0" | "false" | "off" | "no"
    )
}

fn parse_port(value: &str, label: &str) -> Result<u16, String> {
    let parsed = value
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("{label} must be an integer from 1 to 65535"))?;
    if parsed == 0 || parsed > u16::MAX as u32 {
        return Err(format!("{label} must be from 1 to 65535"));
    }
    Ok(parsed as u16)
}
