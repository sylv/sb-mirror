use std::env;

use actix_cors::Cors;
use actix_web::{http::header, middleware, web, App, HttpServer};
use anyhow::Result;
use download::download_http;
use r2d2_sqlite::SqliteConnectionManager;
use routes::segments_by_hash;
use rusqlite::OpenFlags;
use rusqlite_migration::{Migrations, M};
use sync::sync;
use tokio::signal;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

mod download;
mod routes;
mod segment;
mod sync;

pub type Pool = r2d2::Pool<SqliteConnectionManager>;
pub type Connection = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let migrations = Migrations::new(vec![
        M::up(
            r#"
            CREATE TABLE segments (
                id TEXT NOT NULL UNIQUE,
                video_id TEXT NOT NULL,
                hash_full TEXT NOT NULL,
                hash_prefix TEXT GENERATED ALWAYS AS (substring(hash_full, 1, 4)) STORED,
                start_time INTEGER NOT NULL,
                end_time INTEGER NOT NULL,
                category TEXT NOT NULL,
                user_id TEXT NOT NULL,
                votes INTEGER NOT NULL,
                service TEXT NOT NULL,
                action_type TEXT NOT NULL,
                locked INTEGER NOT NULL,
                video_duration INTEGER NOT NULL
            );

            CREATE TABLE imports (
                total_size INTEGER NOT NULL,
                etag TEXT,
                imported_at INTEGER NOT NULL,
                header TEXT
            );

            CREATE INDEX segments_video_id ON segments (video_id);
            CREATE INDEX segments_hash_prefix ON segments (hash_prefix);
        "#,
        ),
        M::up(r#"CREATE INDEX segments_start_time ON segments (start_time);"#),
    ]);

    let download_dir = env::var("DATA_PATH").unwrap_or_else(|_| "/data".to_string());
    let path = format!("{}/sponsorTimes.db", download_dir);
    let manager = SqliteConnectionManager::file(path)
        .with_flags(OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE)
        .with_init(|conn| {
            rusqlite::vtab::array::load_module(conn)?;
            conn.execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA cache_size = 1000000;
                PRAGMA synchronous = normal;
            "#,
            )?;

            Ok(())
        });

    let pool = r2d2::Pool::new(manager).unwrap();

    info!("Running migrations");
    let mut conn = pool.get().expect("could not get connection");
    migrations.to_latest(&mut conn).unwrap();

    {
        let (kill_tx, mut kill_rx) = tokio::sync::oneshot::channel::<()>();
        let pool = pool.clone();
        tokio::spawn(async move {
            let duration = env::var("SYNC_INTERVAL").unwrap_or_else(|_| "300".to_string());
            let duration = std::time::Duration::from_secs(duration.parse().unwrap());
            loop {
                let csv_path = download_http().await.expect("failed to download csv");
                sync(csv_path, &pool, &mut kill_rx).expect("failed to sync");

                info!("syncing again in {} seconds", duration.as_secs());
                tokio::time::sleep(duration).await;
            }
        });

        // terrifying, isnt it?
        // tokio cant kill the sync task because sync() is synchronous, so this channel lets us
        // tell it to fuck off when the process is killed. sync() checks for messages on every csv entry and exits when it gets one.
        // todo: this is all a terrible hacky workaround, should probably just use async everywhere but rusqlite doesnt support async that great.
        tokio::spawn(async move {
            signal::ctrl_c().await.expect("failed to listen for ctrl-c");
            debug!("shutting down sync thread");
            kill_tx.send(()).expect("failed to send kill signal");
        });
    }

    info!("Starting server");
    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_method()
            .allow_any_origin()
            .allowed_header(header::CONTENT_TYPE)
            .max_age(3600);

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(pool.clone()))
            .service(segments_by_hash)
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
    .expect("could not start server");

    Ok(())
}
