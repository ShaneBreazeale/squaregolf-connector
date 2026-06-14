pub mod api;
pub mod config;
pub mod core;
pub mod device;
pub mod simulator;
pub mod squarelaunch;

pub fn run() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "squaregolf_connector=info,tower_http=info".into()),
        )
        .try_init();

    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = api::spawn_from_env(handle).await {
                    tracing::error!("API server failed: {err}");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
