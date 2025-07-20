use reqwest;
use serde::{Deserialize, Serialize};

const API_BASE_URL: &str = "https://api.twilio.com/2010-04-01";

#[derive(Debug)]
pub struct TwilioError {
    pub message: String,
}

impl std::fmt::Display for TwilioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Twilio Error: {}", self.message)
    }
}

impl std::error::Error for TwilioError {}

pub struct TwilioClient {
    client: reqwest::Client,
    account_sid: String,
    auth_token: String,
    from_phone_number: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TwilioSendSMSData {
    pub sid: String,
    pub status: String,
    pub to: String,
    pub from: String,
    pub body: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl TwilioClient {
    pub fn new(account_sid: String, auth_token: String, from_phone_number: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            account_sid,
            auth_token,
            from_phone_number,
        }
    }

    pub async fn send_sms(
        &self,
        to_phone_number: String,
        message: String,
    ) -> Result<TwilioSendSMSData, TwilioError> {
        let url = format!(
            "{}/Accounts/{}/Messages.json",
            API_BASE_URL, self.account_sid
        );

        let form_data = [
            ("To", to_phone_number.as_str()),
            ("From", self.from_phone_number.as_str()),
            ("Body", message.as_str()),
        ];

        let response = self
            .client
            .post(&url)
            .basic_auth(&self.account_sid, Some(&self.auth_token))
            .form(&form_data)
            .send()
            .await
            .map_err(|e| TwilioError {
                message: format!("Failed to send SMS: {}", e),
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| TwilioError {
            message: format!("Failed to read response text: {}", e),
        })?;

        if !status.is_success() {
            return Err(TwilioError {
                message: format!(
                    "Twilio API returned error status: {}. Raw response: '{}'",
                    status, response_text
                ),
            });
        }

        let send_sms_response =
            serde_json::from_str::<TwilioSendSMSData>(&response_text).map_err(|e| TwilioError {
                message: format!(
                    "Failed to parse send SMS response: {}. Raw response: {}",
                    e, response_text
                ),
            })?;

        Ok(send_sms_response)
    }
}
