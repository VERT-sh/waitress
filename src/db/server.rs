use bollard::{
    container::{self, AttachContainerOptions, CreateContainerOptions, StartContainerOptions},
    image::CreateImageOptions,
    secret::HostConfig,
    volume::CreateVolumeOptions,
    Docker,
};
use chrono::{DateTime, Utc};
use futures::StreamExt as _;
use sqlx::{FromRow, PgPool};
use std::collections::HashMap;
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

use crate::{error_variants, version::server::ServerError};

#[derive(FromRow, PartialEq, Debug)]
pub struct Server {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub owner: Uuid,
    pub port: i32,
    pub name: String,
}

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

error_variants!(ServerProvisionError {
    DockerError(INTERNAL_SERVER_ERROR),
    VersionError(INTERNAL_SERVER_ERROR),
    VersionNotFound(NOT_FOUND),
    ServerInfoError(INTERNAL_SERVER_ERROR),
    FilesystemError(INTERNAL_SERVER_ERROR)
});

#[derive(Debug, Error)]
pub enum ServerCreationError {
    #[error("Database error: {0}")]
    ServerAlreadyExists(#[from] sqlx::Error),
    #[error("Provision error: {0}")]
    ProvisionError(#[from] ServerProvisionError),
}

impl Server {
    pub async fn from_id(id: Uuid, pool: &PgPool) -> Option<Self> {
        sqlx::query_as!(Server, "SELECT * FROM servers WHERE id = $1", id)
            .fetch_one(pool)
            .await
            .ok()
    }

    pub async fn create(
        owner: Uuid,
        version: impl Into<String>,
        pool: &PgPool,
    ) -> Result<Self, ServerCreationError> {
        let server = sqlx::query_as!(
            Server,
            "INSERT INTO servers (owner) VALUES ($1) RETURNING *",
            owner
        )
        .fetch_one(pool)
        .await?;

        if let Err(e) = server.provision(version).await {
            sqlx::query!("DELETE FROM servers WHERE id = $1", server.id)
                .execute(pool)
                .await?;
            return Err(ServerCreationError::ProvisionError(e));
        }

        Ok(server)
    }

    async fn provision(&self, version: impl Into<String>) -> Result<(), ServerProvisionError> {
        let docker = Docker::connect_with_local_defaults()?;

        let manifest = crate::version::manifest::VersionManifest::new().await?;
        let version = manifest
            .get_version(version.into())
            .ok_or(ServerProvisionError::VersionNotFound)?;
        let server_info = version.get_server_info().await?;

        self.create_container(&docker, &server_info).await
    }

    async fn create_container(
        &self,
        docker: &Docker,
        server_info: &crate::version::server::ServerJarInfo,
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

        let container_name = format!("waitress-{}", self.id);

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
