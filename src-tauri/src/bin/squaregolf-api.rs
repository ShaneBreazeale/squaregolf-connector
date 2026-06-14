use squaregolf_connector::{api, config::AppConfig};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "squaregolf_connector=info,tower_http=info".into()),
        )
        .init();

    let config = match AppConfig::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    };

    if let Err(err) = api::serve(config).await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
