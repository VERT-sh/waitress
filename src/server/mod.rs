use std::collections::HashMap;

use crate::version::{manifest::VersionManifest, server::ServerError};
use bollard::{
    Docker,
    container::{self, AttachContainerOptions, CreateContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    secret::HostConfig,
    volume::CreateVolumeOptions,
};
use futures::StreamExt as _;
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum ServerProvisionError {
    #[error("Docker error: {0}")]
    DockerError(#[from] bollard::errors::Error),
    #[error("Mojang API error: {0}")]
    VersionError(#[from] reqwest::Error),
    #[error("Version not found")]
    VersionNotFound,
    #[error("Server error: {0}")]
    ServerInfoError(#[from] ServerError),
    #[error("Filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),
}

pub struct Server {
    docker: Docker,
    id: Uuid,
}

impl Server {
    pub async fn provision(version: impl Into<String>) -> Result<Self, ServerProvisionError> {
        let id = Uuid::new_v4();
        let version = version.into();
        let docker = Docker::connect_with_local_defaults()?;
        let manifest = VersionManifest::new().await?;
        let version = manifest
            .get_version(version)
            .ok_or(ServerProvisionError::VersionNotFound)?;
        let server_info = version.get_server_info().await?;
        Self::create_container(&docker, &server_info, &id).await?;
        Ok(Self { docker, id })
    }

    async fn create_container(
        docker: &Docker,
        server_info: &crate::version::server::ServerJarInfo,
        id: &Uuid,
    ) -> Result<(), ServerProvisionError> {
        let image = format!("openjdk:{}", server_info.java_version);

        let create_image_options = CreateImageOptions {
            from_image: image.as_str(),
            ..Default::default()
        };

        let mut stream = docker.create_image(Some(create_image_options), None, None);
        while let Some(event) = stream.next().await {
            println!("{:?}", event?);
        }

        if !fs::metadata("volumes").await.is_ok() {
            fs::create_dir("volumes").await?;
        }

        let container_name = format!("waitress-{}", id);

        if !fs::metadata(format!("volumes/{}", container_name))
            .await
            .is_ok()
        {
            fs::create_dir(format!("volumes/{}", container_name)).await?;
        }

        let volume_create = CreateVolumeOptions {
            name: &container_name,
            driver: &"local".to_string(),
            driver_opts: Default::default(),
            labels: Default::default(),
        };

        docker.create_volume(volume_create).await?;

        let script = format!(
            "JAR_URL={}\n{}",
            server_info.url,
            include_str!("../../provision_docker.sh")
        );

        // write script to file
        fs::write(format!("volumes/{}/provision.sh", container_name), script).await?;

        let cmd = vec!["sh", "-c", "cd /data && sh provision.sh"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let abs_path = fs::canonicalize(format!("volumes/{}", container_name)).await?;
        let abs_path = abs_path.to_str().ok_or_else(|| {
            ServerProvisionError::FilesystemError(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid path",
            ))
        })?;

        let host_config = HostConfig {
            binds: Some(vec![format!("{}/:/data", abs_path)]),
            ..Default::default()
        };

        let image = image.clone();

        let container_config = container::Config {
            image: Some(image),
            cmd: Some(cmd),
            volumes: Some({
                let mut map = HashMap::new();
                map.insert("/data".to_string(), HashMap::new());
                map
            }),
            host_config: Some(host_config),
            ..Default::default()
        };

        // create container
        docker
            .create_container(
                Some(CreateContainerOptions {
                    name: &container_name,
                    platform: None,
                }),
                container_config,
            )
            .await?;

        // run container
        docker
            .start_container(&container_name, None::<StartContainerOptions<String>>)
            .await?;

        // forever read stdout
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
            let stdout = String::from_utf8_lossy(&bytes);
            print!("{}", stdout);
        }

        Ok(())
    }
}
