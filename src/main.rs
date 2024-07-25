mod helpers;
mod commands;
use helpers::{config, database::{db_create_tables, setup_database}, AppState};
use poise::{serenity_prelude as serenity, CreateReply};
use dotenv::dotenv;
use tracing::log::error;


#[tokio::main]
async fn main() {
    env_logger::init();
    dotenv().ok();
    let config = config::Config::init();
    let app_state = AppState { 
        env: config.clone(),
        db: setup_database(&config).await
    };

    // Initialize tables
    db_create_tables(&app_state.db).await.unwrap();

    let intents = serenity::GatewayIntents::non_privileged();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![commands::paw()],
            on_error: |error| {
                Box::pin(async move {
                    let ctx = error.ctx();
                    match ctx {
                        Some(ctx) => {
                            let _ = ctx.send(CreateReply::default()
                                        .content("An error occured. Please try again later")
                                        .ephemeral(true)
                                    ).await;
                        }
                        None => {

                        }
                    }
                    
                    error!("An error has occured: {:?}", error);
                })
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(app_state)
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(&config.discord_token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}