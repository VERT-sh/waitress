mod config;
mod db;
mod tests;
mod version;
mod web;

use actix_cors::Cors;
use actix_web::{middleware::Logger, web::Data, App, HttpServer};
use bollard::Docker;
use config::CONFIG;
use db::{server::Server, Database};
use dotenvy::dotenv;
use sqlx::PgPool;

async fn restore_servers(pool: &PgPool) -> anyhow::Result<()> {
    let servers = sqlx::query_as!(Server, "SELECT * FROM servers",)
        .fetch_all(pool)
        .await?;

    let docker = Docker::connect_with_local_defaults()?;

    for server in servers {
        server.restore_container(&docker).await?;
    }

    Ok(())
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("waitress"));
    let pool = PgPool::connect(&CONFIG.database_url).await?;
    restore_servers(&pool).await?;
    let db = Database::new(pool);
    log::info!("waitress is listening on ::9090!");
    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(db.clone()))
            .configure(web::configure)
            .wrap(
                Cors::permissive()
                    .allow_any_header()
                    .allow_any_method()
                    .allow_any_origin(),
            )
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 9090))?
    .run()
    .await?;
    Ok(())
}
