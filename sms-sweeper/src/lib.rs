use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Tz;
use common::{get_all_users, get_reminders_by_user_id, DbReminder, DbUser};
use log::{error, info, warn};
use sqlx::SqlitePool;

struct BirthdayReminder {
    name: String,
    days_until: i32,
    age_turning: i32,
}

#[derive(Debug, Clone)]
pub struct SmsMessage {
    pub user_id: i64,
    pub phone_number: String,
    pub message: String,
}

fn calculate_birthday_info(
    birthdate_timestamp: i64,
    user_tz: &Tz,
    current_utc: DateTime<Utc>,
) -> Result<(i32, i32), String> {
    // Convert Unix timestamp (milliseconds) to UTC DateTime
    let birthdate_utc =
        DateTime::from_timestamp_millis(birthdate_timestamp).ok_or("Invalid timestamp")?;
    let birthdate = birthdate_utc.naive_utc().date();

    // Use the user's local timezone to determine "today"
    let user_local_time = current_utc.with_timezone(user_tz);
    let today = user_local_time.naive_local().date();
    let current_year = today.year();

    info!(
        "Birthday calc: birthdate={}, user_local_today={}, current_year={}",
        birthdate.format("%Y-%m-%d"),
        today.format("%Y-%m-%d"),
        current_year
    );

    // Calculate this year's birthday
    let this_year_birthday = birthdate.with_year(current_year);

    let (next_birthday, age_turning) = match this_year_birthday {
        Some(this_year_bd) => {
            if this_year_bd >= today {
                // Birthday hasn't happened this year yet
                (this_year_bd, current_year - birthdate.year())
            } else {
                // Birthday already happened this year, calculate for next year
                let next_year_birthday = birthdate
                    .with_year(current_year + 1)
                    .ok_or("Failed to calculate next year's birthday")?;
                (next_year_birthday, current_year + 1 - birthdate.year())
            }
        }
        None => {
            // Handle leap year edge case (Feb 29 -> Feb 28)
            let next_year_birthday = birthdate
                .with_year(current_year + 1)
                .ok_or("Failed to calculate next year's birthday")?;
            (next_year_birthday, current_year + 1 - birthdate.year())
        }
    };

    let days_until = (next_birthday - today).num_days() as i32;

    info!(
        "Birthday result: next_birthday={}, days_until={}, age_turning={}",
        next_birthday.format("%Y-%m-%d"),
        days_until,
        age_turning
    );

    Ok((days_until, age_turning))
}

fn is_send_time_for_user(user: &DbUser, current_utc: DateTime<Utc>) -> bool {
    // Parse the user's timezone
    let user_tz: Tz = match user.iana_tz.parse() {
        Ok(tz) => tz,
        Err(_) => {
            warn!("Invalid timezone for user {}: {}", user.id, user.iana_tz);
            return false;
        }
    };

    // Convert UTC time to user's timezone
    let user_local_time = current_utc.with_timezone(&user_tz);
    let current_hour = user_local_time.hour() as i64;
    let is_time = current_hour == user.send_hour;

    info!(
        "User {} timezone check: UTC={}, Local={}:{:02} ({}), SendHour={}, IsTime={}",
        user.id,
        current_utc.format("%Y-%m-%d %H:%M:%S"),
        user_local_time.format("%Y-%m-%d %H:%M:%S"),
        user_local_time.minute(),
        user.iana_tz,
        user.send_hour,
        is_time
    );

    // Check if it's the user's preferred send hour
    is_time
}

fn was_notified_recently(user: &DbUser, current_utc: DateTime<Utc>) -> bool {
    // If last_digest_at is None, allow sending
    let last_digest_str = match &user.last_digest_at {
        Some(timestamp) => timestamp,
        None => {
            info!("User {} has no last_digest_at - allowing send", user.id);
            return false;
        }
    };

    // Parse the last_digest_at timestamp
    let last_digest = match DateTime::parse_from_rfc3339(&format!("{}Z", last_digest_str)) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(_) => {
            info!(
                "User {} has unparseable last_digest_at '{}' - allowing send",
                user.id, last_digest_str
            );
            // If we can't parse the timestamp, assume it's old and allow sending
            return false;
        }
    };

    // Check if it's been less than 12 hours since last notification
    let hours_since_last = (current_utc - last_digest).num_hours();
    let was_recent = hours_since_last < 12;

    info!(
        "User {} notification check: LastDigest={}, HoursSince={}, WasRecent={}",
        user.id,
        last_digest.format("%Y-%m-%d %H:%M:%S"),
        hours_since_last,
        was_recent
    );

    was_recent
}

fn get_reminders_to_send(
    user: &DbUser,
    reminders: Vec<DbReminder>,
    current_utc: DateTime<Utc>,
) -> Vec<BirthdayReminder> {
    let mut birthday_reminders = Vec::new();

    // Parse the user's timezone
    let user_tz: Tz = match user.iana_tz.parse() {
        Ok(tz) => tz,
        Err(_) => {
            warn!("Invalid timezone for user {}: {}", user.id, user.iana_tz);
            return birthday_reminders;
        }
    };

    for reminder in reminders {
        match calculate_birthday_info(reminder.birthdate, &user_tz, current_utc) {
            Ok((days_until, age_turning)) => {
                // Check if this birthday is within the user's notice period
                info!(
                    "Reminder for {}: days_until={}, user.days_notice={}, in_range={}",
                    reminder.name,
                    days_until,
                    user.days_notice,
                    days_until >= 0 && days_until <= user.days_notice as i32
                );

                if days_until >= 0 && days_until <= user.days_notice as i32 {
                    info!("Adding birthday reminder for {}", reminder.name);
                    birthday_reminders.push(BirthdayReminder {
                        name: reminder.name,
                        days_until,
                        age_turning,
                    });
                }
            }
            Err(e) => {
                warn!(
                    "Failed to calculate birthday info for reminder {}: {}",
                    reminder.id, e
                );
            }
        }
    }

    // Sort by days until birthday (most urgent first)
    birthday_reminders.sort_by_key(|r| r.days_until);
    birthday_reminders
}

fn format_birthday_message(reminders: &[BirthdayReminder]) -> String {
    let mut message = String::new();
    let suffix = "\nhttps://hbd.bot";
    let mut included_count = 0;

    for (i, reminder) in reminders.iter().enumerate() {
        let day_text = match reminder.days_until {
            0 => "today".to_string(),
            1 => "tomorrow".to_string(),
            n => format!("in {} days", n),
        };

        let reminder_text = format!(
            "{}'s {} is {}",
            reminder.name,
            ordinal_suffix(reminder.age_turning),
            day_text
        );

        // Calculate potential message lengths
        let current_with_newline = if message.is_empty() {
            reminder_text.clone()
        } else {
            format!("{}\n{}", message, reminder_text)
        };
        
        let remaining_after_this = reminders.len() - i - 1;
        
        // Option 1: Add this reminder + suffix
        let with_suffix = format!("{}{}", current_with_newline, suffix);
        
        if with_suffix.len() <= 160 {
            // If this is the last reminder or we can fit everything, add it
            if remaining_after_this == 0 {
                message = current_with_newline;
                included_count += 1;
                break;
            }
            
            // Check if we can fit the next reminder too
            if remaining_after_this == 1 {
                let next_reminder = &reminders[i + 1];
                let next_day_text = match next_reminder.days_until {
                    0 => "today".to_string(),
                    1 => "tomorrow".to_string(),
                    n => format!("in {} days", n),
                };
                let next_reminder_text = format!(
                    "{}'s {} is {}",
                    next_reminder.name,
                    ordinal_suffix(next_reminder.age_turning),
                    next_day_text
                );
                let with_next = format!("{}\n{}{}", current_with_newline, next_reminder_text, suffix);
                
                if with_next.len() <= 160 {
                    // We can fit both, so add this one and continue to next iteration
                    message = current_with_newline;
                    included_count += 1;
                    continue;
                }
            }
            
            // Check if we should show "+ X more..." instead
            if remaining_after_this > 1 {
                let more_text = format!("\n+ {} more...{}", remaining_after_this, suffix);
                let with_more = format!("{}{}", current_with_newline, more_text);
                
                if with_more.len() <= 160 {
                    message = current_with_newline;
                    included_count += 1;
                    break;
                }
            }
            
            // We can fit this reminder, add it and continue
            message = current_with_newline;
            included_count += 1;
        } else {
            // This reminder won't fit, stop here
            break;
        }
    }

    // Add suffix or "+ X more..." if there are remaining reminders
    let remaining = reminders.len() - included_count;
    if remaining > 1 {
        let more_text = format!("\n+ {} more...{}", remaining, suffix);
        let final_message = format!("{}{}", message, more_text);
        if final_message.len() <= 160 {
            return final_message;
        }
    }

    format!("{}{}", message, suffix)
}

fn ordinal_suffix(n: i32) -> String {
    let suffix = match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    };
    format!("{}{}", n, suffix)
}

pub async fn get_birthday_messages(
    db: &SqlitePool,
    current_utc: DateTime<Utc>,
) -> Result<Vec<SmsMessage>, Box<dyn std::error::Error>> {
    info!(
        "Getting birthday messages for current time: {}",
        current_utc.format("%Y-%m-%d %H:%M:%S")
    );

    // Get all users
    let users = get_all_users(&db).await?;
    info!("Found {} users", users.len());

    let mut messages = Vec::new();

    for user in users {
        info!(
            "Checking user {} (phone: {}, tz: {}, send_hour: {})",
            user.id, user.phone_number, user.iana_tz, user.send_hour
        );

        // Check if it's the right time to send for this user
        if !is_send_time_for_user(&user, current_utc) {
            info!("Skipping user {} - not send time", user.id);
            continue;
        }

        // Check if user was notified in the last 12 hours
        if was_notified_recently(&user, current_utc) {
            info!("Skipping user {} - notified within last 12 hours", user.id);
            continue;
        }

        info!(
            "Processing user {} (phone: {}) - passed time checks",
            user.id, user.phone_number
        );

        // Get reminders for this user
        let reminders = match get_reminders_by_user_id(&db, user.id).await {
            Ok(reminders) => reminders,
            Err(e) => {
                error!("Failed to get reminders for user {}: {}", user.id, e);
                continue;
            }
        };

        // Get birthday reminders that need to be sent
        let birthday_reminders = get_reminders_to_send(&user, reminders, current_utc);

        if birthday_reminders.is_empty() {
            continue;
        }

        info!(
            "Found {} birthday reminders for user {}",
            birthday_reminders.len(),
            user.id
        );

        // Format the message
        let message = format_birthday_message(&birthday_reminders);
        info!("Formatted message ({} chars): {}", message.len(), message);

        messages.push(SmsMessage {
            user_id: user.id,
            phone_number: user.phone_number.clone(),
            message,
        });
    }

    info!("Generated {} SMS messages", messages.len());
    Ok(messages)
}
