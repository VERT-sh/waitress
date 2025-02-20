use actix_web::{post, Responder};

enum ServerCreateError {
    InvalidName,
    InvalidHost,
}

#[post("/create")]
pub async fn create() -> impl Responder {
    "Create server"
}
