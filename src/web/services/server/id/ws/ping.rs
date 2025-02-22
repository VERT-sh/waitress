use super::message::WebsocketMessage;
use actix_ws::Session;
use std::{sync::Arc, time::Duration};
use tokio::{
    pin,
    sync::{Mutex, Notify},
};

pub async fn ping(session: Arc<Mutex<Session>>, waiter: Arc<Notify>) -> anyhow::Result<()> {
    let shutdown = waiter.notified();
    pin!(shutdown);

    'outer: loop {
        tokio::select! {
            _ = &mut shutdown => {
                break 'outer;
            }

            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                let mut session = session.lock().await;
                session.text(WebsocketMessage::Ping).await?;
            }
        }
    }

    Ok(())
}
