use lazy_static::lazy_static;

lazy_static! {
    #[derive(Debug)]
    pub static ref CONFIG: Config = Config::new().unwrap();
}

pub struct Config {
    pub signups_enabled: bool,
    pub database_url: String,
    pub jwt_secret: Vec<u8>,
}

impl Config {
    pub fn new() -> anyhow::Result<Self> {
        let signups_enabled = std::env::var("SIGNUPS_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()?;
        let database_url = std::env::var("DATABASE_URL")?;
        let jwt_secret = std::env::var("JWT_SECRET")?.into_bytes();
        Ok(Self {
            signups_enabled,
            database_url,
            jwt_secret,
        })
    }
}
