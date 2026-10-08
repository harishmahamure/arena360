#[cfg(not(feature = "duckdb-analytics"))]
fn main() {
    eprintln!("archive_month requires --features duckdb-analytics");
    std::process::exit(2);
}
#[cfg(feature = "duckdb-analytics")]
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Archive command failed: {error}");
        std::process::exit(1);
    }
}
#[cfg(feature = "duckdb-analytics")]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    use chrono::NaiveDate;
    use gaming_cafe_api::historical::archive;
    use uuid::Uuid;
    let mut args = std::env::args().skip(1);
    let mut tenant = None;
    let mut month = None;
    let mut status = None;
    let mut replan = None;
    let mut p99 = 25;
    let mut batch = 100;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("archive_month --tenant UUID --month YYYY-MM-01 [--p99-ms 25] [--batch-rows 100]\narchive_month --status UUID\narchive_month --replan UUID\nUses CONTROL_DATABASE_URL. Only complete months older than 18 months are eligible. Native owning-cell workers execute the durable job. Replan preserves earlier verified revisions.");
                return Ok(());
            }
            "--tenant" => {
                tenant = Some(
                    args.next()
                        .ok_or("--tenant requires UUID")?
                        .parse::<Uuid>()?,
                )
            }
            "--month" => {
                month = Some(
                    args.next()
                        .ok_or("--month requires date")?
                        .parse::<NaiveDate>()?,
                )
            }
            "--status" => {
                status = Some(
                    args.next()
                        .ok_or("--status requires UUID")?
                        .parse::<Uuid>()?,
                )
            }
            "--replan" => {
                replan = Some(
                    args.next()
                        .ok_or("--replan requires UUID")?
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
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if usize::from(tenant.is_some() || month.is_some())
        + usize::from(status.is_some())
        + usize::from(replan.is_some())
        != 1
    {
        return Err("Choose request, --status, or --replan".into());
    }
    gaming_cafe_api::config::load_dotenv();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CONTROL_DATABASE_URL")?)
        .await?;
    let job = if let Some(id) = status {
        archive::get(&pool, id).await?
    } else if let Some(id) = replan {
        archive::replan(&pool, id).await?
    } else {
        archive::enqueue(
            &pool,
            tenant.ok_or("Pass --tenant")?,
            month.ok_or("Pass --month")?,
            p99,
            batch,
        )
        .await?
    };
    println!("{}", serde_json::to_string_pretty(&job)?);
    Ok(())
}
