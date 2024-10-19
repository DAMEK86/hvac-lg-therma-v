mod thermav;
mod error;

use axum::{Router};
use axum::http::StatusCode;
use axum::routing::get;
use tokio::net::TcpListener;
use tokio::signal;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use thermav_lib::config::HttpConfig;
use crate::api::error::Error;

pub async fn start_service(cfg: HttpConfig) {
    #[derive(OpenApi)]
    #[openapi(
        paths(health),
        nest(
            (path = "/api", api = thermav::ThermavApi)
        ))]
    struct ApiDoc;
    let addr = format!("{}:{}", cfg.listen_address, cfg.listen_port);
    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/apidoc/openapi.json", ApiDoc::openapi()))
        .nest("/api", thermav::create_router("thermav".to_string()))
        .route("/health", get(health));

    let listener: TcpListener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!(target: "api", "Unable to create TCP listener: {}", e);
            std::process::exit(1);
        }
    };

    log::info!(target: "api", "Listening on http://{}", addr);

    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.unwrap_or_else(|e| {
        log::error!(target: "api", "Unable to start server: {}", e);
        std::process::exit(1);
    });
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.unwrap_or_else(|e| {
            log::error!(target: "http-server", "failed to install Ctrl+C handler: {}", e);
            std::process::exit(1);
        });
    };

    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .unwrap_or_else(|e| {
                log::error!(target: "http-server", "failed to install terminate signal handler: {}", e);
                std::process::exit(1);
            })
            .recv()
            .await;
    };

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}


/// Get health of the API.
#[utoipa::path(get, path = "/health", responses((status = OK, body = str)))]
async fn health() -> &'static str {
    "ok"
}

fn not_found(message: String, details: Option<String>) -> Error {
    Error {
        status_code: StatusCode::NOT_FOUND,
        message,
        details,
    }
}
