use actix_web::{
    post,
    web::{Data, Json},
    Responder,
};
use serde::Deserialize;

use crate::{
    config::CONFIG,
    db::{
        user::{User, UserCreationError},
        Database,
    },
    web::response::ApiResponse,
};

#[derive(Deserialize)]
struct Signup {
    username: String,
    password: String,
}

#[post("/signup")]
pub async fn signup(
    body: Json<Signup>,
    data: Data<Database>,
) -> Result<impl Responder, UserCreationError> {
    if !CONFIG.signups_enabled {
        return Err(UserCreationError::SignupsDisabled);
    }
    let Signup { username, password } = body.into_inner();
    let user = User::create(username, password, &data.pool).await?;
    Ok(ApiResponse::Success(user))
}
