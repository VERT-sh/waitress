use crate::{
    db::{server::Server, user::User, Database},
    response_codes,
    web::response::ApiResponse,
};
use actix_web::{get, web::Data, HttpMessage, HttpRequest, Responder};
use thiserror::Error;

#[derive(Debug, Error)]
enum UserFetchError {
    #[error("User not found")]
    UserNotFound,
    #[error("A database error occurred: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

response_codes!(UserFetchError {
    UserNotFound(NOT_FOUND),
    DatabaseError(INTERNAL_SERVER_ERROR),
});

#[get("/all")]
pub async fn all(req: HttpRequest, data: Data<Database>) -> Result<impl Responder, UserFetchError> {
    let extensions = req.extensions();
    let user = extensions
        .get::<User>()
        .ok_or_else(|| UserFetchError::UserNotFound)?;

    let servers = Server::get_all(user.id, &data.pool).await?;

    Ok(ApiResponse::Success(servers))
}
