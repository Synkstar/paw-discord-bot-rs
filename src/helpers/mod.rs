use config::Config;
use sqlx;
pub mod config;
pub mod database;
pub mod structs;
pub mod types;

#[derive(Debug)]
pub struct AppState {
    pub env: Config,
    pub db: sqlx::PgPool
}