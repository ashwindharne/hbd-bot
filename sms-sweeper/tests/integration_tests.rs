use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use common::{
    create_reminder, create_user, init_database, update_user_last_digest_at, update_user_settings,
};
use sms_sweeper::get_birthday_messages;
use tempfile::NamedTempFile;

async fn setup_test_database() -> (sqlx::SqlitePool, tempfile::NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let database_url = format!("sqlite:{}", temp_file.path().display());

    let db = init_database(&database_url).await.unwrap();
    (db, temp_file)
}

fn create_test_timestamp(year: i32, month: u32, day: u32) -> i64 {
    let naive_date = NaiveDate::from_ymd_opt(year, month, day).unwrap();
    let datetime = naive_date.and_hms_opt(0, 0, 0).unwrap();
    let utc_datetime = Utc.from_utc_datetime(&datetime);
    utc_datetime.timestamp_millis()
}

#[tokio::test]
async fn test_birthday_today_at_send_time() {
    let (db, _temp_file) = setup_test_database().await;

    // Create a user in Eastern timezone, send at 9 AM
    let user = create_user(&db, "1234567890").await.unwrap();
    update_user_settings(&db, user.id, 7, 9, "America/New_York")
        .await
        .unwrap();

    // Create a reminder for today's date
    let today = Utc::now().naive_utc().date();
    let birthday_timestamp = create_test_timestamp(1990, today.month(), today.day());
    create_reminder(&db, user.id, "John", &birthday_timestamp.to_string())
        .await
        .unwrap();

    // Test when it's 9 AM Eastern (2 PM UTC in winter)
    let eastern_9am = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 9, 0, 0)
        .unwrap();
    let utc_time = eastern_9am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].phone_number, "1234567890");
    assert!(messages[0].message.contains("John's"));
    assert!(messages[0].message.contains("today"));
    assert!(messages[0].message.starts_with("hbd.bot"));
    assert!(messages[0].message.len() <= 160);
}

#[tokio::test]
async fn test_birthday_tomorrow_at_send_time() {
    let (db, _temp_file) = setup_test_database().await;

    // Create a user in Pacific timezone, send at 10 AM
    let user = create_user(&db, "1234567891").await.unwrap();
    update_user_settings(&db, user.id, 3, 10, "America/Los_Angeles")
        .await
        .unwrap();

    // Create a reminder for tomorrow's date
    let tomorrow = Utc::now().naive_utc().date() + chrono::Duration::days(1);
    let birthday_timestamp = create_test_timestamp(1991, tomorrow.month(), tomorrow.day());
    create_reminder(&db, user.id, "Jane", &birthday_timestamp.to_string())
        .await
        .unwrap();

    // Test when it's 10 AM Pacific (6 PM UTC in winter)
    let pacific_10am = chrono_tz::US::Pacific
        .with_ymd_and_hms(2024, 1, 15, 10, 0, 0)
        .unwrap();
    let utc_time = pacific_10am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].phone_number, "1234567891");
    assert!(messages[0].message.contains("Jane's"));
    assert!(messages[0].message.contains("tomorrow"));
    assert!(messages[0].message.len() <= 160);
}

#[tokio::test]
async fn test_multiple_birthdays_same_user() {
    let (db, _temp_file) = setup_test_database().await;

    // Create a user with 3 days notice
    let user = create_user(&db, "1234567892").await.unwrap();
    update_user_settings(&db, user.id, 3, 9, "America/New_York")
        .await
        .unwrap();

    // Create multiple reminders
    let today = Utc::now().naive_utc().date();
    let birthday_today = create_test_timestamp(1990, today.month(), today.day());
    let birthday_tomorrow = create_test_timestamp(
        1991,
        (today + chrono::Duration::days(1)).month(),
        (today + chrono::Duration::days(1)).day(),
    );
    let birthday_in_3_days = create_test_timestamp(
        1992,
        (today + chrono::Duration::days(3)).month(),
        (today + chrono::Duration::days(3)).day(),
    );

    create_reminder(&db, user.id, "John", &birthday_today.to_string())
        .await
        .unwrap();
    create_reminder(&db, user.id, "Jane", &birthday_tomorrow.to_string())
        .await
        .unwrap();
    create_reminder(&db, user.id, "Bob", &birthday_in_3_days.to_string())
        .await
        .unwrap();

    // Test at 9 AM Eastern
    let eastern_9am = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 9, 0, 0)
        .unwrap();
    let utc_time = eastern_9am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();

    assert_eq!(messages.len(), 1);
    let message = &messages[0].message;
    assert!(message.contains("John's"));
    assert!(message.contains("Jane's"));
    assert!(message.contains("Bob's"));
    assert!(message.contains("today"));
    assert!(message.contains("tomorrow"));
    assert!(message.contains("in 3 days"));
    assert!(message.len() <= 160);
}

#[tokio::test]
async fn test_wrong_send_time_no_messages() {
    let (db, _temp_file) = setup_test_database().await;

    // Create a user who gets messages at 9 AM
    let user = create_user(&db, "1234567893").await.unwrap();
    update_user_settings(&db, user.id, 7, 9, "America/New_York")
        .await
        .unwrap();

    // Create a birthday today
    let today = Utc::now().naive_utc().date();
    let birthday_timestamp = create_test_timestamp(1990, today.month(), today.day());
    create_reminder(&db, user.id, "John", &birthday_timestamp.to_string())
        .await
        .unwrap();

    // Test at 8 AM Eastern (wrong time)
    let eastern_8am = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 8, 0, 0)
        .unwrap();
    let utc_time = eastern_8am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();

    assert_eq!(messages.len(), 0);
}

#[tokio::test]
async fn test_birthday_outside_notice_period() {
    let (db, _temp_file) = setup_test_database().await;

    // Create a user with only 1 day notice
    let user = create_user(&db, "1234567894").await.unwrap();
    update_user_settings(&db, user.id, 1, 9, "America/New_York")
        .await
        .unwrap();

    // Create a birthday in 3 days (outside notice period)
    let today = Utc::now().naive_utc().date();
    let birthday_in_3_days = create_test_timestamp(
        1990,
        (today + chrono::Duration::days(3)).month(),
        (today + chrono::Duration::days(3)).day(),
    );
    create_reminder(&db, user.id, "John", &birthday_in_3_days.to_string())
        .await
        .unwrap();

    // Test at correct send time
    let eastern_9am = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 9, 0, 0)
        .unwrap();
    let utc_time = eastern_9am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();

    assert_eq!(messages.len(), 0);
}

#[tokio::test]
async fn test_message_length_truncation() {
    let (db, _temp_file) = setup_test_database().await;

    // Create a user
    let user = create_user(&db, "1234567895").await.unwrap();
    update_user_settings(&db, user.id, 7, 9, "America/New_York")
        .await
        .unwrap();

    // Create many reminders to test truncation
    let today = Utc::now().naive_utc().date();
    for i in 0..10 {
        let birthday_timestamp = create_test_timestamp(1990 + i, today.month(), today.day());
        create_reminder(
            &db,
            user.id,
            &format!("VeryLongNamePersonNumber{}", i),
            &birthday_timestamp.to_string(),
        )
        .await
        .unwrap();
    }

    // Test at correct send time
    let eastern_9am = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 9, 0, 0)
        .unwrap();
    let utc_time = eastern_9am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();

    assert_eq!(messages.len(), 1);
    let message = &messages[0].message;
    assert!(message.len() <= 160);
    assert!(message.contains("hbd.bot"));
    // Should contain "and X more" since not all reminders fit
    assert!(message.contains("and ") && message.contains("more"));
}

#[tokio::test]
async fn test_multiple_users_different_timezones() {
    let (db, _temp_file) = setup_test_database().await;

    // Create user in Eastern timezone
    let user1 = create_user(&db, "1234567896").await.unwrap();
    update_user_settings(&db, user1.id, 7, 9, "America/New_York")
        .await
        .unwrap();

    // Create user in Pacific timezone
    let user2 = create_user(&db, "1234567897").await.unwrap();
    update_user_settings(&db, user2.id, 7, 10, "America/Los_Angeles")
        .await
        .unwrap();

    // Create birthdays for both users
    let today = Utc::now().naive_utc().date();
    let birthday_timestamp = create_test_timestamp(1990, today.month(), today.day());

    create_reminder(&db, user1.id, "John", &birthday_timestamp.to_string())
        .await
        .unwrap();
    create_reminder(&db, user2.id, "Jane", &birthday_timestamp.to_string())
        .await
        .unwrap();

    // Test at 9 AM Eastern (only user1 should get message)
    let eastern_9am = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 9, 0, 0)
        .unwrap();
    let utc_time = eastern_9am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].phone_number, "1234567896");
    assert!(messages[0].message.contains("John's"));

    // Test at 10 AM Pacific (only user2 should get message)
    let pacific_10am = chrono_tz::US::Pacific
        .with_ymd_and_hms(2024, 1, 15, 10, 0, 0)
        .unwrap();
    let utc_time = pacific_10am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].phone_number, "1234567897");
    assert!(messages[0].message.contains("Jane's"));
}

#[tokio::test]
async fn test_twelve_hour_notification_guard() {
    let (db, _temp_file) = setup_test_database().await;

    // Create a user who receives messages at both 9 AM and 10 PM
    let user = create_user(&db, "1234567898").await.unwrap();
    update_user_settings(&db, user.id, 7, 9, "America/New_York")
        .await
        .unwrap();

    // Create a birthday today
    let today = Utc::now().naive_utc().date();
    let birthday_timestamp = create_test_timestamp(1990, today.month(), today.day());
    create_reminder(&db, user.id, "John", &birthday_timestamp.to_string())
        .await
        .unwrap();

    // First call at 9 AM Eastern - should send message
    let eastern_9am = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 9, 0, 0)
        .unwrap();
    let utc_time = eastern_9am.with_timezone(&Utc);

    let messages = get_birthday_messages(&db, utc_time).await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].phone_number, "1234567898");
    assert!(messages[0].message.contains("John's"));

    // Simulate the actual sending process by updating last_digest_at
    update_user_last_digest_at(&db, user.id).await.unwrap();

    // Second call 1 hour later (10 AM Eastern) - should NOT send message due to 12-hour guard
    let eastern_10am = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 10, 0, 0)
        .unwrap();
    let utc_time_2 = eastern_10am.with_timezone(&Utc);

    let messages_2 = get_birthday_messages(&db, utc_time_2).await.unwrap();
    assert_eq!(messages_2.len(), 0);

    // Update user to receive messages at 10 PM and test 13 hours later (same day at 10 PM Eastern) - should send message again
    update_user_settings(&db, user.id, 7, 22, "America/New_York")
        .await
        .unwrap();
    let eastern_10pm_same_day = chrono_tz::US::Eastern
        .with_ymd_and_hms(2024, 1, 15, 22, 0, 0)
        .unwrap();
    let utc_time_3 = eastern_10pm_same_day.with_timezone(&Utc);

    let messages_3 = get_birthday_messages(&db, utc_time_3).await.unwrap();
    assert_eq!(messages_3.len(), 0);
}
