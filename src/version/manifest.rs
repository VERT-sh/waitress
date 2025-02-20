use super::server::Version;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct VersionManifest {
    pub versions: Vec<Version>,
}

impl VersionManifest {
    pub async fn new() -> Result<Self, reqwest::Error> {
        const URL: &str = "https://launchermeta.mojang.com/mc/game/version_manifest.json";
        let manifest = reqwest::get(URL).await?.json().await?;
        Ok(manifest)
    }

    pub fn get_version(&self, version: impl Into<String>) -> Option<&Version> {
        let version = version.into();
        self.versions.iter().find(|v| v.id == version.trim())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}
