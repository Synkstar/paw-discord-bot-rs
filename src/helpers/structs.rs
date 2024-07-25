use chrono::NaiveDateTime;
use sqlx::prelude::FromRow;
use super::types::MyDuration;

#[derive(Debug, FromRow)]
pub struct ServerSettings {
    pub steal_interval: MyDuration,
    pub gamble_interval: MyDuration,
    pub gamble_chance: i8,
    pub steal_chance: i8
}

#[derive(Debug, FromRow)]
pub struct UserLimits {
    pub last_steal: NaiveDateTime,
    pub last_daily: NaiveDateTime,
    pub last_gamble: NaiveDateTime
}

#[derive(FromRow)]
pub struct PawCount {
    pub count: i64,
    pub user_id: i64,
}