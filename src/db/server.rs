use bollard::{
    container::{
        self, CreateContainerOptions, InspectContainerOptions, RemoveContainerOptions,
        StartContainerOptions,
    },
    image::CreateImageOptions,
    secret::{HostConfig, PortBinding},
    volume::{CreateVolumeOptions, RemoveVolumeOptions},
    Docker,
};
use chrono::{DateTime, Utc};
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::{collections::HashMap, env};
use thiserror::Error;
use tokio::fs;
use uuid::Uuid;

use crate::{
    response_codes,
    version::server::{ServerError, ServerJarInfo},
};

#[derive(FromRow, PartialEq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub owner: Uuid,
    pub port: i32,
    pub name: String,
    pub docker_image: String,
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
    #[error("Failed to get path")]
    PathError,
    #[error("Failed to start server")]
    StartError(#[from] ServerStartError),
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}

response_codes!(ServerProvisionError {
    DockerError(INTERNAL_SERVER_ERROR),
    VersionError(INTERNAL_SERVER_ERROR),
    VersionNotFound(NOT_FOUND),
    ServerInfoError(INTERNAL_SERVER_ERROR),
    FilesystemError(INTERNAL_SERVER_ERROR),
    PathError(INTERNAL_SERVER_ERROR),
    StartError(INTERNAL_SERVER_ERROR),
    DatabaseError(INTERNAL_SERVER_ERROR),
});

#[derive(Debug, Error)]
pub enum ServerCreationError {
    #[error("Database error: {0}")]
    ServerAlreadyExists(#[from] sqlx::Error),
    #[error("Provision error: {0}")]
    ProvisionError(#[from] ServerProvisionError),
    #[error("Port already allocated")]
    PortAlreadyAllocated,
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Server error: {0}")]
    ServerError(#[from] ServerError),
}

#[derive(Debug, Error)]
pub enum ServerDeletionError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
    #[error("Docker error: {0}")]
    DockerError(#[from] bollard::errors::Error),
    #[error("Server not found")]
    ServerNotFound,
}

response_codes!(ServerDeletionError {
    DatabaseError(INTERNAL_SERVER_ERROR),
    DockerError(INTERNAL_SERVER_ERROR),
    ServerNotFound(NOT_FOUND),
});

#[derive(Debug, Error)]
pub enum ServerStartError {
    #[error("Docker error: {0}")]
    DockerError(#[from] bollard::errors::Error),
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
        name: String,
        port: u16,
        version: impl Into<String>,
        pool: &PgPool,
    ) -> Result<Self, ServerCreationError> {
        // see if a server with this port allocated already exists
        let exists = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM servers WHERE port = $1)",
            port as i32
        )
        .fetch_one(pool)
        .await?
        .exists;
        if exists == Some(true) {
            return Err(ServerCreationError::PortAlreadyAllocated);
        }

        let manifest = crate::version::manifest::VersionManifest::new().await?;
        let version = manifest
            .get_version(version.into())
            .ok_or(ServerProvisionError::VersionNotFound)?;
        let server_info = version.get_server_info().await?;

        let server = sqlx::query_as!(
            Server,
            "INSERT INTO servers (owner, name, port, docker_image) VALUES ($1, $2, $3, $4) RETURNING *",
            owner,
            name,
            port as i32,
            format!("openjdk:{}", server_info.java_version)
        )
        .fetch_one(pool)
        .await?;

        if let Err(e) = server.provision(&server_info, port).await {
            sqlx::query!("DELETE FROM servers WHERE id = $1", server.id)
                .execute(pool)
                .await?;
            return Err(ServerCreationError::ProvisionError(e));
        }

        Ok(server)
    }

    async fn provision(
        &self,
        server_info: &ServerJarInfo,
        port: u16,
    ) -> Result<(), ServerProvisionError> {
        let docker = Docker::connect_with_local_defaults()?;

        match self
            .create_container(&docker, Some(&server_info), port)
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                log::error!("failed to provision server: {}", e);
                Err(e)
            }
        }
    }

    async fn create_container(
        &self,
        docker: &Docker,
        server_info: Option<&crate::version::server::ServerJarInfo>,
        port: u16,
    ) -> Result<(), ServerProvisionError> {
        let image = if let Some(server_info) = server_info {
            format!("openjdk:{}", server_info.java_version)
        } else {
            self.docker_image.clone()
        };

        let create_image_options = CreateImageOptions {
            from_image: image.as_str(),
            ..Default::default()
        };

        let mut stream = docker.create_image(Some(create_image_options), None, None);
        while let Some(event) = stream.next().await {
            let event = event?;
            let Some(status) = event.status else {
                continue;
            };
            log::info!("{}: {}", self.id, status)
        }

        if !fs::metadata("volumes").await.is_ok() {
            fs::create_dir("volumes").await?;
        }

        let container_name = self.container_name();

        let volume_path = self.volume_path();
        let volume_path = volume_path.as_str();

        if !fs::metadata(volume_path).await.is_ok() {
            fs::create_dir(volume_path).await?;
        }

        if let Some(server_info) = server_info {
            let volume_create = CreateVolumeOptions {
                name: &container_name,
                driver: &"local".to_string(),
                driver_opts: Default::default(),
                labels: Default::default(),
            };

            docker.create_volume(volume_create).await?;

            let script = include_str!("../../provision_docker.sh").replace("\r\n", "\n");

            let script = format!("JAR_URL={}\n{}", server_info.url, script);

            fs::write(format!("{}/provision.sh", volume_path), script).await?;
        }

        let cmd = vec!["sh", "-c", "cd /data && sh provision.sh"]
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let mut abs_path = fs::canonicalize(volume_path)
            .await?
            .to_str()
            .ok_or(ServerProvisionError::PathError)?
            .to_string();

        if env::consts::OS == "windows" {
            abs_path = abs_path
                .strip_prefix("\\\\?\\")
                .ok_or(ServerProvisionError::PathError)?
                .to_string();
        }

        let host_config = HostConfig {
            binds: Some(vec![format!("{}/:/data", abs_path)]),
            // todo: figure this out
            port_bindings: Some({
                let mut map = HashMap::new();
                map.insert(
                    "25565/tcp".to_string(),
                    Some(vec![PortBinding {
                        host_ip: Some("127.0.0.1".to_string()),
                        host_port: Some(port.to_string()),
                    }]),
                );
                map
            }),
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
            exposed_ports: Some({
                let mut map = HashMap::new();
                map.insert("25565/tcp".to_string(), HashMap::new());
                map
            }),
            host_config: Some(host_config),
            open_stdin: Some(true),
            ..Default::default()
        };

        docker
            .create_container(
                Some(CreateContainerOptions {
                    name: &container_name,
                    platform: None,
                }),
                container_config,
            )
            .await?;

        self.start().await?;

        Ok(())
    }

    pub async fn delete(self, pool: &PgPool) -> Result<(), ServerDeletionError> {
        sqlx::query!("DELETE FROM servers WHERE id = $1", self.id)
            .execute(pool)
            .await?;
        // delete the docker container if it exists
        let docker = Docker::connect_with_local_defaults()?;
        let container_name = self.container_name();
        if let Ok(_) = docker
            .inspect_container(&container_name, None::<InspectContainerOptions>)
            .await
        {
            Self::force_remove(self.id).await?;
            // delete the volume
            docker
                .remove_volume(&container_name, Some(RemoveVolumeOptions { force: true }))
                .await?;
        }
        Ok(())
    }

    pub fn container_name(&self) -> String {
        format!("waitress-{}", self.id)
    }

    pub async fn force_remove(id: Uuid) -> Result<(), ServerDeletionError> {
        let docker = Docker::connect_with_local_defaults()?;
        docker
            .remove_container(
                &format!("waitress-{}", id),
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await?;
        Ok(())
    }

    pub async fn get_all(owner: Uuid, pool: &PgPool) -> Result<Vec<Self>, sqlx::Error> {
        sqlx::query_as!(Server, "SELECT * FROM servers WHERE owner = $1", owner)
            .fetch_all(pool)
            .await
    }

    pub async fn restore_container(
        &self,
        docker: &Docker,
        pool: &PgPool,
    ) -> Result<(), ServerProvisionError> {
        // docker containers are ephemeral by nature
        // this function fixes this by restoring the container
        // (assuming the volume is still present)

        let container_name = self.container_name();

        if !fs::metadata(self.volume_path()).await.is_ok() {
            log::warn!("no volume for {}, giving up on it :(", container_name);

            // delete the server from the database
            sqlx::query!("DELETE FROM servers WHERE id = $1", self.id)
                .execute(pool)
                .await?;

            docker
                .remove_container(
                    &container_name,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await
                .ok();

            return Ok(());
        }

        if let Ok(_) = docker
            .inspect_container(&container_name, None::<InspectContainerOptions>)
            .await
        {
            log::info!("container {} already exists", container_name);
            if let Err(e) = self.start().await {
                log::error!("failed to start container {}: {}", container_name, e);
            }
            return Ok(()); // container already exists
        }

        log::info!("restoring container {}", container_name);

        self.create_container(docker, None, self.port as u16)
            .await?;
        self.start().await?;

        Ok(())
    }

    pub async fn start(&self) -> Result<(), ServerStartError> {
        let docker = Docker::connect_with_local_defaults()?;
        docker
            .start_container(
                &self.container_name(),
                None::<StartContainerOptions<String>>,
            )
            .await?;
        Ok(())
    }

    pub fn volume_path(&self) -> String {
        format!("volumes/{}", self.container_name())
    }
}
