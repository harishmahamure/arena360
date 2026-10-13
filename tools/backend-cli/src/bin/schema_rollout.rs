use gaming_cafe_api::tenancy::rollout;
use std::time::Duration;
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
    let mut all = false;
    let mut wait_seconds = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" => {
                println!("schema_rollout --all [--wait-seconds 600]\nschema_rollout --canary TENANT_UUID[,TENANT_UUID...] [--soak-seconds 300]\nschema_rollout --status ROLLOUT_UUID\nschema_rollout --resume ROLLOUT_UUID [--wait-seconds 600]\nUses CONTROL_DATABASE_URL. --all admits every eligible tenant immediately; otherwise rolls out through canary, 1%, 10%, 25%, 100%. Owning storage cells must run this schema version with replication and current leases. Migration failure halts new admissions. Resume only after diagnosing the failure.");
                return Ok(());
            }
            "--canary" => canary = Some(args.next().ok_or("Missing canaries")?),
            "--all" => all = true,
            "--wait-seconds" => {
                let seconds: u64 = args.next().ok_or("Missing wait seconds")?.parse()?;
                if seconds == 0 {
                    return Err("Wait seconds must be greater than zero".into());
                }
                wait_seconds = Some(seconds);
            }
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
    if usize::from(all)
        + usize::from(canary.is_some())
        + usize::from(status.is_some())
        + usize::from(resume.is_some())
        != 1
    {
        return Err("Choose --all, --canary, --status, or --resume".into());
    }
    gaming_cafe_api::config::load_dotenv();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CONTROL_DATABASE_URL")?)
        .await?;
    let id = if all {
        let excluded: Vec<(String, i64)> = sqlx::query_as(
            "SELECT state,count(*) FROM tenants WHERE schema_version<$1 AND (owner_cell IS NULL OR state IN ('PROVISIONING','FAILED','DELETED','COLD')) GROUP BY state ORDER BY state",
        ).bind(gaming_cafe_api::tenancy::target_schema_version()).fetch_all(&pool).await?;
        for (state, count) in excluded {
            eprintln!("Excluded {count} tenant(s) in state {state}: cold tenants must be woken, provisioning must finish, and tenants need an owning cell before migration.");
        }
        let Some(id) = rollout::create_all(&pool).await? else {
            println!(
                "All eligible tenants already have schema v{}.",
                gaming_cafe_api::tenancy::target_schema_version()
            );
            return Ok(());
        };
        id
    } else if let Some(ids) = canary {
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
    if let Some(seconds) = wait_seconds {
        wait_for_rollout(&pool, id, seconds).await?;
    }
    Ok(())
}

async fn wait_for_rollout(
    pool: &sqlx::PgPool,
    id: Uuid,
    seconds: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let started = tokio::time::Instant::now();
    let mut previous = None;
    loop {
        let status = rollout::status(pool, id).await?;
        let progress = format!(
            "Rollout {id}: {} / {} migrated, {} failed, state {}",
            status["succeeded"], status["tenant_count"], status["failed"], status["state"]
        );
        if previous.as_ref() != Some(&progress) {
            println!("{progress}");
            previous = Some(progress);
        }
        match status["state"].as_str() {
            Some("COMPLETE") => return Ok(()),
            Some("HALTED") => return Err(format!("Rollout {id} halted: {}. Diagnose the failure, then use schema_rollout --resume {id}.", status["last_error"]).into()),
            _ => {},
        }
        let blocked = pending_tenants(pool, id, true).await?;
        if !blocked.is_empty() {
            return Err(format!(
                "Rollout {id} cannot run yet: queued tenants need an ACTIVE owning cell running this schema version, configured replication and a current lease. Start/restart storage with PORT=3001 (pnpm storage:dev), then rerun the migration command. Blocked tenants: {}",
                serde_json::to_string_pretty(&blocked)?
            ).into());
        }
        if started.elapsed() >= Duration::from_secs(seconds) {
            let pending = pending_tenants(pool, id, false).await?;
            return Err(format!("Timed out waiting for rollout {id}. Work remains queued; start/restart owning storage cells with the current code, replication configured and valid leases, then rerun the command. Pending tenants: {}", serde_json::to_string_pretty(&pending)?).into());
        }
        tokio::time::sleep(
            Duration::from_secs(2)
                .min(Duration::from_secs(seconds).saturating_sub(started.elapsed())),
        )
        .await;
    }
}

async fn pending_tenants(
    pool: &sqlx::PgPool,
    id: Uuid,
    blocked_only: bool,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    sqlx::query_scalar(
                r#"SELECT jsonb_build_object('tenant',t.id,'name',t.name,'state',t.state,'ownerCell',t.owner_cell,
                   'leaseReady',COALESCE(l.expires_at>clock_timestamp()+INTERVAL '30 seconds',false),
                   'workerReady',COALESCE(c.hydration_heartbeat_at>clock_timestamp()-INTERVAL '30 seconds',false),
                   'migrationState',m.state,'error',m.last_error)
                   FROM schema_rollout_tenants m JOIN tenants t ON t.id=m.tenant_id
                   LEFT JOIN cells c ON c.id=t.owner_cell LEFT JOIN tenant_leases l ON l.tenant_id=t.id
                     AND l.owner_cell=t.owner_cell AND l.ownership_generation=t.ownership_generation
                   WHERE m.rollout_id=$1 AND m.state<>'SUCCEEDED'
                   AND (NOT $2 OR (m.state='QUEUED' AND (t.state<>'ACTIVE'
                     OR NOT COALESCE(l.expires_at>clock_timestamp()+INTERVAL '30 seconds',false)
                     OR NOT COALESCE(c.hydration_heartbeat_at>clock_timestamp()-INTERVAL '30 seconds',false))))
                   ORDER BY m.ordinal LIMIT 20"#,
    ).bind(id).bind(blocked_only).fetch_all(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::ConnectOptions;

    #[tokio::test]
    #[ignore = "requires isolated PostgreSQL"]
    async fn wait_reports_completion_halt_and_unavailable_owners() {
        let url = std::env::var("CONTROL_TEST_DATABASE_URL").unwrap();
        let admin = sqlx::PgPool::connect(&url).await.unwrap();
        let schema = format!("rollout_wait_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_with(
                url.parse::<sqlx::postgres::PgConnectOptions>()
                    .unwrap()
                    .options([("search_path", schema.as_str())])
                    .disable_statement_logging(),
            )
            .await
            .unwrap();
        gaming_cafe_api::control::migrate(&pool).await.unwrap();
        let cell = Uuid::new_v4();
        let tenant = Uuid::new_v4();
        sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,'Wait','http://wait.invalid')")
            .bind(cell)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,state) VALUES($1,'wait','Wait','UTC',$2,'ACTIVE')")
            .bind(tenant).bind(cell).execute(&pool).await.unwrap();
        let id = rollout::create_all(&pool).await.unwrap().unwrap();
        let error = wait_for_rollout(&pool, id, 1)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("cannot run yet"), "{error}");
        assert!(error.contains(&tenant.to_string()), "{error}");
        assert!(error.contains("\"leaseReady\": false"), "{error}");
        assert!(error.contains("\"workerReady\": false"), "{error}");
        // A ready owner that stops processing still produces a bounded timeout.
        sqlx::query("UPDATE tenants SET ownership_generation=1 WHERE id=$1")
            .bind(tenant)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE cells SET hydration_heartbeat_at=clock_timestamp() WHERE id=$1")
            .bind(cell)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tenant_leases(tenant_id,owner_cell,ownership_generation,expires_at) VALUES($1,$2,1,clock_timestamp()+INTERVAL '5 minutes')")
            .bind(tenant).bind(cell).execute(&pool).await.unwrap();
        let error = wait_for_rollout(&pool, id, 1)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("Timed out"), "{error}");
        sqlx::query(
            "UPDATE schema_rollouts SET state='HALTED',last_error='backup failed' WHERE id=$1",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        let error = wait_for_rollout(&pool, id, 1)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("backup failed"), "{error}");
        assert!(error.contains("--resume"), "{error}");
        sqlx::query("UPDATE schema_rollouts SET state='COMPLETE' WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        wait_for_rollout(&pool, id, 1).await.unwrap();
        pool.close().await;
        sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
            .execute(&admin)
            .await
            .unwrap();
        admin.close().await;
    }
}
