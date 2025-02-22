use crate::db::server::Server;
use actix_ws::{Message, MessageStream, Session};
use bollard::Docker;
use bytestring::ByteString;
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, Notify};

use super::stdin::run_command;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "camelCase")]
pub enum WebsocketMessage {
    Ping,            //  all directions
    Log(String),     // server -> client
    Command(String), // client -> server
}

impl Into<ByteString> for WebsocketMessage {
    fn into(self) -> ByteString {
        serde_json::to_string(&self)
            .expect("Failed to serialize WebsocketMessage")
            .into()
    }
}

pub struct WebsocketState {
    pub docker: Arc<Docker>,
    pub server: Arc<Server>,
    pub session: Arc<Mutex<Session>>,
    pub notify: Arc<Notify>,
    pub msg_stream: MessageStream,
    pub tx: mpsc::Sender<String>,
}

pub async fn handle_messages(mut state: WebsocketState) -> anyhow::Result<()> {
    while let Some(Ok(msg)) = state.msg_stream.next().await {
        match message_loop(&state, msg).await {
            Ok(true) => break,
            Err(e) => log::error!("error in message loop: {}", e),
            _ => {}
        }
    }
    Ok(())
}

async fn message_loop(state: &WebsocketState, msg: Message) -> anyhow::Result<bool> {
    match msg {
        Message::Ping(bytes) => {
            let mut session = state.session.lock().await;
            if session.pong(&bytes).await.is_err() {
                return Err(anyhow::anyhow!("failed to send pong"));
            }
        }

        Message::Text(text) => {
            let text = text.to_string();
            // try to parse it
            let message = match serde_json::from_str::<WebsocketMessage>(&text) {
                Ok(message) => message,
                Err(_) => {
                    log::info!("invalid message: {}", text);
                    return Ok(false);
                }
            };

            match message {
                WebsocketMessage::Command(command) => {
                    run_command(command, state).await?;
                }

                WebsocketMessage::Ping => {}

                _ => {
                    log::warn!("unhandled message: {:?}", message);
                }
            }
        }

        Message::Close(_) => {
            state.notify.notify_waiters();
            return Ok(true);
        }

        _ => {}
    }

    Ok(false)
}
