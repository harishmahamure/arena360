#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    gaming_cafe_api::analytics::worker::run(std::env::args().any(|a| a == "--backfill")).await
}
