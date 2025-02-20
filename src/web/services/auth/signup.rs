use actix_web::{
    post,
    web::{Data, Json},
    Responder,
};
use serde::Deserialize;

use crate::{
    db::{user::User, Database},
    web::response::ApiResponse,
};

#[derive(Deserialize)]
struct Signup {
    username: String,
    password: String,
}

#[post("/signup")]
pub async fn signup(body: Json<Signup>, data: Data<Database>) -> impl Responder {
    let Signup { username, password } = body.into_inner();
    let user = User::create(username, password, &data.pool).await;
    match user {
        Ok(user) => ApiResponse::Success(user),
        Err(e) => ApiResponse::Error(e.to_string()),
    }
}
