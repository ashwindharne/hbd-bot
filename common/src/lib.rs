use sqlx::{migrate::MigrateDatabase, sqlite::SqlitePool, Row, Sqlite};

pub mod message_central;
pub mod twilio;
pub use message_central::MessageCentralSendOTPData;

pub struct DbUser {
    pub id: i64,
    pub phone_number: String,
    pub created_at: String,
    pub last_digest_at: Option<String>,
    pub days_notice: i64,
    pub send_hour: i64,
    pub iana_tz: String,
}

pub struct DbReminder {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub birthdate: i64, // Unix timestamp in milliseconds
    pub created_at: String,
    pub updated_at: String,
}

pub async fn init_database(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    if !Sqlite::database_exists(database_url).await.unwrap_or(false) {
        println!("Creating database {}", database_url);
        Sqlite::create_database(database_url).await?;
        println!("Database created successfully");
    }

    let pool = SqlitePool::connect(database_url).await?;

    sqlx::migrate!("../migrations").run(&pool).await?;

    Ok(pool)
}

pub async fn get_all_users(pool: &SqlitePool) -> Result<Vec<DbUser>, sqlx::Error> {
    let users =
        sqlx::query("SELECT id, phone_number, created_at, last_digest_at, days_notice, send_hour, iana_tz FROM users")
            .fetch_all(pool)
            .await?;

    let users: Vec<DbUser> = users
        .into_iter()
        .map(|row| DbUser {
            id: row.get("id"),
            phone_number: row.get("phone_number"),
            created_at: row.get("created_at"),
            last_digest_at: row.get("last_digest_at"),
            days_notice: row.get("days_notice"),
            send_hour: row.get("send_hour"),
            iana_tz: row.get("iana_tz"),
        })
        .collect();

    Ok(users)
}

pub async fn get_user_by_phone(
    pool: &SqlitePool,
    phone_number: &str,
) -> Result<Option<DbUser>, sqlx::Error> {
    let user = sqlx::query("SELECT id, phone_number, created_at, last_digest_at, days_notice, send_hour, iana_tz FROM users WHERE phone_number = ?")
        .bind(phone_number)
        .fetch_optional(pool)
        .await?;

    match user {
        Some(row) => Ok(Some(DbUser {
            id: row.get("id"),
            phone_number: row.get("phone_number"),
            created_at: row.get("created_at"),
            last_digest_at: row.get("last_digest_at"),
            days_notice: row.get("days_notice"),
            send_hour: row.get("send_hour"),
            iana_tz: row.get("iana_tz"),
        })),
        None => Ok(None),
    }
}

pub async fn get_user_by_id(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Option<DbUser>, sqlx::Error> {
    let user = sqlx::query("SELECT id, phone_number, created_at, last_digest_at, days_notice, send_hour, iana_tz FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

    match user {
        Some(row) => Ok(Some(DbUser {
            id: row.get("id"),
            phone_number: row.get("phone_number"),
            created_at: row.get("created_at"),
            last_digest_at: row.get("last_digest_at"),
            days_notice: row.get("days_notice"),
            send_hour: row.get("send_hour"),
            iana_tz: row.get("iana_tz"),
        })),
        None => Ok(None),
    }
}

pub async fn create_user(pool: &SqlitePool, phone_number: &str) -> Result<DbUser, sqlx::Error> {
    let result = sqlx::query("INSERT INTO users (phone_number) VALUES (?)")
        .bind(phone_number)
        .execute(pool)
        .await?;

    let user = sqlx::query(
        "SELECT id, phone_number, created_at, last_digest_at, days_notice, send_hour, iana_tz FROM users WHERE id = ?",
    )
    .bind(result.last_insert_rowid())
    .fetch_one(pool)
    .await?;

    Ok(DbUser {
        id: user.get("id"),
        phone_number: user.get("phone_number"),
        created_at: user.get("created_at"),
        last_digest_at: user.get("last_digest_at"),
        days_notice: user.get("days_notice"),
        send_hour: user.get("send_hour"),
        iana_tz: user.get("iana_tz"),
    })
}

pub async fn get_all_reminders(pool: &SqlitePool) -> Result<Vec<DbReminder>, sqlx::Error> {
    let reminders =
        sqlx::query("SELECT id, user_id, name, birthdate, created_at, updated_at FROM reminders")
            .fetch_all(pool)
            .await?;

    let reminders: Vec<DbReminder> = reminders
        .into_iter()
        .map(|row| DbReminder {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            birthdate: row.get("birthdate"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(reminders)
}

pub async fn get_reminders_by_user_id(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<Vec<DbReminder>, sqlx::Error> {
    let reminders = sqlx::query(
        "SELECT id, user_id, name, birthdate, created_at, updated_at 
         FROM reminders 
         WHERE user_id = ? 
         ORDER BY 
           CASE 
             WHEN strftime('%m-%d', datetime(birthdate / 1000, 'unixepoch')) >= strftime('%m-%d', 'now')
             THEN strftime('%m-%d', datetime(birthdate / 1000, 'unixepoch'))
             ELSE '13' || strftime('%m-%d', datetime(birthdate / 1000, 'unixepoch'))
           END"
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let reminders: Vec<DbReminder> = reminders
        .into_iter()
        .map(|row| DbReminder {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            birthdate: row.get("birthdate"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
        .collect();

    Ok(reminders)
}

pub async fn get_reminder_by_id(
    pool: &SqlitePool,
    reminder_id: i64,
) -> Result<Option<DbReminder>, sqlx::Error> {
    let reminder = sqlx::query(
        "SELECT id, user_id, name, birthdate, created_at, updated_at FROM reminders WHERE id = ?",
    )
    .bind(reminder_id)
    .fetch_optional(pool)
    .await?;

    match reminder {
        Some(row) => Ok(Some(DbReminder {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            birthdate: row.get("birthdate"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })),
        None => Ok(None),
    }
}

pub async fn create_reminder(
    pool: &SqlitePool,
    user_id: i64,
    name: &str,
    birthdate: &str,
) -> Result<DbReminder, sqlx::Error> {
    let result = sqlx::query("INSERT INTO reminders (user_id, name, birthdate) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(name)
        .bind(birthdate)
        .execute(pool)
        .await?;

    let reminder = sqlx::query(
        "SELECT id, user_id, name, birthdate, created_at, updated_at FROM reminders WHERE id = ?",
    )
    .bind(result.last_insert_rowid())
    .fetch_one(pool)
    .await?;

    Ok(DbReminder {
        id: reminder.get("id"),
        user_id: reminder.get("user_id"),
        name: reminder.get("name"),
        birthdate: reminder.get("birthdate"),
        created_at: reminder.get("created_at"),
        updated_at: reminder.get("updated_at"),
    })
}

pub async fn update_reminder(
    pool: &SqlitePool,
    reminder_id: i64,
    name: &str,
    birthdate: &str,
) -> Result<DbReminder, sqlx::Error> {
    sqlx::query(
        "UPDATE reminders SET name = ?, birthdate = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(name)
    .bind(birthdate)
    .bind(reminder_id)
    .execute(pool)
    .await?;

    let reminder = sqlx::query(
        "SELECT id, user_id, name, birthdate, created_at, updated_at FROM reminders WHERE id = ?",
    )
    .bind(reminder_id)
    .fetch_one(pool)
    .await?;

    Ok(DbReminder {
        id: reminder.get("id"),
        user_id: reminder.get("user_id"),
        name: reminder.get("name"),
        birthdate: reminder.get("birthdate"),
        created_at: reminder.get("created_at"),
        updated_at: reminder.get("updated_at"),
    })
}

pub async fn update_user_settings(
    pool: &SqlitePool,
    user_id: i64,
    days_notice: i64,
    send_hour: i64,
    iana_tz: &str,
) -> Result<DbUser, sqlx::Error> {
    sqlx::query("UPDATE users SET days_notice = ?, send_hour = ?, iana_tz = ? WHERE id = ?")
        .bind(days_notice)
        .bind(send_hour)
        .bind(iana_tz)
        .bind(user_id)
        .execute(pool)
        .await?;

    let user = sqlx::query(
        "SELECT id, phone_number, created_at, last_digest_at, days_notice, send_hour, iana_tz FROM users WHERE id = ?",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(DbUser {
        id: user.get("id"),
        phone_number: user.get("phone_number"),
        created_at: user.get("created_at"),
        last_digest_at: user.get("last_digest_at"),
        days_notice: user.get("days_notice"),
        send_hour: user.get("send_hour"),
        iana_tz: user.get("iana_tz"),
    })
}

pub async fn update_user_last_digest_at(
    pool: &SqlitePool,
    user_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET last_digest_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;

    Ok(())
}

pub async fn delete_reminder(pool: &SqlitePool, reminder_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM reminders WHERE id = ?")
        .bind(reminder_id)
        .execute(pool)
        .await?;

    Ok(())
}
