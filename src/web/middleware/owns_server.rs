use crate::{
    db::{server::Server, user::User, Database},
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

pub async fn owns_server(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let regex = Regex::new(
        r"^/api/server/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/ws$",
    )
    .unwrap();
    if regex.is_match(req.path()) {
        return next.call(req).await;
    }

    {
        let (server, user_id) = {
            let extensions = req.extensions();
            let user = extensions
                .get::<User>()
                .ok_or_else(|| middleware_error!(ErrorInternalServerError, "User is missing"))?;
            // this middleware will only be called in routes matching /api/server/{id}/*
            let id = req.match_info().get("id").ok_or_else(|| {
                middleware_error!(ErrorInternalServerError, "Server ID is missing")
            })?;
            let id = uuid::Uuid::parse_str(id)
                .map_err(|_| middleware_error!(ErrorBadRequest, "Invalid server ID"))?;

            let data = req.app_data::<Data<Database>>().ok_or_else(|| {
                middleware_error!(ErrorInternalServerError, "Database connection is missing")
            })?;

            (
                Server::from_id(id, &data.pool)
                    .await
                    .ok_or_else(|| middleware_error!(ErrorNotFound, "Server not found"))?,
                user.id,
            )
        };

        if server.owner != user_id {
            return Err(middleware_error!(
                ErrorForbidden,
                "You do not own this server"
            ));
        }
        let mut extensions = req.extensions_mut();
        extensions.insert(server);
    }
    next.call(req).await
}
