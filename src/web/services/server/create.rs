use actix_web::{
    post,
    web::{Data, Json},
    HttpMessage, HttpRequest, Responder,
};
use serde::Deserialize;
use thiserror::Error;

use crate::{
    db::{
        server::{self, Server, ServerProvisionError},
        Database,
    },
    response_codes,
    web::response::ApiResponse,
};

#[derive(Debug, Error)]
enum ServerCreateError {
    #[error("Invalid server name")]
    InvalidName,
    #[error("Port must be between 1024 and 65535")]
    InvalidPort,
    #[error("{0}")]
    CreationError(#[from] server::ServerCreationError),
    #[error("{0}")]
    ProvisionError(#[from] ServerProvisionError),
    #[error("Invalid authentication")]
    InvalidAuth,
    #[error("Failed to start server: {0}")]
    StartError(#[from] server::ServerStartError),
}

response_codes!(ServerCreateError {
    InvalidName(BAD_REQUEST),
    InvalidPort(BAD_REQUEST),
    CreationError(INTERNAL_SERVER_ERROR),
    ProvisionError(INTERNAL_SERVER_ERROR),
    InvalidAuth(UNAUTHORIZED),
    StartError(INTERNAL_SERVER_ERROR),
});

#[derive(Deserialize)]
struct ServerCreateRequest {
    name: String,
    version: String,
    port: u16,
}

#[post("/create")]
pub async fn create(
    body: Json<ServerCreateRequest>,
    req: HttpRequest,
    data: Data<Database>,
) -> Result<impl Responder, ServerCreateError> {
    let extensions = req.extensions();
    let Some(user) = extensions.get::<crate::db::user::User>() else {
        return Err(ServerCreateError::InvalidAuth);
    };
    let ServerCreateRequest {
        name,
        port,
        version,
    } = body.into_inner();
    if name.is_empty() || name.len() > 128 {
        return Err(ServerCreateError::InvalidName);
    }

    if port < 1024 {
        return Err(ServerCreateError::InvalidPort);
    }

    let server = Server::create(user.id, name, port, version, &data.pool).await?;
    server.start().await?;
    Ok(ApiResponse::Success(server))
}
