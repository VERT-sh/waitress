mod error;
mod middleware;
pub mod response;
mod services;

use actix_web::web;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            .configure(services::auth::configure)
            .configure(services::server::configure),
    );
}
