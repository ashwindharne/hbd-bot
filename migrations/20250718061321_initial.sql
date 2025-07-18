-- Initial migration for hbd-bot
-- Users table for storing user information
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    phone_number TEXT NOT NULL UNIQUE,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_digest_at DATETIME,
    days_notice INTEGER NOT NULL DEFAULT 7,
    send_hour INTEGER NOT NULL DEFAULT 9,
    iana_tz TEXT NOT NULL DEFAULT 'America/New_York'
);

-- Index for efficient lookups
CREATE INDEX idx_users_phone_number ON users(phone_number);

-- Friends table for storing birthday information
CREATE TABLE reminders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    birthdate INTEGER NOT NULL,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Index for efficient lookups
CREATE INDEX idx_reminders_user_id ON reminders(user_id);
CREATE INDEX idx_reminders_birthdate ON reminders(birthdate);
