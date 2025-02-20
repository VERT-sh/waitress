use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Server info not found")]
    ServerInfoNotFound,
    #[error("Failed to download server: {0}")]
    DownloadFailed(#[from] reqwest::Error),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    pub url: String,
}

impl Version {
    pub async fn get_server_info(&self) -> Result<ServerJarInfo, ServerError> {
        let info = ServerVersionInfo::new(&self.url).await?;
        let server_info = info
            .downloads
            .server
            .ok_or(ServerError::ServerInfoNotFound)?;
        let java_version = info.java_version.major_version;
        Ok(ServerJarInfo {
            url: server_info.url,
            java_version,
        })
    }
}

#[derive(Debug)]
pub struct ServerJarInfo {
    pub url: String,
    pub java_version: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerDownloadInfo {
    pub url: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerVersionInfo {
    downloads: Downloads,
    java_version: JavaVersion,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JavaVersion {
    major_version: u8,
}

impl ServerVersionInfo {
    pub async fn new(url: &str) -> Result<Self, ServerError> {
        let server_info: ServerVersionInfo = reqwest::get(url).await?.json().await?;
        Ok(server_info)
    }
}

#[derive(Debug, Deserialize)]
struct Downloads {
    server: Option<ServerDownloadInfo>,
}
