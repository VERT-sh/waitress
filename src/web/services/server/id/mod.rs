mod delete;
mod ws;

use actix_web::{
    middleware::from_fn,
    web::{self, ServiceConfig},
};

use crate::web::middleware::owns_server::owns_server;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/{id}")
            .service(ws::ws)
            .service(delete::delete)
            .wrap(from_fn(owns_server)),
    );
}
