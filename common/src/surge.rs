use log::info;
use reqwest;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct SurgeError {
    pub message: String,
}

impl std::fmt::Display for SurgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Surge Error: {}", self.message)
    }
}

impl std::error::Error for SurgeError {}

pub struct SurgeClient {
    client: reqwest::Client,
    api_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageRequest1 {
    /// The recipient's phone number in E.164 format
    pub to: String,

    /// Message body
    pub body: String,
}

impl SurgeClient {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }

    pub async fn send_sms(&self, to: String, body: String) -> Result<(), SurgeError> {
        // Validate required field
        if to.is_empty() {
            return Err(SurgeError {
                message: "Phone number 'to' is required".to_string(),
            });
        }

        // Validate E.164 format for 'to' field
        if !to.starts_with('+') {
            return Err(SurgeError {
                message: "Phone number 'to' must be in E.164 format (starting with +)".to_string(),
            });
        }

        let request = MessageRequest1 {
            to: to.clone(),
            body: body.clone(),
        };

        info!("Sending SMS to: {}", to);
        info!("Message body: {}", body);

        let response = self
            .client
            .post("https://api.surge.app/accounts/acct_01k0g5thf6f1dtcr0v0ye8va2f/messages")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| SurgeError {
                message: format!("Failed to send SMS request: {}", e),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(SurgeError {
                message: format!(
                    "SMS API returned error status: {}. Response: {}",
                    status, error_text
                ),
            });
        }

        info!("SMS sent successfully");
        Ok(())
    }
}
