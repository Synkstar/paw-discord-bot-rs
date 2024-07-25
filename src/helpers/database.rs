use sqlx::PgPool;
use super::{config::Config, types::MyDuration};
use super::structs::*;
use chrono::{DateTime, Duration, NaiveDate, Utc};
type Error = Box<dyn std::error::Error + Send + Sync>;

pub async fn setup_database(config: &Config) -> PgPool {
    PgPool::connect(&config.database_url).await.expect("Failed to connect to Postgres")
}

pub async fn db_create_tables(pool: &PgPool) -> Result<(), Error> {
    let queries = vec![
        r#"CREATE SCHEMA IF NOT EXISTS "paw-bot" AUTHORIZATION postgres;"#,
        r#"
            CREATE TABLE IF NOT EXISTS "paw-bot".paw_count (
                user_id int8 NOT NULL,
                server_id int8 NOT NULL,
                count int8 NOT NULL,
                CONSTRAINT paw_count_client_id_server_id_key UNIQUE (user_id, server_id),
                CONSTRAINT paw_count_count_check CHECK ((count >= 0))
            );
        "#,
        r#"
            CREATE TABLE IF NOT EXISTS "paw-bot".server_settings (
                server_id int8 NOT NULL,
                steal_interval interval NOT NULL,
                gamble_interval interval NOT NULL,
                steal_chance int4 NOT NULL,
                gamble_chance int4 NULL,
                CONSTRAINT server_settings_gamble_chance_check CHECK (((gamble_chance >= 0) AND (gamble_chance <= 100))),
                CONSTRAINT server_settings_server_id_key UNIQUE (server_id),
                CONSTRAINT server_settings_steal_chance_check CHECK (((steal_chance >= 0) AND (steal_chance <= 100)))
            );
        "#,
        r#"
            CREATE TABLE IF NOT EXISTS "paw-bot".user_limits (
                user_id int8 NOT NULL,
                server_id int8 NOT NULL,
                last_steal timestamptz NULL,
                last_daily timestamptz NULL,
                last_gamble timestamptz NULL,
                CONSTRAINT user_limits_user_id_server_id_key UNIQUE (user_id, server_id)
            );     
    "#];

    let mut transaction = pool.begin().await?;
 
    for query in queries {
        sqlx::query(query).execute(&mut *transaction).await?;
    }   

    transaction.commit().await?;

    Ok(())
}

pub async fn db_get_paw_count(pool: &PgPool, user_id: u64, server_id: u64) -> Result<u64,Error> {
    let result = sqlx::query_as::<_,(i64,)>("SELECT count FROM \"paw-bot\".\"paw_count\" WHERE user_id = $1 AND server_id = $2")
        .bind(user_id as i64)
        .bind(server_id as i64)
        .fetch_one(pool)
        .await;
    
    match result {
        Ok((count,)) => Ok(count as u64),
        Err(e) if matches!(e, sqlx::Error::RowNotFound) => Ok(0), // Handle RowNotFound specifically
        Err(e) => Err(Box::new(e)), // Propagate other errors
    }
}

pub async fn db_get_rank(pool: &PgPool, user_id: u64, server_id: u64) -> Result<u64,Error> {
    let result = sqlx::query_scalar::<_, i64>(
        "SELECT rank::BIGINT FROM (
            SELECT user_id, server_id, RANK() OVER (ORDER BY count DESC) AS rank
            FROM \"paw-bot\".\"paw_count\"
        ) subquery
        WHERE user_id = $1 AND server_id = $2"
    )
    .bind(user_id as i64)
    .bind(server_id as i64)
    .fetch_one(pool)
    .await;

    match result {
        Ok(rank) => Ok((rank as u64) + 1),
        Err(e) if matches!(e, sqlx::Error::RowNotFound) => Ok(0), // Handle RowNotFound specifically
        Err(e) => Err(Box::new(e)), // Propagate other errors
    }
}

pub async fn db_update_paw_count(pool: &PgPool, user_id: u64, server_id: u64, difference: i64) -> Result<u64,Error> {
    let query = r#"
        INSERT INTO "paw-bot"."paw_count" (user_id, server_id, count)
        VALUES ($1, $2, 1)
        ON CONFLICT (user_id, server_id)
        DO UPDATE SET count = "paw-bot"."paw_count".count + $3
        RETURNING count;
    "#;

    let count = sqlx::query_scalar::<_,i64>(&query)
        .bind(user_id as i64)
        .bind(server_id as i64)
        .bind(difference)
        .fetch_one(pool)
        .await?;

    Ok(count as u64)
}

pub async fn db_get_server_settings(pool: &PgPool, server_id: u64) -> Result<ServerSettings, Error> {
    let server_settings = sqlx::query_as::<_,ServerSettings>("select steal_interval, gamble_interval FROM \"paw-bot\".\"server_settings\" WHERE server_id = $1")
        .bind(server_id as i64)
        .fetch_one(pool)
        .await;

    Ok(server_settings.unwrap_or(ServerSettings {
        steal_interval: MyDuration(Duration::minutes(0)), // default to no delay
        gamble_interval: MyDuration(Duration::minutes(10)), // default to 10 minutes
        steal_chance: 50, // default to 50%
        gamble_chance: 50 // default to 50%
    }))
}

pub async fn db_get_last_daily(pool: &PgPool, user_id: u64, server_id: u64) -> Result<DateTime<Utc>, Error> {
    // Collect the date time from the database
    let result = sqlx::query_scalar::<_, DateTime<Utc>>(
        "select last_daily from \"paw-bot\".\"user_limits\" WHERE user_id = $1 AND server_id = $2"
    )
    .bind(user_id as i64)
    .bind(server_id as i64)
    .fetch_one(pool)
    .await;

    // Return the date time if it exists and a time way before the current if it doesn't
    match result {
        Ok(last_daily) => Ok(last_daily),
        Err(e) if matches!(e, sqlx::Error::RowNotFound) => {
            let naive_date = NaiveDate::from_ymd_opt(2004, 04, 11).unwrap();
            let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap(); // Convert to NaiveDateTime
            Ok(DateTime::from_naive_utc_and_offset(naive_datetime,Utc))
        }
        Err(e) => Err(Box::new(e)), // Propagate other errors
    }
}

pub async fn db_get_last_steal(pool: &PgPool, user_id: u64, server_id: u64) -> Result<DateTime<Utc>, Error> {
    // Collect the date time from the database
    let result: Result<DateTime<Utc>, sqlx::Error> = sqlx::query_scalar::<_, DateTime<Utc>>(
        "select last_steal from \"paw-bot\".\"user_limits\" WHERE user_id = $1 AND server_id = $2"
    )
    .bind(user_id as i64)
    .bind(server_id as i64)
    .fetch_one(pool)
    .await;

    // Return the date time if it exists and a time way before the current if it doesn't
    match result {
        Ok(last_daily) => Ok(last_daily),
        Err(e) if matches!(e, sqlx::Error::RowNotFound) => {
            let naive_date = NaiveDate::from_ymd_opt(1990, 1, 1).unwrap();
            let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
            Ok(DateTime::from_naive_utc_and_offset(naive_datetime,Utc))
        }
        Err(e) => Err(Box::new(e)), // Propagate other errors
    }
}

pub async fn db_get_last_gamble(pool: &PgPool, user_id: u64, server_id: u64) -> Result<DateTime<Utc>, Error> {
    // Collect the time from the database
    let result = sqlx::query_scalar::<_, DateTime<Utc>>(
        "select last_gamble from \"paw-bot\".\"user_limits\" WHERE user_id = $1 AND server_id = $2"
    )
    .bind(user_id as i64)
    .bind(server_id as i64)
    .fetch_one(pool)
    .await;

    // Return the date time if it exists and a time way before the current if it doesn't
    match result {
        Ok(last_daily) => Ok(last_daily),
        Err(e) if (matches!(e, sqlx::Error::RowNotFound) || matches!(e, sqlx::Error::ColumnDecode { .. })) => { 
            let naive_date = NaiveDate::from_ymd_opt(1990, 1, 1).unwrap();
            let naive_datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
            Ok(DateTime::from_naive_utc_and_offset(naive_datetime,Utc))
        }
        Err(e) => Err(Box::new(e)), // Propagate other errors
    }
}

pub async fn db_update_last_steal(pool: &PgPool, user_id: u64, server_id: u64, time: DateTime<Utc>) -> Result<DateTime<Utc>,Error> {
    let query = r#"
        INSERT INTO "paw-bot"."user_limits" (user_id, server_id, last_steal, last_daily, last_gamble)
        VALUES ($1, $2, $3, NULL, NULL)
        ON CONFLICT (user_id, server_id)
        DO UPDATE SET last_steal = $3
        RETURNING last_steal;
    "#;

    let time = sqlx::query_scalar::<_,DateTime<Utc>>(&query)
        .bind(user_id as i64)
        .bind(server_id as i64)
        .bind(time)
        .fetch_one(pool)
        .await?;

    Ok(time)
}

pub async fn db_update_last_daily(pool: &PgPool, user_id: u64, server_id: u64, time: DateTime<Utc>) -> Result<DateTime<Utc>,Error> {
    let query = r#"
        INSERT INTO "paw-bot"."user_limits" (user_id, server_id, last_daily, last_steal, last_gamble)
        VALUES ($1, $2, $3, NULL, NULL)
        ON CONFLICT (user_id, server_id)
        DO UPDATE SET last_daily = $3
        RETURNING last_daily;
    "#;

    let time = sqlx::query_scalar::<_,DateTime<Utc>>(&query)
        .bind(user_id as i64)
        .bind(server_id as i64)
        .bind(time)
        .fetch_one(pool)
        .await?;

    Ok(time)
}

pub async fn db_update_last_gamble(pool: &PgPool, user_id: u64, server_id: u64, time: DateTime<Utc>) -> Result<DateTime<Utc>,Error> {
    let query = r#"
        INSERT INTO "paw-bot"."user_limits" (user_id, server_id, last_gamble, last_daily, last_steal)
        VALUES ($1, $2, $3, NULL, NULL)
        ON CONFLICT (user_id, server_id)
        DO UPDATE SET last_gamble = $3
        RETURNING last_gamble;
    "#;

    let time = sqlx::query_scalar::<_,DateTime<Utc>>(&query)
        .bind(user_id as i64)
        .bind(server_id as i64)
        .bind(time)
        .fetch_one(pool)
        .await?;

    Ok(time)
}

pub async fn db_get_leaderboard(pool: &PgPool, server_id: u64, page: &u8) ->Result<(Vec<PawCount>, u64, u64),Error> {
    // Convert page number to 0 index and multiply by 10 to offset database query
    let offset = ((*page as u64) - 1) * 10;

    // Fetch a max of 10 paw count and user_id pairs from the database 
    let leaderboard = sqlx::query_as::<_,PawCount>("SELECT count, user_id::BIGINT FROM \"paw-bot\".\"paw_count\" WHERE server_id = $1 ORDER BY count DESC LIMIT 10 OFFSET $2")
        .bind(server_id as i64)
        .bind(offset as i32)
        .fetch_all(pool)
        .await?;

    // Get the total number of farmers and their paw counts from the database
    let (farmers, total_paws) = sqlx::query_as::<_,(i64,i64)>("SELECT COUNT(*), SUM(count)::BIGINT FROM \"paw-bot\".\"paw_count\" WHERE server_id = $1")
        .bind(server_id as i64)
        .fetch_one(pool)
        .await?;

    let combined = (leaderboard, farmers as u64, total_paws as u64);

    Ok(combined)
}