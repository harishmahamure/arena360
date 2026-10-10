//! Operator-issued cold transitions; cell agents execute the durable lifecycle.
use gaming_cafe_api::cold as control;
use uuid::Uuid;
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Cold transition failed: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut tenant = None;
    let mut idle = 86400;
    let mut status = None;
    let mut cancel = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("tenant_cold --tenant UUID [--idle-seconds SECONDS]\ntenant_cold --status JOB_UUID\ntenant_cold --cancel JOB_UUID\nUses CONTROL_DATABASE_URL. Cells need replication and tenant keys. The default idle threshold is 86400 seconds. Cancellation is allowed before lease release.");
                return Ok(());
            }
            "--tenant" => {
                tenant = Some(Uuid::parse_str(
                    &args.next().ok_or("--tenant requires UUID")?,
                )?)
            }
            "--idle-seconds" => {
                idle = args
                    .next()
                    .ok_or("--idle-seconds requires integer")?
                    .parse::<i64>()?;
            }
            "--status" => {
                status = Some(Uuid::parse_str(
                    &args.next().ok_or("--status requires UUID")?,
                )?)
            }
            "--cancel" => {
                cancel = Some(Uuid::parse_str(
                    &args.next().ok_or("--cancel requires UUID")?,
                )?)
            }
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    gaming_cafe_api::config::load_dotenv();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CONTROL_DATABASE_URL")?)
        .await?;
    let modes = usize::from(status.is_some())
        + usize::from(cancel.is_some())
        + usize::from(tenant.is_some());
    if modes != 1 {
        return Err("Choose exactly one of request, --status, or --cancel".into());
    }
    let job = if let Some(id) = status {
        control::get(&pool, id).await?
    } else if let Some(id) = cancel {
        control::cancel(&pool, id).await?;
        control::get(&pool, id).await?
    } else {
        control::enqueue(&pool, tenant.ok_or("Pass --tenant UUID")?, idle).await?
    };
    println!("{}", serde_json::to_string_pretty(&job)?);
    Ok(())
}
