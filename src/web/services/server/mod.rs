mod create;
mod id;

use actix_web::{
    middleware::from_fn,
    web::{self, ServiceConfig},
};

use crate::web::middleware::auth::authenticated;

pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/server")
            .service(create::create)
            .configure(id::configure)
            .wrap(from_fn(authenticated)),
    );
}
