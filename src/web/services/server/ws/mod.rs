mod docker_stdout;
mod message;

use crate::db::{server::Server, user::User, Database};
use actix_web::{
    get,
    web::{self, Data},
    HttpRequest, Responder,
};
use actix_ws::Message;
use docker_stdout::docker_stdout;
use futures::StreamExt as _;
use log::info;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
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
    let notify = Arc::new(Notify::new());

    actix_web::rt::spawn(docker_stdout(
        Arc::clone(&server),
        Arc::clone(&session),
        Arc::clone(&notify),
    ));

    actix_web::rt::spawn(async move {
        while let Some(Ok(msg)) = msg_stream.next().await {
            match msg {
                Message::Ping(bytes) => {
                    let mut session = session.lock().await;
                    if session.pong(&bytes).await.is_err() {
                        break;
                    }
                }

                Message::Text(text) => {
                    info!("received text: {}", text);
                }

                _ => {}
            }
        }

        info!("session closed");
        notify.notify_waiters();
    });

    Ok(response)
}
