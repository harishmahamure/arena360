use gaming_cafe_api::tenancy::rollout;
use uuid::Uuid;
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Schema rollout failed: {e}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut canary = None;
    let mut status = None;
    let mut resume = None;
    let mut soak = 300;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("schema_rollout --canary TENANT_UUID[,TENANT_UUID...] [--soak-seconds 300]\nschema_rollout --status ROLLOUT_UUID\nschema_rollout --resume ROLLOUT_UUID\nUses CONTROL_DATABASE_URL. This binary's schema version rolls out through canary, 1%, 10%, 25%, 100%; migration failure halts new admissions. Resume only after diagnosing the failure.");
                return Ok(());
            }
            "--canary" => canary = Some(args.next().ok_or("Missing canaries")?),
            "--status" => {
                status = Some(Uuid::parse_str(
                    &args.next().ok_or("Missing rollout UUID")?,
                )?)
            }
            "--resume" => {
                resume = Some(Uuid::parse_str(
                    &args.next().ok_or("Missing rollout UUID")?,
                )?)
            }
            "--soak-seconds" => soak = args.next().ok_or("Missing soak seconds")?.parse()?,
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    if usize::from(canary.is_some()) + usize::from(status.is_some()) + usize::from(resume.is_some())
        != 1
    {
        return Err("Choose --canary, --status, or --resume".into());
    }
    gaming_cafe_api::config::load_dotenv();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CONTROL_DATABASE_URL")?)
        .await?;
    let id = if let Some(ids) = canary {
        let ids = ids
            .split(',')
            .map(Uuid::parse_str)
            .collect::<Result<Vec<_>, _>>()?;
        rollout::create(
            &pool,
            gaming_cafe_api::tenancy::target_schema_version(),
            &ids,
            soak,
        )
        .await?
    } else if let Some(id) = resume {
        rollout::resume(&pool, id).await?;
        id
    } else {
        status.unwrap()
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&rollout::status(&pool, id).await?)?
    );
    Ok(())
}
