#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use crate::db::user::User;

    #[sqlx::test]
    async fn user_creation(pool: PgPool) {
        let username = "user_creation";
        let password = "test_password";
        let user = User::create(username, password, &pool).await.unwrap();
        assert_eq!(user.username, username);
    }

    #[sqlx::test]
    async fn password_verification(pool: PgPool) {
        let username = "password_verification";
        let password = "test_password";
        let user = User::create(username, password, &pool).await.unwrap();
        assert!(user.verify_password(password, &pool).await);
        assert!(!user.verify_password("wrong_password", &pool).await);
    }

    #[sqlx::test]
    async fn unique_usernames(pool: PgPool) {
        let username = "unique_usernames";
        let password = "test_password";
        let _ = User::create(username, password, &pool).await.unwrap();
        let result = User::create(username, password, &pool).await;
        assert!(result.is_err());
    }

    #[sqlx::test]
    async fn user_by_id(pool: PgPool) {
        let username = "user_by_id";
        let password = "test_password";
        let user = User::create(username, password, &pool).await.unwrap();
        let user_by_id = User::from_id(user.id, &pool).await.unwrap();
        assert_eq!(user, user_by_id);

        let user_by_id = User::from_id(uuid::Uuid::new_v4(), &pool).await;
        assert!(user_by_id.is_none());
    }

    #[sqlx::test]
    async fn user_jwt(pool: PgPool) {
        let username = "user_jwt";
        let password = "test_password";
        let user = User::create(username, password, &pool).await.unwrap();
        let token = user.create_token().await.unwrap();
        let user_from_auth = User::from_token(token, &pool).await.unwrap();
        assert_eq!(user, user_from_auth);
    }
}
