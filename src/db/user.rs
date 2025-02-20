use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash,
};
use argon2::{PasswordHasher, PasswordVerifier};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use thiserror::Error;
use uuid::Uuid;

use crate::config::CONFIG;

#[derive(Error, Debug)]
pub enum UserCreationError {
    #[error("Failed to hash password: {0}")]
    HashError(argon2::password_hash::Error),
    #[error("User already exists")]
    UserAlreadyExists,
    #[error("Threading error")]
    ThreadError,
}

#[derive(Error, Debug)]
pub enum AuthCreationError {
    #[error("Failed to create auth token: {0}")]
    JWTError(jsonwebtoken::errors::Error),
}

#[derive(Error, Debug)]
pub enum AuthDecodeError {
    #[error("Failed to decode auth token: {0}")]
    JWTError(jsonwebtoken::errors::Error),
    #[error("User does not exist")]
    NonexistentUser,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: Uuid,
    exp: usize,
}

#[derive(FromRow, PartialEq, Debug, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub async fn from_id(id: Uuid, pool: &PgPool) -> Option<Self> {
        sqlx::query_as!(
            User,
            "SELECT id, username, created_at FROM users WHERE id = $1",
            id
        )
        .fetch_one(pool)
        .await
        .ok()
    }

    pub async fn create(
        username: impl Into<String>,
        password: impl Into<String>,
        pool: &PgPool,
    ) -> Result<Self, UserCreationError> {
        let username: String = username.into();
        let password: String = password.into();
        let hash = tokio::task::spawn_blocking(move || {
            let hasher = Argon2::default();
            let salt = SaltString::generate(&mut OsRng);
            let hash = hasher
                .hash_password(password.as_bytes(), &salt)
                .map_err(UserCreationError::HashError)?;
            let hash = hash.to_string();
            Ok(hash)
        })
        .await
        .map_err(|_| UserCreationError::ThreadError)??;

        let user = sqlx::query_as!(
            User,
            "INSERT INTO users (username, password) VALUES ($1, $2) RETURNING id, username, created_at",
            username,
            hash.to_string()
        )
        .fetch_one(pool)
        .await.map_err(|_| UserCreationError::UserAlreadyExists)?;

        Ok(user)
    }

    pub async fn verify_password(&self, password: impl Into<String>, pool: &PgPool) -> bool {
        let password_to_verify = password.into();
        let hasher = Argon2::default();
        let Ok(true_password) = sqlx::query!("SELECT password FROM users WHERE id = $1", self.id)
            .fetch_one(pool)
            .await
        else {
            return false;
        };

        let Ok(hash) = PasswordHash::new(&true_password.password) else {
            return false;
        };

        hasher
            .verify_password(password_to_verify.as_bytes(), &hash)
            .is_ok()
    }

    pub async fn create_token(&self) -> Result<String, AuthCreationError> {
        jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &Claims {
                sub: self.id,
                exp: (Utc::now() + Duration::days(365)).timestamp() as usize,
            },
            &jsonwebtoken::EncodingKey::from_secret(&CONFIG.jwt_secret),
        )
        .map_err(|e| AuthCreationError::JWTError(e))
    }

    pub async fn from_auth(
        token: impl Into<String>,
        pool: &PgPool,
    ) -> Result<Self, AuthDecodeError> {
        let claims: Claims = jsonwebtoken::decode(
            &token.into(),
            &jsonwebtoken::DecodingKey::from_secret(&CONFIG.jwt_secret),
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|e| AuthDecodeError::JWTError(e))?
        .claims;

        User::from_id(claims.sub, pool)
            .await
            .ok_or_else(|| AuthDecodeError::NonexistentUser)
    }

    pub async fn from_username(username: impl Into<String>, pool: &PgPool) -> Option<Self> {
        let username = username.into();
        sqlx::query_as!(
            User,
            "SELECT id, username, created_at FROM users WHERE username = $1",
            username
        )
        .fetch_one(pool)
        .await
        .ok()
    }
}
