use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use uuid::Uuid;

#[derive(FromRow, PartialEq, Debug)]
pub struct Server {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub owner: Uuid,
    pub port: i32,
    pub name: String,
}

#[derive(Debug, Error)]
pub enum ServerCreationError {
    #[error("An error occurred while creating the server: {0}")]
    ServerAlreadyExists(#[from] sqlx::Error),
}

impl Server {
    pub async fn from_id(id: Uuid, pool: &PgPool) -> Option<Self> {
        sqlx::query_as!(Server, "SELECT * FROM servers WHERE id = $1", id)
            .fetch_one(pool)
            .await
            .ok()
    }

    pub async fn create(owner: Uuid, pool: &PgPool) -> Result<Self, ServerCreationError> {
        let server = sqlx::query_as!(
            Server,
            "INSERT INTO servers (owner) VALUES ($1) RETURNING *",
            owner
        )
        .fetch_one(pool)
        .await?;

        Ok(server)
    }
}
