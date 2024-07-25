use dotenv::dotenv;
use std::env;

fn get_env_var(var_name: &str) -> String {
    dotenv().ok();
    env::var(var_name).unwrap_or_else(|_| "{} is not set".to_owned())
}

#[derive(Debug, Clone)] 
pub struct Config {
    pub database_url: String,
    pub discord_token: String,
}

impl Config {
    pub fn init() -> Config {
        Config {
            database_url: get_env_var("DATABASE_URL"),
            discord_token: get_env_var("DISCORD_TOKEN")
        }
    }
}