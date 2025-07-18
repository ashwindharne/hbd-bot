use askama::Template;
use axum::{
    extract::{State, Query},
    response::{Html, Redirect},
    Form,
};
use axum_extra::extract::cookie::CookieJar;
use common::{get_user_by_id, get_reminders_by_user_id, create_reminder, get_reminder_by_id, update_reminder};
use serde::Deserialize;
use chrono::{NaiveDate, DateTime};
use std::collections::HashMap;

use crate::controllers::auth::{AppState, AppTemplate, verify_jwt_cookie, convert_reminders_to_display};

#[derive(Template)]
#[template(path = "add.html")]
pub struct AddTemplate {
    pub error_message: String,
    pub name: String,
    pub birthdate: String,
}

#[derive(Template)]
#[template(path = "edit.html")]
pub struct EditTemplate {
    pub reminder_id: i64,
    pub error_message: String,
    pub name: String,
    pub birthdate: String,
}

#[derive(Deserialize)]
pub struct AddBirthdayForm {
    name: String,
    birthdate: String,
}

#[derive(Deserialize)]
pub struct EditBirthdayForm {
    reminder_id: i64,
    name: String,
    birthdate: String,
}

pub async fn root(State(state): State<AppState>, jar: CookieJar) -> Result<Html<String>, Redirect> {
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

    // Get reminders for this user
    let reminders = match get_reminders_by_user_id(&state.db, user_id).await {
        Ok(reminders) => convert_reminders_to_display(reminders),
        Err(_) => {
            // If we can't get reminders, just show empty list
            Vec::new()
        }
    };

    // Render the app template with user data and reminders
    let template = AppTemplate {
        phone_number: user.phone_number,
        reminders,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn add_form(State(state): State<AppState>, jar: CookieJar) -> Result<Html<String>, Redirect> {
    // Verify JWT cookie and get user ID
    match verify_jwt_cookie(&jar, &state.config.jwt_secret) {
        Ok(_) => {
            // User is authenticated, show the form
            let template = AddTemplate {
                error_message: String::new(),
                name: String::new(),
                birthdate: String::new(),
            };
            Ok(Html(template.render().unwrap()))
        }
        Err(_) => {
            // Redirect to login if authentication fails
            Err(Redirect::to("/login"))
        }
    }
}

pub async fn add_birthday(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<AddBirthdayForm>,
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
    if form.name.trim().is_empty() {
        let template = AddTemplate {
            error_message: "Please enter a name".to_string(),
            name: form.name,
            birthdate: form.birthdate,
        };
        return Err(Html(template.render().unwrap()));
    }

    if form.birthdate.trim().is_empty() {
        let template = AddTemplate {
            error_message: "Please select a birthday".to_string(),
            name: form.name,
            birthdate: form.birthdate,
        };
        return Err(Html(template.render().unwrap()));
    }

    // Validate and convert date format
    let parsed_date = match NaiveDate::parse_from_str(&form.birthdate, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => {
            let template = AddTemplate {
                error_message: "Invalid date format".to_string(),
                name: form.name,
                birthdate: form.birthdate,
            };
            return Err(Html(template.render().unwrap()));
        }
    };

    // Convert to timestamp for database storage
    let timestamp = parsed_date.and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();

    // Convert timestamp back to string for the create_reminder function
    let timestamp_str = timestamp.to_string();

    // Create the reminder
    match create_reminder(&state.db, user_id, &form.name.trim(), &timestamp_str).await {
        Ok(_) => {
            // Success - redirect to dashboard
            Ok(Redirect::to("/"))
        }
        Err(_) => {
            // Database error
            let template = AddTemplate {
                error_message: "Failed to save birthday. Please try again.".to_string(),
                name: form.name,
                birthdate: form.birthdate,
            };
            Err(Html(template.render().unwrap()))
        }
    }
}

pub async fn edit_form(
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

    // Get reminder ID from query parameters
    let reminder_id = match params.get("id").and_then(|id| id.parse::<i64>().ok()) {
        Some(id) => id,
        None => {
            // Invalid or missing ID, redirect to dashboard
            return Err(Redirect::to("/"));
        }
    };

    // Get reminder from database
    let reminder = match get_reminder_by_id(&state.db, reminder_id).await {
        Ok(Some(reminder)) => reminder,
        Ok(None) => {
            // Reminder not found, redirect to dashboard
            return Err(Redirect::to("/"));
        }
        Err(_) => {
            // Database error, redirect to dashboard
            return Err(Redirect::to("/"));
        }
    };

    // Check that reminder belongs to authenticated user
    if reminder.user_id != user_id {
        // Unauthorized access, redirect to dashboard
        return Err(Redirect::to("/"));
    }

    // Convert timestamp to date string for form
    let formatted_date = match DateTime::from_timestamp_millis(reminder.birthdate) {
        Some(dt) => dt.naive_utc().date().format("%Y-%m-%d").to_string(),
        None => String::new(),
    };

    // Render edit form with reminder data
    let template = EditTemplate {
        reminder_id: reminder.id,
        error_message: String::new(),
        name: reminder.name,
        birthdate: formatted_date,
    };

    Ok(Html(template.render().unwrap()))
}

pub async fn update_birthday(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(form): Form<EditBirthdayForm>,
) -> Result<Redirect, Html<String>> {
    // Verify JWT cookie and get user ID
    let user_id = match verify_jwt_cookie(&jar, &state.config.jwt_secret) {
        Ok(user_id) => user_id,
        Err(_) => {
            // Redirect to login if authentication fails
            return Ok(Redirect::to("/login"));
        }
    };

    // Get existing reminder to verify ownership
    let existing_reminder = match get_reminder_by_id(&state.db, form.reminder_id).await {
        Ok(Some(reminder)) => reminder,
        Ok(None) => {
            // Reminder not found, redirect to dashboard
            return Ok(Redirect::to("/"));
        }
        Err(_) => {
            // Database error, redirect to dashboard
            return Ok(Redirect::to("/"));
        }
    };

    // Check that reminder belongs to authenticated user
    if existing_reminder.user_id != user_id {
        // Unauthorized access, redirect to dashboard
        return Ok(Redirect::to("/"));
    }

    // Validate form data
    if form.name.trim().is_empty() {
        let template = EditTemplate {
            reminder_id: form.reminder_id,
            error_message: "Please enter a name".to_string(),
            name: form.name,
            birthdate: form.birthdate,
        };
        return Err(Html(template.render().unwrap()));
    }

    if form.birthdate.trim().is_empty() {
        let template = EditTemplate {
            reminder_id: form.reminder_id,
            error_message: "Please select a birthday".to_string(),
            name: form.name,
            birthdate: form.birthdate,
        };
        return Err(Html(template.render().unwrap()));
    }

    // Validate and convert date format
    let parsed_date = match NaiveDate::parse_from_str(&form.birthdate, "%Y-%m-%d") {
        Ok(date) => date,
        Err(_) => {
            let template = EditTemplate {
                reminder_id: form.reminder_id,
                error_message: "Invalid date format".to_string(),
                name: form.name,
                birthdate: form.birthdate,
            };
            return Err(Html(template.render().unwrap()));
        }
    };

    // Convert to timestamp for database storage
    let timestamp = parsed_date.and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis();

    // Convert timestamp back to string for the update_reminder function
    let timestamp_str = timestamp.to_string();

    // Update the reminder
    match update_reminder(&state.db, form.reminder_id, &form.name.trim(), &timestamp_str).await {
        Ok(_) => {
            // Success - redirect to dashboard
            Ok(Redirect::to("/"))
        }
        Err(_) => {
            // Database error
            let template = EditTemplate {
                reminder_id: form.reminder_id,
                error_message: "Failed to update birthday. Please try again.".to_string(),
                name: form.name,
                birthdate: form.birthdate,
            };
            Err(Html(template.render().unwrap()))
        }
    }
}