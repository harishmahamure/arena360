use sqlx::{PgPool, Row};
use uuid::Uuid;
async fn rejected(pool: &PgPool, sql: &str, id: Uuid) {
    assert!(
        sqlx::query(sql).bind(id).execute(pool).await.is_err(),
        "accepted: {sql}"
    );
}
#[tokio::test]
#[ignore = "requires an isolated control-plane database"]
async fn historical_states_guard_verified_evidence_and_tenant_boundaries() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let tenant = Uuid::new_v4();
    let other = Uuid::new_v4();
    let cell = Uuid::new_v4();
    let user = Uuid::new_v4();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(cell)
        .bind(cell.to_string())
        .bind(format!("http://{cell}.invalid"))
        .execute(&pool)
        .await
        .unwrap();
    for id in [tenant, other] {
        sqlx::query("INSERT INTO tenants(id,slug,name,timezone,owner_cell,ownership_generation,state) VALUES($1,$2,'Archive','UTC',$3,1,'ACTIVE')").bind(id).bind(id.to_string()).bind(cell).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO users(id,username,password_hash) VALUES($1,$2,'fixture')")
        .bind(user)
        .bind(user.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let archive = Uuid::new_v4();
    sqlx::query("INSERT INTO archive_manifests(id,tenant_id,source_cell,ownership_generation,period_start,period_end,timezone,schema_version) VALUES($1,$2,$3,1,'2024-01-01','2024-02-01','UTC',16)").bind(archive).bind(tenant).bind(cell).execute(&pool).await.unwrap();
    rejected(
        &pool,
        "UPDATE archive_manifests SET state='PURGING' WHERE id=$1",
        archive,
    )
    .await;
    sqlx::query("UPDATE archive_manifests SET state='EXPORTING' WHERE id=$1")
        .bind(archive)
        .execute(&pool)
        .await
        .unwrap();
    rejected(&pool,"UPDATE archive_manifests SET state='UPLOADED',row_count=2,checksum_sha256=repeat('a',64),objects='[]' WHERE id=$1",archive).await;
    let evidence = serde_json::json!([{"key":format!("tenants/{tenant}/archive/2024/01/facts.parquet"),"checksum_sha256":"b".repeat(64),"rows":2,"bytes":10}]);
    let wrong = serde_json::json!([{"key":format!("tenants/{other}/archive/2024/01/facts.parquet"),"checksum_sha256":"b".repeat(64),"rows":2,"bytes":10}]);
    assert!(sqlx::query("UPDATE archive_manifests SET state='UPLOADED',row_count=2,checksum_sha256=repeat('a',64),objects=$2 WHERE id=$1").bind(archive).bind(wrong).execute(&pool).await.is_err());
    sqlx::query("UPDATE archive_manifests SET state='UPLOADED',row_count=2,checksum_sha256=repeat('a',64),objects=$2 WHERE id=$1").bind(archive).bind(evidence).execute(&pool).await.unwrap();
    let backfill = Uuid::new_v4();
    assert!(sqlx::query("INSERT INTO historical_backfills(id,tenant_id,source_archive,target_schema_version) VALUES($1,$2,$3,16)").bind(backfill).bind(tenant).bind(archive).execute(&pool).await.is_err());
    rejected(
        &pool,
        "UPDATE archive_manifests SET state='VERIFIED' WHERE id=$1",
        archive,
    )
    .await;
    sqlx::query(
        "UPDATE archive_manifests SET state='VERIFIED',verified_at=clock_timestamp() WHERE id=$1",
    )
    .bind(archive)
    .execute(&pool)
    .await
    .unwrap();
    rejected(
        &pool,
        "UPDATE archive_manifests SET objects='[]' WHERE id=$1",
        archive,
    )
    .await;
    rejected(
        &pool,
        "UPDATE archive_manifests SET source_watermark=99 WHERE id=$1",
        archive,
    )
    .await;
    rejected(
        &pool,
        "UPDATE archive_manifests SET state='EXPORTING' WHERE id=$1",
        archive,
    )
    .await;
    assert!(sqlx::query("INSERT INTO historical_backfills(id,tenant_id,source_archive,target_schema_version) VALUES($1,$2,$3,16)").bind(backfill).bind(other).bind(archive).execute(&pool).await.is_err());
    sqlx::query("INSERT INTO historical_backfills(id,tenant_id,source_archive,target_schema_version) VALUES($1,$2,$3,16)").bind(backfill).bind(tenant).bind(archive).execute(&pool).await.unwrap();
    rejected(
        &pool,
        "UPDATE historical_backfills SET state='BACKFILLING' WHERE id=$1",
        backfill,
    )
    .await;
    for state in ["DOWNLOADING", "TRANSFORMING", "VALIDATING"] {
        sqlx::query("UPDATE historical_backfills SET state=$2 WHERE id=$1")
            .bind(backfill)
            .bind(state)
            .execute(&pool)
            .await
            .unwrap();
    }
    rejected(
        &pool,
        "UPDATE historical_backfills SET state='BACKFILLING' WHERE id=$1",
        backfill,
    )
    .await;
    sqlx::query("UPDATE historical_backfills SET state='BACKFILLING',validation_checksum=repeat('c',64),expected_rows=2,rows_processed=1,checkpoint='{\"facts\":\"first\"}' WHERE id=$1").bind(backfill).execute(&pool).await.unwrap();
    rejected(
        &pool,
        "UPDATE historical_backfills SET rows_processed=0 WHERE id=$1",
        backfill,
    )
    .await;
    rejected(
        &pool,
        "UPDATE historical_backfills SET staging_objects='[{}]' WHERE id=$1",
        backfill,
    )
    .await;
    rejected(&pool,"UPDATE historical_backfills SET expected_rows=3 WHERE id=$1",backfill).await;
    rejected(&pool,"UPDATE historical_backfills SET source_objects='[{}]' WHERE id=$1",backfill).await;
    sqlx::query("UPDATE historical_backfills SET state='VERIFYING',rows_processed=2 WHERE id=$1")
        .bind(backfill)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE historical_backfills SET state='COMPLETE',completed_at=clock_timestamp() WHERE id=$1").bind(backfill).execute(&pool).await.unwrap();
    sqlx::query("UPDATE archive_manifests SET state='PURGING',rows_purged=1,purge_checkpoint='{\"facts\":\"first\"}' WHERE id=$1").bind(archive).execute(&pool).await.unwrap();
    rejected(
        &pool,
        "UPDATE archive_manifests SET rows_purged=0 WHERE id=$1",
        archive,
    )
    .await;
    rejected(
        &pool,
        "UPDATE archive_manifests SET state='COMPLETE',purged_at=clock_timestamp() WHERE id=$1",
        archive,
    )
    .await;
    sqlx::query("UPDATE archive_manifests SET state='COMPLETE',rows_purged=2,purged_at=clock_timestamp() WHERE id=$1").bind(archive).execute(&pool).await.unwrap();
    let export = Uuid::new_v4();
    sqlx::query("INSERT INTO historical_exports(id,tenant_id,requested_by,period_start,period_end,format,expires_at) VALUES($1,$2,$3,'2024-01-01','2024-02-01','CSV_GZ',clock_timestamp()+INTERVAL '1 day')").bind(export).bind(tenant).bind(user).execute(&pool).await.unwrap();
    rejected(
        &pool,
        "UPDATE historical_exports SET state='READY' WHERE id=$1",
        export,
    )
    .await;
    for state in ["PREPARING", "SCANNING_ARCHIVE", "GENERATING", "UPLOADING"] {
        sqlx::query("UPDATE historical_exports SET state=$2 WHERE id=$1")
            .bind(export)
            .bind(state)
            .execute(&pool)
            .await
            .unwrap();
    }
    rejected(
        &pool,
        "UPDATE historical_exports SET format='CSV' WHERE id=$1",
        export,
    )
    .await;
    assert!(sqlx::query("UPDATE historical_exports SET state='READY',result_object_key=$2,checksum_sha256=repeat('d',64),result_size_bytes=10,row_count=2,ready_at=clock_timestamp() WHERE id=$1").bind(export).bind(format!("tenants/{other}/exports/{export}/download.csv.gz")).execute(&pool).await.is_err());
    sqlx::query("UPDATE historical_exports SET state='READY',result_object_key=$2,checksum_sha256=repeat('d',64),result_size_bytes=10,row_count=2,ready_at=clock_timestamp() WHERE id=$1").bind(export).bind(format!("tenants/{tenant}/exports/{export}/download.csv.gz")).execute(&pool).await.unwrap();
    rejected(
        &pool,
        "UPDATE historical_exports SET state='FAILED' WHERE id=$1",
        export,
    )
    .await;
    rejected(
        &pool,
        "UPDATE historical_exports SET row_count=3 WHERE id=$1",
        export,
    )
    .await;
    sqlx::query("UPDATE historical_exports SET state='EXPIRED' WHERE id=$1")
        .bind(export)
        .execute(&pool)
        .await
        .unwrap();
    rejected(
        &pool,
        "UPDATE historical_exports SET state='QUEUED' WHERE id=$1",
        export,
    )
    .await;
    let state = sqlx::query("SELECT state FROM historical_exports WHERE id=$1")
        .bind(export)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state.get::<String, _>(0), "EXPIRED");
}
