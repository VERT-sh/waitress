mod login;
mod signup;

use actix_web::web::{self, ServiceConfig};

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .service(signup::signup)
            .service(login::login),
    );
}
