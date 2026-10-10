#[cfg(not(feature = "duckdb-analytics"))]
fn main() {
    eprintln!("archive_backfill requires --features duckdb-analytics");
    std::process::exit(2);
}
#[cfg(feature = "duckdb-analytics")]
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Backfill command failed: {error}");
        std::process::exit(1);
    }
}
#[cfg(feature = "duckdb-analytics")]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    use gaming_cafe_api::historical::backfill;
    use uuid::Uuid;
    let mut args = std::env::args().skip(1);
    let mut source = None;
    let mut status = None;
    let mut p99 = 25;
    let mut batch = 100;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("archive_backfill --archive UUID [--p99-ms 25] [--batch-rows 100]\narchive_backfill --status UUID\nUses CONTROL_DATABASE_URL. Wake COLD tenants first. The native owning cell processes the durable job.");
                return Ok(());
            }
            "--archive" => {
                source = Some(
                    args.next()
                        .ok_or("--archive requires UUID")?
                        .parse::<Uuid>()?,
                )
            }
            "--status" => {
                status = Some(
                    args.next()
                        .ok_or("--status requires UUID")?
                        .parse::<Uuid>()?,
                )
            }
            "--p99-ms" => {
                p99 = args
                    .next()
                    .ok_or("--p99-ms requires number")?
                    .parse::<i64>()?
            }
            "--batch-rows" => {
                batch = args
                    .next()
                    .ok_or("--batch-rows requires number")?
                    .parse::<i32>()?
            }
            _ => return Err(format!("Unknown argument {arg}").into()),
        }
    }
    if usize::from(source.is_some()) + usize::from(status.is_some()) != 1 {
        return Err("Choose --archive or --status".into());
    }
    gaming_cafe_api::config::load_dotenv();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CONTROL_DATABASE_URL")?)
        .await?;
    let job = if let Some(id) = status {
        backfill::get(&pool, id).await?
    } else {
        backfill::enqueue(&pool, source.unwrap(), p99, batch).await?
    };
    println!("{}", serde_json::to_string_pretty(&job)?);
    Ok(())
}
