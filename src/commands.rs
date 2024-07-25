use crate::helpers::{database::*, AppState};
use chrono::{Duration,Utc};
use poise::serenity_prelude as serenity;
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, AppState, Error>;
use ::serenity::all::Mentionable;
use serenity::builder::CreateEmbed;
use poise::reply::CreateReply;
use serenity::model::id::UserId;
use rand::{thread_rng, Rng};


fn get_guild_id(ctx: Context<'_>) -> Result<u64, Error> {
    let id = match ctx.guild_id() {
        Some(guild_id) => guild_id.get(), // GuildId has a .0 field which is the u64 representation
        None => return Err(Error::from("Guild ID not found")), // Handle the error case as needed
    };
    Ok(id)  
}

// User readable formatting for time between durations
fn format_time_left(to: Duration, current: Duration) -> String {
    let difference = to - current;

    if difference > Duration::days(1) {
        let days = difference.num_days();
        let day_word = if days != 1 {"days"} else {"day"};

        return format!("{} {}",difference.num_days(), day_word);
    } else if difference > Duration::hours(1) {
        let hours = difference.num_days();
        let hour_word = if hours != 1 {"hours"} else {"hour"};

        return format!("{} {}", difference.num_hours(), hour_word);
    } else if difference > Duration::minutes(1){
        let minutes = difference.num_days();
        let minute_word = if minutes != 1 {"minutes"} else {"minute"};
        
        return format!("{} {}", difference.num_minutes(), minute_word);
    }

    let seconds = difference.num_seconds();
    let second_word  = if seconds != 1 {"seconds"} else {"second"};
    
    return format!("{} {}", difference.num_seconds(), second_word);
}

#[poise::command(prefix_command, slash_command, subcommands("balance","daily","steal","top","gamble","give"))]
pub async fn paw(_ctx: Context<'_>) -> Result<(), Error> {
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US","Claims your daily paw drop"), prefix_command)]
pub async fn daily(
    ctx: Context<'_>,
) -> Result<(), Error> {
    let user_id = ctx.author().id.get();
    let guild_id = get_guild_id(ctx)?;
    let last_claimed = db_get_last_daily(&ctx.data().db, user_id, guild_id).await?;
    let now = Utc::now();
    let duration_since_last_claimed = now.signed_duration_since(last_claimed);

    // Users can only collect paws daily
    if duration_since_last_claimed < Duration::days(1) {
        ctx.send(CreateReply::default()
            .content(format!("You already claimed your daily paw! (Wait {})", format_time_left(Duration::days(1), duration_since_last_claimed)))
            .ephemeral(true)
        ).await?;
        return Ok(());
    }
    
    // Give new paw to the User
    let _ = db_update_last_daily(&ctx.data().db, user_id, guild_id, now).await?;
    let paw_count = db_update_paw_count(&ctx.data().db, user_id, guild_id,1).await?;
    ctx.reply(format!("You claimed your daily paw, and now hold onto {} paws!",paw_count)).await?;

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US","Donate paws to others"), prefix_command)]
pub async fn give(
    ctx: Context<'_>,
    who: serenity::User,
    count: u32
) -> Result<(), Error> {
    // Users cannot target themselves
    if who == *ctx.author() {
        ctx.send(CreateReply::default()
            .content("You cannot donate to yourself")
            .ephemeral(true)
        ).await?;
        return Ok(());
    }

    let caller_id = ctx.author().id.get();
    let target_id = who.id.get();
    let guild_id = get_guild_id(ctx)?;
    let paw_count = db_get_paw_count(&ctx.data().db, caller_id, guild_id).await?;

    // Make sure the user doesn't give paws they don't have
    if paw_count < (count as u64) {
        ctx.send(CreateReply::default()
            .content("You can only give as many paws as you have!")
            .ephemeral(true)
        ).await?;
        return Ok(());
    }

    // Update paw counts in the database
    db_update_paw_count(&ctx.data().db, caller_id, guild_id, (count as i64) * -1).await?;
    db_update_paw_count(&ctx.data().db, target_id, guild_id, count as i64).await?;

    let paw_word = if count != 1 {"paws"} else {"paw"};
    ctx.reply(format!("You gave {} {} to {}, how nice of you!",count,paw_word,who.mention())).await?;

    Ok(())
}


#[poise::command(slash_command, description_localized("en-US","Test your odds"), prefix_command)]
pub async fn gamble(
    ctx: Context<'_>,
    stake: u8
) -> Result<(), Error> {
    let user_id = ctx.author().id.get();
    let guild_id = get_guild_id(ctx)?;
    let last_gambled = db_get_last_gamble(&ctx.data().db, user_id, guild_id).await?;
    let server_settings = db_get_server_settings(&ctx.data().db, guild_id).await?;
    let now = Utc::now();
    let duration_since_last_gambled = now.signed_duration_since(last_gambled);

    // Limit how often a user can gamble
    if duration_since_last_gambled < server_settings.gamble_interval.duration() {
        ctx.send(CreateReply::default()
            .content("🚫 🐶 gambling addiction is a serious problem. Regulations require a wait. Try again later...")
            .ephemeral(true)
        ).await?;
        return Ok(());
    }

    // Users can only gamble as many paws as they have
    let paw_count = db_get_paw_count(&ctx.data().db, user_id, guild_id).await?;
    if paw_count < (stake as u64) || stake > 10 {
        ctx.send(CreateReply::default()
            .content("You can only gamble as many paws as you have! (up to 10)")
            .ephemeral(true)
        ).await?;
        return Ok(());
    }

    // Set the last time they have gambled
    db_update_last_gamble(&ctx.data().db, user_id, guild_id, now).await?;

    // Will be true if the random number generator feels like it
    let chance = {
        let mut rng = thread_rng();
        rng.gen_ratio(server_settings.gamble_chance as u32, 100)
    };

    let stake_paw_word = if stake != 1 {"paws"} else {"paw"};

    if chance {
        let new_paws = db_update_paw_count(&ctx.data().db, user_id, guild_id, stake as i64).await?;
        let new_paw_word = if new_paws != 1 {"paws"} else {"paw"};

        let mut description = format!("Your gambling paid off, you won {} {}, giving you a total of {} {}.", stake, stake_paw_word, new_paws, new_paw_word).to_string();
        let dogs = "🐶".repeat(std::cmp::min(396,new_paws as usize));
        description.push_str(&dogs);
        description.push_str("📈");

        let embed = CreateEmbed::new() 
            .title("🎲 🐶 🎲")
            .description(description);

        ctx.send(CreateReply::default()
            .embed(embed)).await?;
    } else {
        let new_paws = db_update_paw_count(&ctx.data().db, user_id, guild_id, (stake as i64) * -1).await?;
        let new_paw_word = if new_paws != 1 {"paws"} else {"paw"};

        let mut description = format!("Your gambling sucked, you lost {} {}, giving you a total of {} {}.", stake, stake_paw_word, new_paws, new_paw_word).to_string();
        let dogs = "🐶".repeat(std::cmp::min(396,new_paws as usize));
        description.push_str(&dogs);
        description.push_str("📉");

        let embed = CreateEmbed::new() 
            .title("🎲 🐶 🎲")
            .description(description);

        ctx.send(CreateReply::default()
            .embed(embed)).await?;
    }

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US","tells you how many paws you have"), prefix_command)]
pub async fn balance(
    ctx: Context<'_>,
    #[description = "(optional) member to check the balance of"] 
    who: Option<serenity::User>
) -> Result<(), Error> {
    let target = if let Some(target) = who {
        target
    } else {
        ctx.author().clone()
    };

    let user_id = target.id.get();
    let guild_id = get_guild_id(ctx)?;
    let paw_count = db_get_paw_count(&ctx.data().db, user_id, guild_id).await?;
    let avatar_url = match target.avatar_url() {
        Some(avatar_url) => avatar_url,
        None => "".to_string()
    };

    let embed = CreateEmbed::new() 
        .title("🐶 paw count 🐶")
        .description(format!("{} has {} paws.",target.mention(),paw_count))
        .thumbnail(avatar_url);
    
    ctx.send(CreateReply::default()
        .embed(embed)
    ).await?;
    
    Ok(())
}

#[poise::command(slash_command, description_localized("en-US","If you're lucky, you might be able to steal some."), prefix_command)]
pub async fn steal(
    ctx: Context<'_>,
    who: serenity::User,
    count: u64
) -> Result<(), Error> {
    if who == *ctx.author() {
        ctx.send(CreateReply::default()
            .content("You cannot steal from yourself")
            .ephemeral(true)
        ).await?;

        return Ok(());
    }

    let caller_user_id = ctx.author().id.get();
    let target_user_id = who.id.get();
    let guild_id = get_guild_id(ctx)?;

    let caller_paw_count = db_get_paw_count(&ctx.data().db, caller_user_id, guild_id).await?;
    if caller_paw_count < count {
        ctx.send(CreateReply::default()
            .content("You can only steal as many paws as you have!")
            .ephemeral(true)
        ).await?;

        return Ok(());
    }

    let target_paw_count = db_get_paw_count(&ctx.data().db, target_user_id, guild_id).await?;
    if target_paw_count < count {
        ctx.send(CreateReply::default()
            .content("That user doesnt have that many paws!")
            .ephemeral(true)
        ).await?;

        return Ok(());
    }

    let server_settings = db_get_server_settings(&ctx.data().db, guild_id).await?;
    let last_stole = db_get_last_steal(&ctx.data().db, caller_user_id, guild_id).await?;
    let now = Utc::now();
    let duration_since_last_stole = now.signed_duration_since(last_stole);
    
    // Limit how often someone can steal
    if duration_since_last_stole < server_settings.steal_interval.duration() {
        ctx.send(CreateReply::default()
            .content("🚫 🐶 stealing addiction is a serious problem. Regulations require a wait. Try again later...")
            .ephemeral(true)
        ).await?;
        return Ok(());
    }
    
    let now = Utc::now();
    let _ = db_update_last_steal(&ctx.data().db, caller_user_id, guild_id, now).await?;

    // Will be true if the random number generator feels like it
    let chance = {
        let mut rng = thread_rng();
        rng.gen_ratio(server_settings.gamble_chance as u32, 100)
    };

    let count_paw_word = if count != 1 {"paws"} else {"paw"};

    if chance {
        let new_paws = db_update_paw_count(&ctx.data().db, caller_user_id, guild_id, count as i64).await?;
        let _ = db_update_paw_count(&ctx.data().db, target_user_id, guild_id, (count as i64) * -1).await?;
        let new_paw_word = if new_paws != 1 {"paws"} else {"paw"};

        let mut description = format!("Your thievery paid off, you stole {} {} from {}, giving you a total of {} {}.", count, count_paw_word, who.mention(), new_paws, new_paw_word).to_string();
        let dogs = "🐶".repeat(std::cmp::min(396,new_paws as usize));
        description.push_str(&dogs);
        description.push_str("📈");

        let embed = CreateEmbed::new() 
            .title("🧤 🐶 🧤")
            .description(description);

        ctx.send(CreateReply::default()
            .embed(embed)
        ).await?;
    } else {
        let new_paws = db_update_paw_count(&ctx.data().db, caller_user_id, guild_id, (count as i64) * -1).await?;
        db_update_paw_count(&ctx.data().db, target_user_id, guild_id, count as i64).await?;

        let new_paw_word = if new_paws != 1 {"paws"} else {"paw"};
        let mut description = format!("Your thievery sucked, you gave {} {} to {}, giving you a total of {} {}.", count, count_paw_word, who.mention(), new_paws, new_paw_word).to_string();
        let dogs = "🐶".repeat(std::cmp::min(396,new_paws as usize));
        description.push_str(&dogs);
        description.push_str("📉");

        let embed = CreateEmbed::new() 
            .title("🧤 🐶 🧤")
            .description(description);

        ctx.send(CreateReply::default()
            .embed(embed)
        ).await?;
    }

    Ok(())
}

#[poise::command(slash_command, description_localized("en-US","Take a gander at the paw leaderboard"), prefix_command)]
pub async fn top(
    ctx: Context<'_>,
    #[description = "(optional) page number"] 
    page: Option<u8>
) -> Result<(), Error> {
    let page = if let Some (page) = page {
        page
    } else {
        1
    };

    let user_id = ctx.author().id.get();
    let guild_id = get_guild_id(ctx)?;
    let (leaderboard, farmers, total_paws) = db_get_leaderboard(&ctx.data().db, guild_id, &page).await?;
    let caller_pawcount = db_get_paw_count(&ctx.data().db, user_id, guild_id).await?;
    let caller_rank = db_get_rank(&ctx.data().db, user_id, guild_id).await?;

    // Top of embed content
    let mut description = "".to_string();
    description.push_str(&format!("🐶 {}\n",total_paws));
    description.push_str(&format!("👨‍🌾 {}\n\n",farmers));
    description.push_str("📈 Ranks 💪\n");

    // Handle no content on page
    if leaderboard.len() < 1  {
        description.push_str("Page contains no farmers 🌵");
     
        let embed = CreateEmbed::new()
            .title("🏆 Leaderboard​ 👑")
            .description(description);

        ctx.send(CreateReply::default()
            .embed(embed)
        ).await?;

        return Ok(());
    }

    // Add users to the list
    for (index,farmer) in leaderboard.iter().enumerate() {
        let position = ((index as u64) + 1) * (page as u64);

        // Top 3 get special medals
        match position {
            1 => description.push_str("`` 🥇 ``"),
            2 => description.push_str("`` 🥈 ``"),
            3 => description.push_str("`` 🥉 ``"),
            _ => description.push_str(&format!("`` {} ``",position))
        }

        // Get username from user id
        match ctx.http().get_user(UserId::new(farmer.user_id as u64)).await {
            Ok(user) => {
                description.push_str(&user.name);
            }
            Err(_) => {
                description.push_str(&farmer.user_id.to_string())
            }
        }

        let paw_word = if farmer.count != 1 {"paws"} else {"paw"};
        description.push_str(&format!(" - {} {}\n",farmer.count,paw_word));
    }

    description.push_str(&format!("``...`` {} other farmers\n",farmers));

    let paw_word = if caller_pawcount != 1 {"paws"} else {"paw"};
    description.push_str(&format!("`` {} `` {} - {} {}",caller_rank,ctx.author().name, caller_pawcount, paw_word));

    let embed = CreateEmbed::new()
        .title("🏆 Leaderboard​ 👑")
        .description(description);

    ctx.send(CreateReply::default()
        .embed(embed)
    ).await?;

    Ok(())
}