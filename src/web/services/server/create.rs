use actix_web::{post, web::Json, Responder};
use serde::Deserialize;
use thiserror::Error;

use crate::{db::server::ServerProvisionError, error_variants};

#[derive(Debug, Error)]
enum ServerCreateError {
    #[error("Invalid server name")]
    InvalidName,
    #[error("Invalid server host")]
    InvalidHost,
    #[error("Invalid server port")]
    InvalidPort,
    #[error("Failed to provision your server: {0}")]
    ProvisionError(#[from] ServerProvisionError),
}

error_variants!(ServerCreateError {
    InvalidName(BAD_REQUEST),
    InvalidHost(BAD_REQUEST),
    InvalidPort(BAD_REQUEST),
    ProvisionError(INTERNAL_SERVER_ERROR)
});

#[derive(Deserialize)]
struct ServerCreateRequest {
    name: String,
    host: String,
    port: u16,
}

#[post("/create")]
pub async fn create(req: Json<ServerCreateRequest>) -> Result<impl Responder, ServerCreateError> {
    Ok("Server created")
}
