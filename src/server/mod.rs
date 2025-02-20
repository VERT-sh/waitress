// use std::collections::HashMap;

// use crate::version::{manifest::VersionManifest, server::ServerError};
// use bollard::{
//     container::{self, AttachContainerOptions, CreateContainerOptions, StartContainerOptions},
//     image::CreateImageOptions,
//     secret::HostConfig,
//     volume::CreateVolumeOptions,
//     Docker,
// };
// use futures::StreamExt as _;
// use thiserror::Error;
// use tokio::fs;
// use uuid::Uuid;

// #[derive(Error, Debug)]
// pub enum ServerProvisionError {
//     #[error("Docker error: {0}")]
//     DockerError(#[from] bollard::errors::Error),
//     #[error("Mojang API error: {0}")]
//     VersionError(#[from] reqwest::Error),
//     #[error("Version not found")]
//     VersionNotFound,
//     #[error("Server error: {0}")]
//     ServerInfoError(#[from] ServerError),
//     #[error("Filesystem error: {0}")]
//     FilesystemError(#[from] std::io::Error),
// }

// pub struct Server {
//     docker: Docker,
//     id: Uuid,
// }

// impl Server {
//     pub async fn provision(version: impl Into<String>) -> Result<Self, ServerProvisionError> {
//         let id = Uuid::new_v4();
//         let version = version.into();
//         let docker = Docker::connect_with_local_defaults()?;
//         let manifest = VersionManifest::new().await?;
//         let version = manifest
//             .get_version(version)
//             .ok_or(ServerProvisionError::VersionNotFound)?;
//         let server_info = version.get_server_info().await?;
//         Self::create_container(&docker, &server_info, &id).await?;
//         Ok(Self { docker, id })
//     }
// }
