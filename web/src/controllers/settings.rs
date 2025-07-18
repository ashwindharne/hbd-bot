use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, Redirect},
    Form,
};
use axum_extra::extract::cookie::CookieJar;
use common::{get_user_by_id, update_user_settings};
use serde::Deserialize;
use std::collections::HashMap;

use crate::controllers::auth::{verify_jwt_cookie, AppState};

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub error_message: String,
    pub success_message: String,
    pub days_notice: i64,
    pub send_hour: i64,
    pub iana_tz: String,
}

#[derive(Deserialize)]
pub struct SettingsForm {
    days_notice: i64,
    send_hour: i64,
    iana_tz: String,
}

pub async fn settings_form(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Html<String>, Redirect> {
    // Verify JWT cookie and get user ID
    let user_id = match verify_jwt_cookie(&jar, &state.config.jwt_secret) {
        Ok(user_id) => user_id,
        Err(_) => {
            // Redirect to login if authentication fails
            return Err(Redirect::to("/login"));
        }
    };

    // Get user from database
    let user = match get_user_by_id(&state.db, user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            // User not found in database, redirect to login
            return Err(Redirect::to("/login"));
        }
        Err(_) => {
            // Database error, redirect to login
            return Err(Redirect::to("/login"));
        }
    };

    // Check for success message from query parameters
    let success_message = if params.get("success").is_some() {
        "Settings saved successfully!".to_string()
    } else {
        String::new()
    };

    // Render the settings template with current user settings
    let template = SettingsTemplate {
        error_message: String::new(),
        success_message,
        days_notice: user.days_notice,
        send_hour: user.send_hour,
        iana_tz: user.iana_tz,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn update_settings(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<SettingsForm>,
) -> Result<Redirect, Html<String>> {
    // Verify JWT cookie and get user ID
    let user_id = match verify_jwt_cookie(&jar, &state.config.jwt_secret) {
        Ok(user_id) => user_id,
        Err(_) => {
            // Redirect to login if authentication fails
            return Ok(Redirect::to("/login"));
        }
    };

    // Validate form data
    if form.days_notice < 1 || form.days_notice > 14 {
        let template = SettingsTemplate {
            error_message: "Days notice must be between 1 and 14".to_string(),
            success_message: String::new(),
            days_notice: form.days_notice,
            send_hour: form.send_hour,
            iana_tz: form.iana_tz,
        };
        return Err(Html(template.render().unwrap()));
    }

    if form.send_hour < 0 || form.send_hour > 23 {
        let template = SettingsTemplate {
            error_message: "Send hour must be between 0 and 23".to_string(),
            success_message: String::new(),
            days_notice: form.days_notice,
            send_hour: form.send_hour,
            iana_tz: form.iana_tz,
        };
        return Err(Html(template.render().unwrap()));
    }

    // Validate timezone (US timezones only)
    let valid_timezones = vec![
        "America/New_York",
        "America/Chicago",
        "America/Denver",
        "America/Los_Angeles",
        "America/Anchorage",
        "Pacific/Honolulu",
    ];

    if !valid_timezones.contains(&form.iana_tz.as_str()) {
        let template = SettingsTemplate {
            error_message: "Invalid timezone selected".to_string(),
            success_message: String::new(),
            days_notice: form.days_notice,
            send_hour: form.send_hour,
            iana_tz: form.iana_tz,
        };
        return Err(Html(template.render().unwrap()));
    }

    // Update user settings in database
    match update_user_settings(
        &state.db,
        user_id,
        form.days_notice,
        form.send_hour,
        &form.iana_tz,
    )
    .await
    {
        Ok(_) => {
            // Success - redirect to settings page with success message
            Ok(Redirect::to("/settings?success=1"))
        }
        Err(_) => {
            // Database error
            let template = SettingsTemplate {
                error_message: "Failed to save settings. Please try again.".to_string(),
                success_message: String::new(),
                days_notice: form.days_notice,
                send_hour: form.send_hour,
                iana_tz: form.iana_tz,
            };
            Err(Html(template.render().unwrap()))
        }
    }
}
