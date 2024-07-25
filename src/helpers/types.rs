use chrono::Duration;
use sqlx::prelude::FromRow;
use sqlx::{Decode, Type, Postgres};
use sqlx::postgres::{PgValueRef, PgTypeInfo};

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct MyDuration(pub Duration);

impl MyDuration {
    pub fn duration(&self) -> Duration {
        self.0
    }
}

impl<'r> Decode<'r, Postgres> for MyDuration {
    fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let interval_str = value.as_str()?;
        let duration = parse_postgres_interval(interval_str)?;
        Ok(MyDuration(duration))
    }
}

impl Type<Postgres> for MyDuration {
    fn type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("interval")
    }
}

fn parse_postgres_interval(interval: &str) -> Result<Duration, Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = interval.split_whitespace().collect();
    let mut duration = Duration::zero();

    for chunk in parts.chunks(2) {
        let value: i64 = chunk[0].parse()?;
        match chunk[1] {
            "day" | "days" => duration = duration + Duration::days(value),
            "hour" | "hours" => duration = duration + Duration::hours(value),
            "minute" | "minutes" => duration = duration + Duration::minutes(value),
            "second" | "seconds" => duration = duration + Duration::seconds(value),
            _ => return Err(format!("Unsupported interval part: {}", chunk[1]).into()),
        }
    }

    Ok(duration)
}
