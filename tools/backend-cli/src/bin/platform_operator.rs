use std::io::{self, Read};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let username = std::env::args().nth(1).ok_or("Usage: platform_operator USERNAME (password on stdin)")?;
    if username.len()>100 || username.is_empty()
        || !username.bytes().next().is_some_and(|b|b.is_ascii_alphanumeric())
        || !username.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'.'||b==b'_'||b==b'-') {
        return Err("Username must start with a letter or digit and contain only letters, digits, dots, dashes or underscores".into());
    }
    let mut password = String::new();
    io::stdin().read_to_string(&mut password)?;
    let password = password.trim_end_matches(['\r','\n']);
    if !(12..=72).contains(&password.len()) {
        return Err("Password must be 12..72 bytes".into());
    }
    let hash = tokio::task::spawn_blocking({
        let password = password.to_owned();
        move || bcrypt::hash(password, bcrypt::DEFAULT_COST)
    }).await??;
    let pool = sqlx::PgPool::connect(&std::env::var("CONTROL_DATABASE_URL")?).await?;
    gaming_cafe_api::control::migrate(&pool).await?;
    sqlx::query("INSERT INTO platform_operators(username,password_hash) VALUES($1,$2)")
        .bind(&username).bind(hash).execute(&pool).await?;
    println!("Created platform operator {username}. TOTP enrollment is required on first login.");
    Ok(())
}
