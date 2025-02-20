use crate::{
    db::{user::User, Database},
    middleware_error,
};
use actix_web::{
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    middleware::Next,
    web::Data,
    Error, HttpMessage,
};
use regex::Regex;

pub async fn authenticated(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    // if the route exactly matches `/api/server/{id}/ws`, we don't need to authenticate
    // damn you, actix!! this is gonna cause a vulnerability!
    let regex = Regex::new(
        r"^/api/server/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/ws$",
    )
    .unwrap();
    if regex.is_match(req.path()) {
        return next.call(req).await;
    }

    let data = req.app_data::<Data<Database>>().ok_or_else(|| {
        middleware_error!(ErrorInternalServerError, "Database connection is missing")
    })?;

    let auth_header = req
        .headers()
        .get("Authorization")
        .ok_or_else(|| middleware_error!(ErrorUnauthorized, "Authorization header is missing"))?
        .to_str()
        .map_err(|_| middleware_error!(ErrorUnauthorized, "Invalid Authorization header"))?;

    let user = User::from_token(auth_header, &data.pool)
        .await
        .map_err(|err| middleware_error!(ErrorUnauthorized, "Invalid token: {}", err))?;

    let mut extensions = req.extensions_mut();
    extensions.insert(user);
    drop(extensions);
    next.call(req).await
}
