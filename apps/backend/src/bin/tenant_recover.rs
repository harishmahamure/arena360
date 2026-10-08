//! Explicit operator command; lease expiry/skew and restore verification are mandatory.
use gaming_cafe_api::{
    control::{LeaseClient, LeaseConfig},
    metrics::Metrics,
    replication::{self, recovery::Recoverer},
    tenancy::{TenantDbConfig, TenantDbManager},
};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Cell recovery failed: {e}");
        std::process::exit(1);
    }
}
async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut lost = None;
    let mut target = None;
    let mut list = false;
    let mut operations_only = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("tenant_recover --lost-cell UUID [--list | --target-cell UUID [--operations-only]]\nUses CONTROL_DATABASE_URL, TENANT_DATA_DIR and replication configuration. Recovery waits for the old lease plus skew to expire; it never force-steals ownership. Native analytics recovery also requires NATS_URL. --list only reads the control plane.");
                return Ok(());
            }
            "--lost-cell" => {
                lost = Some(Uuid::parse_str(
                    &args.next().ok_or("--lost-cell requires UUID")?,
                )?)
            }
            "--target-cell" => {
                target = Some(Uuid::parse_str(
                    &args.next().ok_or("--target-cell requires UUID")?,
                )?)
            }
            "--list" => list = true,
            "--operations-only" => operations_only = true,
            _ => return Err(format!("Unknown argument: {arg}").into()),
        }
    }
    let lost = lost.ok_or("Pass --lost-cell UUID")?;
    gaming_cafe_api::config::load_dotenv();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&std::env::var("CONTROL_DATABASE_URL")?)
        .await?;
    if list {
        let tenants:Vec<Uuid>=sqlx::query_scalar("SELECT id FROM tenants WHERE owner_cell=$1 AND storage_engine='SQLITE' AND state NOT IN ('PROVISIONING','DELETED','FAILED','COLD') ORDER BY id").bind(lost).fetch_all(&pool).await?;
        println!("{}", serde_json::to_string_pretty(&tenants)?);
        return Ok(());
    }
    let target = target.ok_or("Pass --target-cell UUID")?;
    let root = PathBuf::from(
        std::env::var("TENANT_DATA_DIR")
            .map_err(|_| "Set TENANT_DATA_DIR explicitly for the destination cell")?,
    );
    let jobs = gaming_cafe_api::background::BackgroundJobs::new(Default::default())?;
    let leases = Arc::new(LeaseClient::new(
        pool.clone(),
        target,
        LeaseConfig::default(),
    )?);
    leases.clone().spawn_renewal();
    let manager = Arc::new(
        TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                ..Default::default()
            },
            leases.clone(),
        )?
        .with_background_jobs(jobs),
    );
    let worker = replication::configured_worker(
        Some(pool.clone()),
        Some(target),
        Arc::new(Metrics::default()),
    )?
    .ok_or("Configure replication bucket and durable tenant keys")?;
    let analytics: Option<Arc<dyn replication::recovery::RecoveryAnalytics>> = if operations_only {
        None
    } else {
        #[cfg(feature = "duckdb-analytics")]
        {
            Some(Arc::new(replication::recovery::NativeAnalytics {
                registry: Arc::new(
                    gaming_cafe_api::analytics::registry::AnalyticsRegistry::default(),
                ),
                broker_url: std::env::var("NATS_URL")?,
            }))
        }
        #[cfg(not(feature = "duckdb-analytics"))]
        {
            return Err("Build with --features duckdb-analytics, or use --operations-only while the running cell rebuilds analytics".into());
        }
    };
    let recoverer = Arc::new(Recoverer {
        ledger: Arc::new(replication::ledger::PostgresLedger {
            pool: pool.clone(),
            cell_id: target,
        }),
        leases,
        databases: manager.clone(),
        store: worker.store,
        keys: worker.keys,
        staging_root: root.join("recovery-staging"),
        analytics,
    });
    let outcomes = recoverer.clone().recover_cell(lost).await?;
    println!("{}", serde_json::to_string_pretty(&outcomes)?);
    let failed = outcomes
        .iter()
        .any(|o| !o.operations_ready || o.error.is_some());
    for db in manager.open_handles().await {
        db.close().await?;
    }
    if failed {
        return Err("Some tenants remain pending; rerun to resume their durable jobs".into());
    }
    Ok(())
}
