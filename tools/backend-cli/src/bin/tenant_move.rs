//! Operator-issued planned moves; cell agents execute the durable state machine.
use gaming_cafe_api::moving::control;
use uuid::Uuid;
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Tenant move failed: {error}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut tenant = None;
    let mut target = None;
    let mut status = None;
    let mut cancel = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("tenant_move --tenant UUID --target-cell UUID\ntenant_move --status MOVE_UUID\ntenant_move --cancel MOVE_UUID\nUses CONTROL_DATABASE_URL. Both cells must run the move agent with replication and the tenant key configured. Cancellation is allowed only before cutover.");
                return Ok(());
            }
            "--tenant" => {
                tenant = Some(Uuid::parse_str(
                    &args.next().ok_or("--tenant requires UUID")?,
                )?)
            }
            "--target-cell" => {
                target = Some(Uuid::parse_str(
                    &args.next().ok_or("--target-cell requires UUID")?,
                )?)
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
        + usize::from(tenant.is_some() || target.is_some());
    if modes != 1 {
        return Err("Choose exactly one of request, --status, or --cancel".into());
    }
    let job = if let Some(id) = status {
        control::get(&pool, id).await?
    } else if let Some(id) = cancel {
        control::cancel(&pool, id).await?;
        control::get(&pool, id).await?
    } else {
        control::enqueue(
            &pool,
            tenant.ok_or("Pass --tenant UUID")?,
            target.ok_or("Pass --target-cell UUID")?,
        )
        .await?
    };
    println!("{}", serde_json::to_string_pretty(&job)?);
    Ok(())
}
