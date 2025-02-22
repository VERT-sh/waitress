use actix_web::rt;
use bollard::{container::AttachContainerOptions, Docker};
use futures::StreamExt as _;
use std::sync::Arc;
use tokio::{
    io::AsyncWriteExt as _,
    pin,
    sync::{broadcast, mpsc, Notify},
};

pub async fn create_container_stream(
    container_name: impl Into<String>,
    waiter: Arc<Notify>,
) -> anyhow::Result<(mpsc::Sender<String>, broadcast::Receiver<String>)> {
    let docker = Docker::connect_with_local_defaults()?;

    // stdout should have many readers and one writer
    // stdin should have one reader and many writers
    // so we need to create a stream for each
    // unfortunately there's no spmc in tokio, but
    // we can use broadcast for stdout and mpsc for stdin
    let (stdout_tx, stdout_rx) = broadcast::channel(512);
    let (stdin_tx, mut stdin_rx) = mpsc::channel::<String>(512);

    let container_name = container_name.into();
    let options = Some(AttachContainerOptions::<String> {
        stream: Some(true),
        stdin: Some(true),
        stdout: Some(true),
        stderr: Some(true),
        ..Default::default()
    });

    let mut stream = docker.attach_container(&container_name, options).await?;

    rt::spawn(async move {
        let shutdown = waiter.notified();
        pin!(shutdown);

        'outer: loop {
            tokio::select! {
                _ = &mut shutdown => {
                    break 'outer;
                }

                maybe_event = stream.output.next() => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            let bytes = event.into_bytes();
                            let stdout = String::from_utf8_lossy(&bytes).to_string();
                            stdout_tx.send(stdout).ok();
                        }

                        _ => {}
                    }
                }

                maybe_command = stdin_rx.recv() => {
                    match maybe_command {
                        Some(command) => {
                            stream.input.write_all(command.as_bytes()).await.ok();
                            stream.input.flush().await.ok();
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    Ok((stdin_tx, stdout_rx))
}
