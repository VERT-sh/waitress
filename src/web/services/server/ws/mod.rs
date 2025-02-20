mod docker_stdout;

use std::sync::Arc;

use crate::db::{server::Server, user::User, Database};
use actix_web::{
    get,
    web::{self, Data},
    HttpMessage as _, HttpRequest, Responder,
};
use bollard::{container::AttachContainerOptions, Docker};
use docker_stdout::docker_stdout;
use futures_util::StreamExt as _;
use serde::Deserialize;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct Info {
    auth: String,
}

#[get("/{id}/ws")]
async fn ws(
    req: HttpRequest,
    body: web::Payload,
    data: Data<Database>,
    path: web::Path<Uuid>,
    query: web::Query<Info>,
) -> actix_web::Result<impl Responder> {
    let user = match User::from_token(&query.auth, &data.pool).await {
        Ok(user) => user,
        Err(_) => {
            return Err(actix_web::error::ErrorUnauthorized(
                "Invalid authentication",
            ))
        }
    };

    let server_id = path.into_inner();
    let server = match Server::from_id(server_id, &data.pool).await {
        Some(server) => server,
        None => return Err(actix_web::error::ErrorNotFound("Server not found")),
    };

    if server.owner != user.id {
        return Err(actix_web::error::ErrorUnauthorized(
            "Invalid authentication",
        ));
    }

    let (response, mut session, mut msg_stream) = actix_ws::handle(&req, body)?;

    let server = Arc::new(server);
    let session = Arc::new(Mutex::new(session));

    actix_web::rt::spawn(docker_stdout(Arc::clone(&server), Arc::clone(&session)));

    Ok(response)
}
