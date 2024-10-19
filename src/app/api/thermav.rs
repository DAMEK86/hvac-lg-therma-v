use crate::api::not_found;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{routing, Json, Router};
use utoipa::OpenApi;

const API_VERSION: &str = "v1";

#[derive(OpenApi)]
#[openapi(
    paths(
        get_coil,
        post_coil
    )
)]
pub(super) struct ThermavApi;

pub fn create_router(hvac: thermav_lib::ThermaV) -> Router {
    Router::new().nest(
        &format!("/{}", API_VERSION),
        Router::new()
            .route(
                "/coils/:name",
                routing::get(get_coil),
            )
            .route(
                "/coils/:name",
                routing::post(post_coil),
            )
            .with_state(hvac),
    )
}

#[utoipa::path(get, path = "/v1/coils/{name}",
    responses(
        (status = OK, body = str),
        (status = NOT_FOUND, description = "Register not found")
    ),
    params(
            ("name" = String, Path, description = "Register name"),
    ))]
async fn get_coil(
    Path(name): Path<String>,
    State(hvac): State<thermav_lib::ThermaV>,
) -> impl IntoResponse {
    return (StatusCode::OK, Json(hvac)).into_response();

    not_found("Register not found".into(), None).into_response()
}

#[utoipa::path(post, path = "/v1/coils/{name}",
    responses(
        (status = OK, body = str),
        (status = NOT_FOUND, description = "Register not found")
    ),
    params(
            ("name" = String, Path, description = "Register name"),
    ))]
async fn post_coil(
    Path(name): Path<String>,
    State(hvac): State<thermav_lib::ThermaV>,
) -> impl IntoResponse {

    not_found("Register not found".into(), None).into_response()
}
