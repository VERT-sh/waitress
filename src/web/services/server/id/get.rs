use crate::{db::server::Server, response_codes, web::response::ApiResponse};
use actix_web::{get, HttpMessage, HttpRequest, Responder};
use thiserror::Error;

#[derive(Debug, Error)]
enum ServerFetchError {
    #[error("Server not found")]
    ServerNotFound,
}

response_codes!(ServerFetchError {
    ServerNotFound(NOT_FOUND),
});

#[get("")]
pub async fn get(req: HttpRequest) -> Result<impl Responder, ServerFetchError> {
    let server = req
        .extensions_mut()
        .remove::<Server>()
        .ok_or_else(|| ServerFetchError::ServerNotFound)?;
    Ok(ApiResponse::Success(server))
}
