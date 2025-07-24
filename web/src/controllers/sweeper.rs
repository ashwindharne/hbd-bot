use axum::{extract::State, http::StatusCode};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use chrono::Utc;
use common::{surge::SurgeClient, update_user_last_digest_at};
use log::{error, info};
use sms_sweeper::get_birthday_messages;
use tokio::time::{sleep, Duration};

use super::auth::AppState;

pub async fn run_sweeper_handler(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    State(state): State<AppState>,
) -> StatusCode {
    let token = auth.token();
    if token != state.config.admin_token {
        error!("Unauthorized access attempt");
        return StatusCode::UNAUTHORIZED;
    }
    info!("Starting birthday reminder processing via API");
    let current_utc = Utc::now();
    let messages = match get_birthday_messages(&state.db, current_utc).await {
        Ok(messages) => messages,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };

    let client = SurgeClient::new(state.config.surge_api_key.clone());
    let mut total_sms_sent = 0;

    for (i, sms_message) in messages.iter().enumerate() {
        if i > 0 {
            sleep(Duration::from_secs(1)).await;
            info!("Sending SMS to {}", sms_message.phone_number);
            match client
                .send_sms(
                    format!("+1{}", &sms_message.phone_number),
                    sms_message.message.clone(),
                )
                .await
            {
                Ok(_) => {
                    info!("Successfully sent SMS to {}", sms_message.phone_number);
                    total_sms_sent += 1;
                    match update_user_last_digest_at(&state.db, sms_message.user_id).await {
                        Ok(_) => {
                            info!("Successfully updated user last digest at");
                        }
                        Err(e) => {
                            error!("Failed to update user last digest at: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to send SMS to {}: {}", sms_message.phone_number, e);
                }
            }
        }
    }
    info!("Total SMS sent: {}", total_sms_sent);
    StatusCode::OK
}
