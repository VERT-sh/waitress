use super::message::WebsocketMessage;
use crate::db::server::Server;
use actix_ws::Session;
use bollard::{container::AttachContainerOptions, Docker};
use futures::StreamExt as _;
use std::sync::Arc;
use tokio::{
    pin,
    sync::{broadcast::Receiver, Mutex, Notify},
};

pub async fn receive_stdout(
    mut rx: Receiver<String>,
    session: Arc<Mutex<Session>>,
    waiter: Arc<Notify>,
) -> anyhow::Result<()> {
    let shutdown = waiter.notified();
    pin!(shutdown);

    'outer: loop {
        tokio::select! {
            _ = &mut shutdown => {
                break 'outer;
            }

            stdout = rx.recv() => {
                match stdout {
                    Ok(stdout) => {
                        let mut session = session.lock().await;
                        session.text(WebsocketMessage::Log(stdout.trim().to_string())).await?;
                    }

                    Err(e) => {
                        log::error!("error receiving stdout: {}", e);
                        break 'outer;
                    }
                }
            }
        }
    }

    Ok(())
}
