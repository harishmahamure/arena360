//! Called by pnpm demo:seed; never connects to the old operational database.
use chrono::NaiveDate;
use gaming_cafe_api::demo::{self, DemoOptions};
use std::path::PathBuf;
use uuid::Uuid;
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Demo seed failed: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    gaming_cafe_api::config::load_dotenv();
    let mut args = std::env::args().skip(1);
    let mut date = chrono::Utc::now().date_naive();
    let mut slug = "arena360-demo".to_string();
    let mut dry = false;
    let mut cell = std::env::var("ARENA_CELL_ID").ok();
    let mut owner = std::env::var("DEMO_OWNER_USER_ID").ok();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--date" => {
                date = NaiveDate::parse_from_str(
                    &args.next().ok_or("--date requires YYYY-MM-DD")?,
                    "%Y-%m-%d",
                )?
            }
            "--tenant-slug" => slug = args.next().ok_or("--tenant-slug requires a value")?,
            "--cell-id" => cell = Some(args.next().ok_or("--cell-id requires a UUID")?),
            "--owner-user-id" => {
                owner = Some(args.next().ok_or("--owner-user-id requires a UUID")?)
            }
            "--dry-run" => dry = true,
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    demo::validate(&slug, date)?;
    if dry {
        println!("{}", demo::preview(&slug, date));
        return Ok(());
    }
    let url = std::env::var("DEMO_CONTROL_DATABASE_URL")
        .or_else(|_| std::env::var("CONTROL_DATABASE_URL"))
        .map_err(|_| "Set CONTROL_DATABASE_URL or DEMO_CONTROL_DATABASE_URL")?;
    let cell_id =
        Uuid::parse_str(&cell.ok_or("Set ARENA_CELL_ID or pass --cell-id for a registered cell")?)?;
    let hash = match std::env::var("DEMO_PLAYER_PASSWORD") {
        Ok(password) => {
            if password.len() < 8 || password.len() > 72 {
                return Err("DEMO_PLAYER_PASSWORD must be 8–72 bytes".into());
            }
            bcrypt::hash(password, 10)?
        }
        Err(_) => "!demo-login-disabled!".into(),
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await?;
    gaming_cafe_api::control::migrate(&pool).await?;
    let value = demo::seed(
        pool.clone(),
        DemoOptions {
            slug,
            date,
            root: std::env::var("TENANT_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("data/tenants")),
            cell_id,
            owner_user_id: owner.map(|v| Uuid::parse_str(&v)).transpose()?,
            player_password_hash: hash,
        },
    )
    .await?;
    pool.close().await;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
