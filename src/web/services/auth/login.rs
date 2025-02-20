use crate::{
    db::{user::User, Database},
    error_variants,
    web::response::ApiResponse,
};
use actix_web::{
    post,
    web::{Data, Json},
    Responder,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize)]
struct Login {
    username: String,
    password: String,
}

#[derive(Debug, Error)]
enum LoginError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Failed to create auth token: {0}")]
    AuthCreationError(#[from] crate::db::user::AuthCreationError),
}

error_variants!(LoginError {
    InvalidCredentials(BAD_REQUEST),
    AuthCreationError(INTERNAL_SERVER_ERROR)
});

#[derive(Serialize)]
struct LoginResponse {
    user: User,
    token: String,
}

#[post("/login")]
pub async fn login(body: Json<Login>, data: Data<Database>) -> Result<impl Responder, LoginError> {
    let Login { username, password } = body.into_inner();
    let user = User::from_username(username, &data.pool)
        .await
        .ok_or(LoginError::InvalidCredentials)?;
    if !user.verify_password(password, &data.pool).await {
        return Err(LoginError::InvalidCredentials);
    }
    let jwt = user.create_token().await?;
    Ok(ApiResponse::Success(LoginResponse { user, token: jwt }))
}
