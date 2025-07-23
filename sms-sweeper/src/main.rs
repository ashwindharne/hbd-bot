use chrono::Utc;
use common::{init_database, surge::SurgeClient, update_user_last_digest_at};
use log::{error, info};
use sms_sweeper::get_birthday_messages;
use std::env;
use tokio::time::{sleep, Duration};

#[derive(Clone)]
struct Config {
    database_url: String,
    surge_api_key: String,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        Ok(Config {
            database_url: env::var("DATABASE_URL")
                .map_err(|_| "DATABASE_URL environment variable not set")?,
            surge_api_key: env::var("SURGE_API_KEY")
                .map_err(|_| "SURGE_API_KEY environment variable not set")?,
        })
    }
}

async fn process_birthday_reminders(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting birthday reminder processing");

    // Get current UTC time
    let current_utc = Utc::now();
    let db = init_database(&config.database_url).await?;
    // Get messages to send
    let messages = get_birthday_messages(&db, current_utc).await?;

    if messages.is_empty() {
        info!("No birthday messages to send");
        return Ok(());
    }

    // Initialize Surge client
    let client = SurgeClient::new(config.surge_api_key.clone());

    let mut total_sms_sent = 0;

    info!("Using Surge client for sending SMS");
    for (i, sms_message) in messages.iter().enumerate() {
        // Rate limit: sleep for 1 second between messages (except for the first one)
        if i > 0 {
            sleep(Duration::from_secs(1)).await;
        }

        // Send SMS
        match send_sms_to_user(&client, &sms_message.phone_number, &sms_message.message).await {
            Ok(_) => {
                info!("Successfully sent SMS to {}", sms_message.phone_number);
                update_user_last_digest_at(&db, sms_message.user_id).await?;
                total_sms_sent += 1;
            }
            Err(e) => {
                error!("Failed to send SMS to {}: {}", sms_message.phone_number, e);
            }
        }
    }

    info!(
        "Birthday reminder processing complete. Sent {} SMS messages",
        total_sms_sent
    );
    Ok(())
}

async fn send_sms_to_user(
    client: &SurgeClient,
    phone_number: &str,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Send SMS
    match client
        .send_sms(format!("+1{}", phone_number), message.to_string())
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("SMS send failed: {}", e).into()),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    info!("Starting SMS Sweeper");

    // Parse configuration
    let config = Config::from_env().map_err(|e| {
        error!("Failed to parse environment variables: {}", e);
        e
    })?;

    // Process birthday reminders
    if let Err(e) = process_birthday_reminders(&config).await {
        error!("Failed to process birthday reminders: {}", e);
        return Err(e);
    }

    info!("SMS Sweeper completed successfully");
    Ok(())
}
