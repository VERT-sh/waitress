use crate::db::server::Server;
use actix_ws::Session;
use bollard::{container::AttachContainerOptions, Docker};
use futures::StreamExt as _;
use log::warn;
use std::sync::Arc;
use tokio::{
    pin,
    sync::{Mutex, Notify},
};

use super::message::WebsocketMessage;

pub async fn docker_stdout(
    server: Arc<Server>,
    session: Arc<Mutex<Session>>,
    waiter: Arc<Notify>,
) -> anyhow::Result<()> {
    let docker = Docker::connect_with_local_defaults()?;
    let container_name = server.container_name();
    let mut stream = docker
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
        .await?;

    let shutdown = waiter.notified();
    pin!(shutdown);

    'outer: loop {
        tokio::select! {
            _ = &mut shutdown => {
                log::info!("docker stdout stream shutting down");
                break 'outer;
            }

            maybe_event = stream.output.next() => {
                match maybe_event {
                    Some(Ok(event)) => {
                        let bytes = event.into_bytes();
                        let stdout = String::from_utf8_lossy(&bytes).to_string();
                        let mut session = session.lock().await;
                        session.text(WebsocketMessage::LogMessage(stdout)).await?;
                    }
                    Some(Err(e)) => {
                        log::error!("failed to read docker stdout: {}", e);
                        break 'outer;
                    }
                    None => {
                        log::error!("docker stdout stream ended unexpectedly");
                        break 'outer;
                    }
                }
            }
        }
    }

    Ok(())
}
