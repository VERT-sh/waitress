use crate::middleware_error;
use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
    Error,
};

pub async fn authenticated(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let auth_header = req.headers().get("Authorization");
    let Some(auth_header) = auth_header else {
        return middleware_error!(ErrorUnauthorized, "Authorization header is missing");
    };
    let auth_header = match auth_header.to_str() {
        Ok(auth_header) => auth_header,
        Err(_) => return middleware_error!(ErrorUnauthorized, "Authorization header is invalid"),
    };
    next.call(req).await
}
