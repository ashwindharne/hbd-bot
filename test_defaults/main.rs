use common::{init_database, create_user};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a temporary test database
    let test_db = "test_defaults.db";

    // Remove the test database if it exists
    if std::path::Path::new(test_db).exists() {
        std::fs::remove_file(test_db)?;
    }

    // Initialize the database
    let pool = init_database(test_db).await?;

    // Create a test user
    let test_phone = "+1234567890";
    let user = create_user(&pool, test_phone).await?;

    // Check that the defaults were applied
    println!("Created user with ID: {}", user.id);
    println!("Phone number: {}", user.phone_number);
    println!("Days notice: {}", user.days_notice);
    println!("Send hour: {}", user.send_hour);
    println!("Timezone: {}", user.iana_tz);

    // Verify the defaults
    assert_eq!(user.days_notice, 7, "days_notice should default to 7");
    assert_eq!(user.send_hour, 9, "send_hour should default to 9");
    assert_eq!(user.iana_tz, "America/New_York", "iana_tz should default to America/New_York");

    println!("✅ All defaults are working correctly!");

    // Clean up
    std::fs::remove_file(test_db)?;

    Ok(())
}
