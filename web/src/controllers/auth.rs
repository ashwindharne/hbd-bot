use askama::Template;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{Html, Json, Redirect},
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use common::{create_user, get_user_by_phone, message_central::MessageCentralClient};
use jsonwebtoken::{encode, decode, EncodingKey, DecodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::{SystemTime, UNIX_EPOCH};
use time;
use log::{error, info, warn};
use chrono::{Utc, Datelike, DateTime};
use aes_gcm::{Aes256Gcm, Key, Nonce, KeyInit, AeadInPlace};
use base64::{Engine as _, engine::general_purpose};
use common::DbReminder;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub config: crate::Config,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate;

#[derive(Template)]
#[template(path = "app.html")]
pub struct AppTemplate {
    pub phone_number: String,
    pub reminders: Vec<ReminderDisplay>,
}

#[derive(Debug, Clone)]
pub struct ReminderDisplay {
    pub id: i64,
    pub name: String,
    pub birthdate: String,
    pub days_until_birthday: i32,
    pub age_turning: i32,
}

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    phone: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    success: bool,
    message: String,
    validity_seconds: u32,
}

#[derive(Serialize, Deserialize)]
pub struct VerifyOtpRequest {
    phone: String,
    code: String,
}

#[derive(Serialize, Deserialize)]
pub struct VerifyOtpResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    user_id: i64,
    exp: usize,
}

fn encrypt_token(token: &str, secret: &str) -> Result<String, Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    
    // Create a 32-byte key from the JWT secret
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let key_bytes = hasher.finalize();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    
    let cipher = Aes256Gcm::new(key);
    
    // Generate a random nonce
    let nonce_bytes = [0u8; 12]; // AES-GCM standard nonce size
    let nonce = Nonce::from_slice(&nonce_bytes);
    
    let mut buffer = token.as_bytes().to_vec();
    cipher.encrypt_in_place(nonce, b"", &mut buffer)
        .map_err(|e| format!("Encryption failed: {}", e))?;
    
    // Combine nonce + ciphertext and base64 encode
    let mut result = nonce_bytes.to_vec();
    result.extend(buffer);
    
    Ok(general_purpose::STANDARD.encode(result))
}

fn decrypt_token(encrypted_token: &str, secret: &str) -> Result<String, Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    
    // Create a 32-byte key from the JWT secret
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    let key_bytes = hasher.finalize();
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    
    let cipher = Aes256Gcm::new(key);
    
    // Base64 decode
    let combined = general_purpose::STANDARD.decode(encrypted_token)?;
    
    if combined.len() < 12 {
        return Err("Invalid encrypted token".into());
    }
    
    // Split nonce and ciphertext
    let (nonce_bytes, mut ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let mut buffer = ciphertext.to_vec();
    
    cipher.decrypt_in_place(nonce, b"", &mut buffer)
        .map_err(|e| format!("Decryption failed: {}", e))?;
    
    Ok(String::from_utf8(buffer)?)
}

pub fn calculate_birthday_info(birthdate_timestamp: i64) -> Result<(i32, i32), String> {
    // Convert Unix timestamp (milliseconds) to UTC DateTime
    let birthdate_utc = DateTime::from_timestamp_millis(birthdate_timestamp)
        .ok_or("Invalid timestamp")?;
    let birthdate = birthdate_utc.naive_utc().date();
    
    let today = Utc::now().naive_utc().date();
    let current_year = today.year();
    
    // Calculate this year's birthday
    let this_year_birthday = birthdate.with_year(current_year);
    
    let (next_birthday, age_turning) = match this_year_birthday {
        Some(this_year_bd) => {
            if this_year_bd >= today {
                // Birthday hasn't happened this year yet
                (this_year_bd, current_year - birthdate.year())
            } else {
                // Birthday already happened this year, calculate for next year
                let next_year_birthday = birthdate.with_year(current_year + 1)
                    .ok_or("Failed to calculate next year's birthday")?;
                (next_year_birthday, current_year + 1 - birthdate.year())
            }
        }
        None => {
            // Handle leap year edge case (Feb 29 -> Feb 28)
            let next_year_birthday = birthdate.with_year(current_year + 1)
                .ok_or("Failed to calculate next year's birthday")?;
            (next_year_birthday, current_year + 1 - birthdate.year())
        }
    };
    
    let days_until = (next_birthday - today).num_days() as i32;
    
    Ok((days_until, age_turning))
}

pub fn convert_reminders_to_display(reminders: Vec<DbReminder>) -> Vec<ReminderDisplay> {
    reminders
        .into_iter()
        .filter_map(|reminder| {
            match calculate_birthday_info(reminder.birthdate) {
                Ok((days_until, age_turning)) => {
                    // Convert timestamp to formatted date string for display
                    let formatted_date = match DateTime::from_timestamp_millis(reminder.birthdate) {
                        Some(dt) => dt.naive_utc().date().format("%Y-%m-%d").to_string(),
                        None => "Unknown".to_string(),
                    };
                    
                    Some(ReminderDisplay {
                        id: reminder.id,
                        name: reminder.name,
                        birthdate: formatted_date,
                        days_until_birthday: days_until,
                        age_turning,
                    })
                },
                Err(e) => {
                    warn!("Failed to calculate birthday info for reminder {}: {}", reminder.id, e);
                    None
                }
            }
        })
        .collect()
}

pub fn verify_jwt_cookie(jar: &CookieJar, jwt_secret: &str) -> Result<i64, String> {
    let auth_cookie = jar.get("auth_token")
        .ok_or("No auth cookie found".to_string())?;
    
    let token = auth_cookie.value();
    
    let decoding_key = DecodingKey::from_secret(jwt_secret.as_ref());
    let validation = Validation::default();
    
    match decode::<Claims>(token, &decoding_key, &validation) {
        Ok(token_data) => {
            let claims = token_data.claims;
            
            // Check if token is expired
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as usize;
            
            if claims.exp < now {
                return Err("Token expired".to_string());
            }
            
            Ok(claims.user_id)
        },
        Err(e) => Err(format!("Invalid JWT: {}", e))
    }
}

pub async fn login(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>), StatusCode> {
    info!("Login request for phone: {}", payload.phone);

    if payload.phone.is_empty() {
        let response = LoginResponse {
            success: false,
            message: "Phone number is required".to_string(),
            validity_seconds: 0,
        };
        return Ok((jar, Json(response)));
    }

    // Initialize MessageCentral client using config from app state
    let client = MessageCentralClient::new(
        app_state.config.message_central_customer_id.clone(),
        app_state.config.message_central_email.clone(),
        app_state.config.message_central_password_b64.clone(),
    );

    // Generate token
    let token = match client.generate_token().await {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate MessageCentral token: {}", e);
            let response = LoginResponse {
                success: false,
                message: "Failed to send OTP. Please try again.".to_string(),
                validity_seconds: 0,
            };
            return Ok((jar, Json(response)));
        }
    };

    // Send OTP
    let otp_data = match client
        .send_otp(token.clone(), "1".to_string(), 6, payload.phone.clone())
        .await
    {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to send OTP: {}", e);
            let response = LoginResponse {
                success: false,
                message: "Failed to send OTP. Please try again.".to_string(),
                validity_seconds: 0,
            };
            return Ok((jar, Json(response)));
        }
    };

    // Encrypt the auth token before storing
    let encrypted_token = match encrypt_token(&token, &app_state.config.jwt_secret) {
        Ok(encrypted) => encrypted,
        Err(e) => {
            error!("Failed to encrypt auth token: {}", e);
            let response = LoginResponse {
                success: false,
                message: "Failed to send OTP. Please try again.".to_string(),
                validity_seconds: 0,
            };
            return Ok((jar, Json(response)));
        }
    };
    
    // Store verification ID and encrypted auth token in the cookie (JSON encoded)
    let verification_data = serde_json::json!({
        "verification_id": otp_data.verification_id,
        "auth_token": encrypted_token
    });
    
    let verification_cookie = Cookie::build(("otp_verification_data", verification_data.to_string()))
        .http_only(true)
        .secure(false)  // Set to false for development (non-HTTPS)
        .path("/")
        .build();

    info!("Setting verification cookie: {}", otp_data.verification_id);
    let jar = jar.add(verification_cookie);

    info!("Successfully sent OTP to phone {} with {} seconds validity", payload.phone, otp_data.timeout);

    let response = LoginResponse {
        success: true,
        message: "OTP sent successfully".to_string(),
        validity_seconds: otp_data.timeout.parse::<f64>().unwrap_or(600.0) as u32,
    };

    Ok((jar, Json(response)))
}

pub async fn verify_otp(
    State(app_state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<VerifyOtpRequest>,
) -> Result<(CookieJar, HeaderMap, Json<VerifyOtpResponse>), StatusCode> {
    info!(
        "OTP verification for phone: {}, code: {}",
        payload.phone, payload.code
    );

    let mut headers = HeaderMap::new();

    // Get verification data from cookie
    let (verification_id, auth_token) = match jar.get("otp_verification_data") {
        Some(cookie) => {
            match serde_json::from_str::<serde_json::Value>(cookie.value()) {
                Ok(data) => {
                    let verification_id = data["verification_id"].as_str().unwrap_or("").to_string();
                    let encrypted_token = data["auth_token"].as_str().unwrap_or("").to_string();
                    
                    // Decrypt the auth token
                    match decrypt_token(&encrypted_token, &app_state.config.jwt_secret) {
                        Ok(auth_token) => (verification_id, auth_token),
                        Err(e) => {
                            error!("Failed to decrypt auth token: {}", e);
                            let response = VerifyOtpResponse {
                                success: false,
                                message: "Verification session expired. Please try again.".to_string(),
                            };
                            return Ok((jar, headers, Json(response)));
                        }
                    }
                },
                Err(_) => {
                    warn!("Invalid verification cookie format for phone: {}", payload.phone);
                    let response = VerifyOtpResponse {
                        success: false,
                        message: "Verification session expired. Please try again.".to_string(),
                    };
                    return Ok((jar, headers, Json(response)));
                }
            }
        },
        None => {
            warn!("OTP verification attempted without valid session cookie for phone: {}", payload.phone);
            let response = VerifyOtpResponse {
                success: false,
                message: "Verification session expired. Please try again.".to_string(),
            };
            return Ok((jar, headers, Json(response)));
        }
    };

    // Clean up verification cookies now that we've read the data
    let remove_old_verification_cookie = Cookie::build(("otp_verification_id", ""))
        .http_only(true)
        .secure(false)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    
    let remove_verification_cookie = Cookie::build(("otp_verification_data", ""))
        .http_only(true)
        .secure(false)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    
    let jar = jar.add(remove_old_verification_cookie).add(remove_verification_cookie);

    // Initialize MessageCentral client using config from app state
    let client = MessageCentralClient::new(
        app_state.config.message_central_customer_id.clone(),
        app_state.config.message_central_email.clone(),
        app_state.config.message_central_password_b64.clone(),
    );

    // Verify OTP using the stored auth token
    if let Err(e) = client
        .verify_otp(auth_token, verification_id, payload.code)
        .await
    {
        warn!("OTP verification failed for phone {}: {}", payload.phone, e);
        let response = VerifyOtpResponse {
            success: false,
            message: "Invalid OTP code. Please try again.".to_string(),
        };
        return Ok((jar, headers, Json(response)));
    }

    // Find or create user
    let user = match get_user_by_phone(&app_state.db, &payload.phone).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            // Create new user
            match create_user(&app_state.db, &payload.phone).await {
                Ok(user) => user,
                Err(e) => {
                    error!("Failed to create user for phone {}: {}", payload.phone, e);
                    let response = VerifyOtpResponse {
                        success: false,
                        message: "Failed to create user account.".to_string(),
                    };
                    return Ok((jar, headers, Json(response)));
                }
            }
        }
        Err(e) => {
            error!("Database error when looking up user by phone {}: {}", payload.phone, e);
            let response = VerifyOtpResponse {
                success: false,
                message: "Database error. Please try again.".to_string(),
            };
            return Ok((jar, headers, Json(response)));
        }
    };

    // Generate JWT token using config from app state

    let claims = Claims {
        user_id: user.id,
        exp: (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + (90 * 24 * 60 * 60)) as usize, // 90 days
    };

    let jwt_token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(app_state.config.jwt_secret.as_ref()),
    ) {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to generate JWT token for user {}: {}", user.id, e);
            let response = VerifyOtpResponse {
                success: false,
                message: "Failed to generate authentication token.".to_string(),
            };
            return Ok((jar, headers, Json(response)));
        }
    };

    // Set JWT cookie
    let auth_cookie = Cookie::build(("auth_token", jwt_token))
        .http_only(true)
        .secure(false)  // Set to false for development (non-HTTPS)
        .path("/")
        .max_age(time::Duration::days(90))
        .build();

    let jar = jar.add(auth_cookie);

    info!("Successfully authenticated user {} with phone {}", user.id, payload.phone);
    
    let response = VerifyOtpResponse {
        success: true,
        message: "Login successful".to_string(),
    };

    headers.insert("HX-Redirect", "/".parse().unwrap());

    Ok((jar, headers, Json(response)))
}

pub async fn login_page() -> Html<String> {
    let template = LoginTemplate;
    Html(template.render().unwrap())
}

pub async fn logout(jar: CookieJar) -> (CookieJar, Redirect) {
    // Clear auth token cookie
    let clear_auth_cookie = Cookie::build(("auth_token", ""))
        .http_only(true)
        .secure(false)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    
    // Clear any remaining verification cookies
    let clear_verification_cookie = Cookie::build(("otp_verification_data", ""))
        .http_only(true)
        .secure(false)
        .path("/")
        .max_age(time::Duration::seconds(0))
        .build();
    
    let jar = jar.add(clear_auth_cookie).add(clear_verification_cookie);
    
    (jar, Redirect::to("/login"))
}
