use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use common::{
    create_reminder, create_user as db_create_user, get_all_reminders, get_all_users, init_database,
};
use log;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

mod controllers;

use controllers::auth::AppState;
use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub message_central_customer_id: String,
    pub message_central_email: String,
    pub message_central_password_b64: String,
    pub jwt_secret: String,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let database_url =
            env::var("DATABASE_URL").map_err(|_| "DATABASE_URL environment variable not set")?;
        let message_central_customer_id = env::var("MESSAGE_CENTRAL_CUSTOMER_ID")
            .map_err(|_| "MESSAGE_CENTRAL_CUSTOMER_ID environment variable not set")?;
        let message_central_email = env::var("MESSAGE_CENTRAL_EMAIL")
            .map_err(|_| "MESSAGE_CENTRAL_EMAIL environment variable not set")?;
        let message_central_password_b64 = env::var("MESSAGE_CENTRAL_PASSWORD_B64")
            .map_err(|_| "MESSAGE_CENTRAL_PASSWORD_B64 environment variable not set")?;
        let jwt_secret =
            env::var("JWT_SECRET").map_err(|_| "JWT_SECRET environment variable not set")?;

        Ok(Config {
            database_url,
            message_central_customer_id,
            message_central_email,
            message_central_password_b64,
            jwt_secret,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct User {
    id: i64,
    phone_number: String,
}

#[derive(Serialize, Deserialize)]
struct Reminder {
    id: i64,
    user_id: i64,
    name: String,
    birthdate: i64, // Unix timestamp in milliseconds
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    log::info!("Starting HBD Bot Web Server");

    // Parse environment variables at startup
    let config = Config::from_env().map_err(|e| {
        log::error!("Failed to parse environment variables: {}", e);
        e
    })?;

    let db = init_database(&config.database_url).await?;

    let state = AppState { db, config };

    let app = Router::new()
        .route("/", get(controllers::app::root))
        .route(
            "/add",
            get(controllers::app::add_form).post(controllers::app::add_birthday),
        )
        .route(
            "/edit",
            get(controllers::app::edit_form).post(controllers::app::update_birthday),
        )
        .route(
            "/settings",
            get(controllers::settings::settings_form).post(controllers::settings::update_settings),
        )
        .route(
            "/login",
            get(controllers::auth::login_page).post(controllers::auth::login),
        )
        .route("/verify-otp", post(controllers::auth::verify_otp))
        .route("/logout", get(controllers::auth::logout))
        .route("/health", get(health))
        .route("/users", get(get_users).post(create_user))
        .route(
            "/reminders",
            get(get_reminders).post(create_reminder_endpoint),
        )
        .nest_service("/static", ServeDir::new("web/static"))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    log::info!("Server running on http://0.0.0.0:3000");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "hbd-bot-web"
    }))
}

async fn get_users(State(state): State<AppState>) -> Result<Json<Vec<User>>, StatusCode> {
    let db_users = get_all_users(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let users: Vec<User> = db_users
        .into_iter()
        .map(|db_user| User {
            id: db_user.id,
            phone_number: db_user.phone_number,
        })
        .collect();

    Ok(Json(users))
}

async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<User>, StatusCode> {
    let phone_number = payload["phone_number"]
        .as_str()
        .ok_or(StatusCode::BAD_REQUEST)?;

    let db_user = db_create_user(&state.db, phone_number)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user = User {
        id: db_user.id,
        phone_number: db_user.phone_number,
    };

    Ok(Json(user))
}

async fn get_reminders(State(state): State<AppState>) -> Result<Json<Vec<Reminder>>, StatusCode> {
    let db_reminders = get_all_reminders(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let reminders: Vec<Reminder> = db_reminders
        .into_iter()
        .map(|db_reminder| Reminder {
            id: db_reminder.id,
            user_id: db_reminder.user_id,
            name: db_reminder.name,
            birthdate: db_reminder.birthdate,
        })
        .collect();

    Ok(Json(reminders))
}

async fn create_reminder_endpoint(
    State(state): State<AppState>,
    Json(payload): Json<serde_json::Value>,
) -> Result<Json<Reminder>, StatusCode> {
    let user_id = payload["user_id"].as_i64().ok_or(StatusCode::BAD_REQUEST)?;
    let name = payload["name"].as_str().ok_or(StatusCode::BAD_REQUEST)?;
    let birthdate = payload["birthdate"]
        .as_str()
        .ok_or(StatusCode::BAD_REQUEST)?;

    let db_reminder = create_reminder(&state.db, user_id, name, birthdate)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let reminder = Reminder {
        id: db_reminder.id,
        user_id: db_reminder.user_id,
        name: db_reminder.name,
        birthdate: db_reminder.birthdate,
    };

    Ok(Json(reminder))
}
