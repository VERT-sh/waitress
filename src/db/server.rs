use bollard::{
    container::{
        self, CreateContainerOptions, InspectContainerOptions, RemoveContainerOptions,
        StartContainerOptions,
    },
    image::CreateImageOptions,
    secret::{HostConfig, PortBinding},
    volume::CreateVolumeOptions,
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

use crate::{error_variants, version::server::ServerError};

#[derive(FromRow, PartialEq, Debug, Serialize, Deserialize)]
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
    #[error("Failed to get path")]
    PathError,
}

error_variants!(ServerProvisionError {
    DockerError(INTERNAL_SERVER_ERROR),
    VersionError(INTERNAL_SERVER_ERROR),
    VersionNotFound(NOT_FOUND),
    ServerInfoError(INTERNAL_SERVER_ERROR),
    FilesystemError(INTERNAL_SERVER_ERROR),
    PathError(INTERNAL_SERVER_ERROR),
});

#[derive(Debug, Error)]
pub enum ServerCreationError {
    #[error("Database error: {0}")]
    ServerAlreadyExists(#[from] sqlx::Error),
    #[error("Provision error: {0}")]
    ProvisionError(#[from] ServerProvisionError),
    #[error("Port already allocated")]
    PortAlreadyAllocated,
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

error_variants!(ServerDeletionError {
    DatabaseError(INTERNAL_SERVER_ERROR),
    DockerError(INTERNAL_SERVER_ERROR),
    ServerNotFound(NOT_FOUND),
});

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
        let server = sqlx::query_as!(
            Server,
            "INSERT INTO servers (owner, name, port) VALUES ($1, $2, $3) RETURNING *",
            owner,
            name,
            port as i32
        )
        .fetch_one(pool)
        .await?;

        if let Err(e) = server.provision(version, port).await {
            sqlx::query!("DELETE FROM servers WHERE id = $1", server.id)
                .execute(pool)
                .await?;
            return Err(ServerCreationError::ProvisionError(e));
        }

        Ok(server)
    }

    async fn provision(
        &self,
        version: impl Into<String>,
        port: u16,
    ) -> Result<(), ServerProvisionError> {
        let docker = Docker::connect_with_local_defaults()?;

        let manifest = crate::version::manifest::VersionManifest::new().await?;
        let version = manifest
            .get_version(version.into())
            .ok_or(ServerProvisionError::VersionNotFound)?;
        let server_info = version.get_server_info().await?;

        match self.create_container(&docker, &server_info, port).await {
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
        server_info: &crate::version::server::ServerJarInfo,
        port: u16,
    ) -> Result<(), ServerProvisionError> {
        let image = format!("openjdk:{}", server_info.java_version);

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

        let mut abs_path = fs::canonicalize(format!("volumes/{}", container_name))
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

        docker
            .start_container(&container_name, None::<StartContainerOptions<String>>)
            .await?;

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
}
