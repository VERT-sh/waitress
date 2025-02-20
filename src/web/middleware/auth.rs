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

pub async fn authenticated(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let Some(data) = req.app_data::<Data<Database>>() else {
        return middleware_error!(ErrorInternalServerError, "Database connection is missing");
    };
    let auth_header = req.headers().get("Authorization");
    let Some(auth_header) = auth_header else {
        return middleware_error!(ErrorUnauthorized, "Authorization header is missing");
    };
    let auth_header = match auth_header.to_str() {
        Ok(auth_header) => auth_header,
        Err(_) => return middleware_error!(ErrorUnauthorized, "Authorization header is invalid"),
    };
    let user = User::from_token(auth_header, &data.pool).await;
    // let Ok(user) = user else {
    //     return middleware_error!(ErrorUnauthorized, "Invalid token: {}", err);
    // };
    let user = match user {
        Ok(user) => user,
        Err(err) => return middleware_error!(ErrorUnauthorized, "Invalid token: {}", err),
    };
    req.extensions_mut().insert(user);
    next.call(req).await
}
