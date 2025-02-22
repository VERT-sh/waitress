use super::message::WebsocketState;

pub async fn run_command(command: String, state: &WebsocketState) -> anyhow::Result<()> {
    log::trace!("sending command: {}", command);
    state.tx.send(format!("{}\n", command)).await?;
    Ok(())
}
