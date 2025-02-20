mod config;
mod db;
mod server;
mod tests;
mod version;
mod web;

use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use config::CONFIG;
use db::Database;
use dotenvy::dotenv;
use sqlx::PgPool;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("waitress"));
    let pool = PgPool::connect(&CONFIG.database_url).await?;
    let db = Database::new(pool);
    log::info!("waitress is listening on ::9090!");
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(db.clone()))
            .configure(web::configure)
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 9090))?
    .run()
    .await?;
    Ok(())
}
