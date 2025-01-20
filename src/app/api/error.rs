use axum::http::StatusCode;
use axum::{response::IntoResponse, Json};

pub struct Error {
    pub status_code: StatusCode,
    pub message: String,
    pub details: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ErrorResponse {
    pub message: String,
    pub details: Option<String>,
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        (
            self.status_code,
            Json(ErrorResponse {
                message: self.message,
                details: self.details,
            }),
        )
            .into_response()
    }
}
