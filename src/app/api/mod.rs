mod thermav;
mod error;

use std::fs;
use axum::response::{Html, IntoResponse};
use axum::{routing, Router};
use axum::http::StatusCode;
use axum::routing::get;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api;
use crate::api::error::Error;

#[derive(Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub listen_address: String,
    pub listen_port: u16,
}

pub async fn router(cfg: HttpConfig) {
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
        .nest("/api", thermav::create_router(thermav_lib::ThermaV{}))
        .route("/health", get(health));

    let listener: TcpListener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            log::error!(target: "api", "Unable to create TCP listener: {}", e);
            std::process::exit(1);
        }
    };

    log::info!(target: "api", "Listening on http://{}", addr);

    axum::serve(listener, app).await.unwrap_or_else(|e| {
        log::error!(target: "api", "Unable to start server: {}", e);
        std::process::exit(1);
    });
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
