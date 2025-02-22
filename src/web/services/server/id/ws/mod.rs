mod container_stream;
mod message;
mod ping;
mod stdin;
mod stdout;

use crate::db::{server::Server, user::User, Database};
use actix_web::{
    get, rt,
    web::{self, Data},
    HttpRequest, Responder,
};
use bollard::{container::AttachContainerOptions, Docker};
use container_stream::create_container_stream;
use message::{handle_messages, WebsocketState};
use ping::ping;
use serde::Deserialize;
use std::sync::Arc;
use stdout::receive_stdout;
use tokio::sync::{Mutex, Notify};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct Info {
    auth: String,
}

#[get("/ws")]
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

    let (response, session, msg_stream) = actix_ws::handle(&req, body)?;

    let container_name = server.container_name();

    let server = Arc::new(server);
    let session = Arc::new(Mutex::new(session));
    let notify = Arc::new(Notify::new());
    let docker = Arc::new(Docker::connect_with_local_defaults().unwrap());
    let stream = Arc::new(Mutex::new(
        docker
            .attach_container(
                &container_name,
                Some(AttachContainerOptions::<String> {
                    stream: Some(true),
                    stdin: Some(true),
                    stdout: Some(true),
                    stderr: Some(true),
                    ..Default::default()
                }),
            )
            .await
            .unwrap(),
    ));

    let notify_clone = Arc::clone(&notify);

    let (tx, rx) = create_container_stream(container_name.clone(), notify_clone)
        .await
        .unwrap();

    rt::spawn(ping(Arc::clone(&session), Arc::clone(&notify)));
    rt::spawn(receive_stdout(
        rx,
        Arc::clone(&session),
        Arc::clone(&notify),
    ));

    let state = WebsocketState {
        docker: Arc::clone(&docker),
        server: Arc::clone(&server),
        session: Arc::clone(&session),
        notify: Arc::clone(&notify),
        tx,
        msg_stream,
    };

    rt::spawn(handle_messages(state));

    Ok(response)
}
