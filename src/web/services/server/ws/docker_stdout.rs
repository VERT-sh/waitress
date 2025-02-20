use std::sync::Arc;

use crate::db::server::Server;
use actix_ws::Session;
use bollard::{container::AttachContainerOptions, Docker};
use futures::StreamExt as _;
use tokio::sync::Mutex;

pub async fn docker_stdout(
    server: Arc<Server>,
    session: Arc<Mutex<Session>>,
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

    while let Some(event) = stream.output.next().await {
        let event = event?;
        let bytes = event.into_bytes();
        let stdout = String::from_utf8_lossy(&bytes).to_string();
        let mut session = session.lock().await;
        session.text(stdout).await.ok();
    }

    Ok(())
}
