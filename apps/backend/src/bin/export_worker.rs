#[cfg(not(feature = "duckdb-analytics"))]
fn main() {
    eprintln!("export_worker requires --features duckdb-analytics");
    std::process::exit(2);
}
#[cfg(feature = "duckdb-analytics")]
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("Export worker failed: {error}");
        std::process::exit(1);
    }
}
#[cfg(feature = "duckdb-analytics")]
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    use gaming_cafe_api::{
        historical::{export_worker::Worker, exports},
        metrics::Metrics,
    };
    use std::{sync::Arc, time::Duration};
    use uuid::Uuid;
    let mut args = std::env::args().skip(1);
    let mut once = false;
    let mut id = None;
    let mut token = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("export_worker [--once]\nexport_worker --run-job UUID --token UUID\nUses CONTROL_DATABASE_URL, CELL_ID, EXPORT_STAGING_DIR, replication storage and tenant keys. Admission limits are in historical_export_limits. Every job runs in a separate process with one DuckDB thread and a 128 MB memory limit.");
                return Ok(());
            }
            "--once" => once = true,
            "--run-job" => {
                id = Some(
                    args.next()
                        .ok_or("--run-job requires UUID")?
                        .parse::<Uuid>()?,
                )
            }
            "--token" => {
                token = Some(
                    args.next()
                        .ok_or("--token requires UUID")?
                        .parse::<Uuid>()?,
                )
            }
            _ => return Err(format!("Unknown argument {arg}").into()),
        }
    }
    gaming_cafe_api::config::load_dotenv();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&std::env::var("CONTROL_DATABASE_URL")?)
        .await?;
    let cell = std::env::var("CELL_ID")?.parse::<Uuid>()?;
    let staging = std::path::PathBuf::from(std::env::var("EXPORT_STAGING_DIR")?);
    std::fs::create_dir_all(&staging)?;
    if !staging.is_absolute() {
        return Err("EXPORT_STAGING_DIR must be absolute".into());
    }
    if let Some(id) = id {
        let token = token.ok_or("--run-job requires --token")?;
        let (store, keys) = exports::storage_from_env()?;
        let metrics = Arc::new(Metrics::default());
        let jobs = gaming_cafe_api::background::BackgroundJobs::new(
            gaming_cafe_api::background::Limits {
                outbox_slots: 1,
                background_slots: 1,
                backfill_slots: 1,
            },
        )?;
        if std::env::var("DISK_PRESSURE_MONITOR").as_deref() != Ok("false") {
            let sample = gaming_cafe_api::disk::measure(&staging)?;
            jobs.set_disk_zone(gaming_cafe_api::disk::zone(sample.used_permille(), 0));
            gaming_cafe_api::disk::spawn(staging.clone(), jobs.clone(), metrics.clone());
        }
        let work = async {
            let _permit = jobs
                .acquire(gaming_cafe_api::background::Priority::HistoricalExport)
                .await?;
            Worker {
                pool: pool.clone(),
                cell,
                store,
                keys,
                staging_root: staging,
                metrics,
            }
            .run(id, token)
            .await
        };
        tokio::pin!(work);
        let mut ticks = tokio::time::interval(Duration::from_secs(20));
        loop {
            tokio::select! {
             result=&mut work=>return result.map_err(Into::into),
             _=ticks.tick()=>if !matches!(tokio::time::timeout(Duration::from_secs(15),exports::renew(&pool,id,token)).await,Ok(Ok(()))){eprintln!("Export heartbeat failed or timed out; fencing this process");std::process::exit(1);},
             _=tokio::signal::ctrl_c()=>std::process::exit(130),
            }
        }
    }
    if token.is_some() {
        return Err("--token requires --run-job".into());
    }
    let mut running = tokio::task::JoinSet::new();
    let executable = std::env::current_exe()?;
    let mut cleaned = std::time::Instant::now() - Duration::from_secs(60);
    loop {
        if cleaned.elapsed() >= Duration::from_secs(60) {
            exports::cleanup_staging(&pool, &staging).await?;
            cleaned = std::time::Instant::now();
        }
        while let Some(result) = running.try_join_next() {
            if let Err(error) = result {
                eprintln!("Export supervisor task failed: {error}");
            }
        }
        // Avoid claiming work while disk pressure pauses staging; existing jobs continue to heartbeat.
        let admitted = std::env::var("DISK_PRESSURE_MONITOR").as_deref() == Ok("false")
            || gaming_cafe_api::disk::measure(&staging)
                .is_ok_and(|s| gaming_cafe_api::disk::zone(s.used_permille(), 0) < 2);
        let job = if admitted {
            exports::claim(&pool, cell).await?
        } else {
            None
        };
        if let Some(job) = job {
            let pool = pool.clone();
            let executable = executable.clone();
            running.spawn(async move {
                let token = job.worker_token.expect("claimed export token");
                #[cfg(unix)]
                let mut command = {
                    let mut c = tokio::process::Command::new("/usr/bin/nice");
                    c.args(["-n", "10"]).arg(&executable);
                    c
                };
                #[cfg(not(unix))]
                let mut command = tokio::process::Command::new(&executable);
                let status = command
                    .args([
                        "--run-job",
                        &job.id.to_string(),
                        "--token",
                        &token.to_string(),
                    ])
                    .kill_on_drop(true)
                    .status()
                    .await;
                if !status.as_ref().is_ok_and(|s| s.success()) {
                    let error = format!("Isolated export process failed: {status:?}");
                    eprintln!("{error}");
                    let _ = exports::release(&pool, job.id, token, &error).await;
                }
            });
            continue;
        }
        if once {
            while running.join_next().await.is_some() {}
            return Ok(());
        }
        sqlx::query("UPDATE historical_exports SET state='EXPIRED' WHERE state='READY' AND expires_at<=clock_timestamp()").execute(&pool).await?;
        sqlx::query("UPDATE historical_exports SET state='FAILED',last_error='Export request expired before completion' WHERE state IN ('QUEUED','PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING') AND expires_at<=clock_timestamp()").execute(&pool).await?;
        tokio::select! {_=tokio::time::sleep(Duration::from_secs(1))=>{},_=tokio::signal::ctrl_c()=>{running.abort_all();while running.join_next().await.is_some(){};return Ok(());}}
    }
}
