use utoipa::OpenApi;
use crate::api::{router, HttpConfig};

mod api;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("RUST_LOG").is_err() {
        // set default to info if none is set already
        std::env::set_var("RUST_LOG", "info")
    }
    pretty_env_logger::init();

    let cfg = HttpConfig{
        listen_address: "0.0.0.0".to_string(),
        listen_port: 3000,
    };
    router(cfg).await;

    Ok(())
}
