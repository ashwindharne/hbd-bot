use reqwest;
use serde::{Deserialize, Serialize};

const API_BASE_URL: &str = "https://cpaas.messagecentral.com";

#[derive(Debug)]
pub struct MessageCentralError {
    pub message: String,
}

impl std::fmt::Display for MessageCentralError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MessageCentral Error: {}", self.message)
    }
}

impl std::error::Error for MessageCentralError {}

pub struct MessageCentralClient {
    client: reqwest::Client,
    customer_id: String,
    email: String,
    password_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessageCentralTokenResponse {
    status: i32,
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageCentralSendOTPResponse {
    response_code: i32,
    message: String,
    data: MessageCentralSendOTPData,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageCentralSendOTPData {
    pub verification_id: String,
    pub mobile_number: String,
    pub response_code: String,
    pub error_message: Option<String>,
    pub timeout: String,  // Changed from i32 to String since API returns "60.0"
    pub transaction_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageCentralVerifyOTPResponse {
    response_code: i32,
    message: String,
    data: MessageCentralVerifyOTPData,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MessageCentralVerifyOTPData {
    verification_id: i64,  // Changed from String to i64 since API returns integer
    mobile_number: String,
    response_code: String,
    verification_status: String,
    error_message: Option<String>,  // Added missing field
    auth_token: Option<String>,
    transaction_id: String,
}

impl MessageCentralClient {
    pub fn new(customer_id: String, email: String, password_b64: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            customer_id,
            email,
            password_b64,
        }
    }

    pub async fn generate_token(&self) -> Result<String, MessageCentralError> {
        let url = format!("{}/auth/v1/authentication/token", API_BASE_URL);
        let query_params = [
            ("customerId", self.customer_id.as_str()),
            ("key", self.password_b64.as_str()),
            ("scope", "NEW"),
            ("country", "1"),
            ("email", self.email.as_str()),
        ];

        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .header("accept", "*/*")
            .send()
            .await
            .map_err(|e| MessageCentralError {
                message: format!("Failed to generate token: {}", e),
            })?;

        let response_text = response.text().await.map_err(|e| MessageCentralError {
            message: format!("Failed to read response text: {}", e),
        })?;

        let token_response = serde_json::from_str::<MessageCentralTokenResponse>(&response_text)
            .map_err(|e| MessageCentralError {
                message: format!(
                    "Failed to parse token response: {}. Raw response: {}",
                    e, response_text
                ),
            })?;

        Ok(token_response.token)
    }

    pub async fn send_otp(
        &self,
        token: String,
        country_code: String,
        otp_length: i32,
        phone_number: String,
    ) -> Result<MessageCentralSendOTPData, MessageCentralError> {
        let url = format!("{}/verification/v3/send", API_BASE_URL);
        let otp_length_str = otp_length.to_string();
        let query_params = [
            ("countryCode", country_code.as_str()),
            ("otpLength", otp_length_str.as_str()),
            ("mobileNumber", phone_number.as_str()),
            ("flowType", "SMS"),
        ];
        
        let response = self
            .client
            .post(&url)
            .query(&query_params)
            .header("authToken", token)
            .send()
            .await
            .map_err(|e| MessageCentralError {
                message: format!("Failed to send OTP: {}", e),
            })?;

        let response_text = response.text().await.map_err(|e| MessageCentralError {
            message: format!("Failed to read response text: {}", e),
        })?;

        let send_otp_response = serde_json::from_str::<MessageCentralSendOTPResponse>(&response_text)
            .map_err(|e| MessageCentralError {
                message: format!("Failed to parse send OTP response: {}. Raw response: {}", e, response_text),
            })?;
        Ok(send_otp_response.data)
    }

    pub async fn verify_otp(
        &self,
        auth_token: String,
        verification_id: String,
        code: String,
    ) -> Result<(), MessageCentralError> {
        let url = format!("{}/verification/v3/validateOtp", API_BASE_URL);
        let query_params = [
            ("verificationId", verification_id.as_str()),
            ("code", code.as_str()),
        ];
        
        let response = self
            .client
            .get(&url)
            .query(&query_params)
            .header("authToken", auth_token)
            .send()
            .await
            .map_err(|e| MessageCentralError {
                message: format!("Failed to verify OTP: {}", e),
            })?;

        let response_text = response.text().await.map_err(|e| MessageCentralError {
            message: format!("Failed to read response text: {}", e),
        })?;

        let verify_otp_response = serde_json::from_str::<MessageCentralVerifyOTPResponse>(&response_text)
            .map_err(|e| MessageCentralError {
                message: format!("Failed to parse verify OTP response: {}. Raw response: {}", e, response_text),
            })?;

        if verify_otp_response.data.verification_status == "VERIFICATION_COMPLETED" {
            Ok(())
        } else {
            Err(MessageCentralError {
                message: "OTP verification failed".to_string(),
            })
        }
    }
}
