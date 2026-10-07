use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::models::{
    status, AssignPlanDto, CreateDeviceDto, DeviceFilterDto, PlayerPlanFilterDto,
    PlayerPlanUpdateValues, PurchaseBalanceDto, SessionFilterDto,
};
use gaming_cafe_api::realtime::OutboxService;
use gaming_cafe_api::repositories::{
    TenantBalanceRepository, TenantDeviceRepository, TenantPlayerPlanRepository,
    TenantSessionRepository,
};
use gaming_cafe_api::services::deduction_profile::weighted_minutes_between;
use gaming_cafe_api::services::{
    BalanceService, DeviceService, EventService, NotificationService, PlayerPlanService,
};
use gaming_cafe_api::sse::Broadcaster;
use gaming_cafe_api::tenancy::{
    tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use uuid::Uuid;

#[derive(Default)]
struct Lease {
    generations: RwLock<HashMap<Uuid, i64>>,
}

impl TenantLease for Lease {
    fn writable_generation(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        self.generations
            .read()
            .unwrap()
            .get(&tenant_id)
            .copied()
            .ok_or_else(|| AppError::Forbidden("tenant lease is not writable".into()))
    }

    fn ensure_writable(&self, tenant_id: Uuid, generation: i64) -> Result<(), AppError> {
        if self.writable_generation(tenant_id)? == generation {
            Ok(())
        } else {
            Err(AppError::Forbidden(
                "tenant lease generation changed".into(),
            ))
        }
    }
}

#[tokio::test]
async fn device_wallet_and_session_mutations_are_atomic_and_idempotent() {
    let fixture = Fixture::new().await;
    let devices = TenantDeviceRepository::new(fixture.db.clone());
    let device = devices
        .create(
            &CreateDeviceDto {
                name: "PC-01".into(),
                location_id: Some(fixture.location_id),
                serial_number: Some("AA:BB".into()),
                local_ip_address: None,
                device_type: Some("PC".into()),
                device_sub_type: Some("HIGH_END_PCS".into()),
                location: Some("Floor A".into()),
                status: None,
                registration_status: Some("registered".into()),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        devices
            .list(
                &DeviceFilterDto {
                    name: Some("pc-".into()),
                    ..Default::default()
                },
                &[fixture.location_id],
            )
            .await
            .unwrap()
            .total,
        1
    );

    let balances = TenantBalanceRepository::new(fixture.db.clone());
    let balance = balances
        .purchase_or_recharge(
            &PurchaseBalanceDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: Some(fixture.transaction_id),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(balance.remaining_minutes, 120);
    assert_eq!(balance.allowed_days, Some(json!(["monday", "tuesday"])));
    assert_eq!(balance.allowed_months, Some(json!([1, 10, 12])));

    let sessions = TenantSessionRepository::new(fixture.db.clone());
    let session_start = Utc::now() - ChronoDuration::minutes(30);
    let profile: gaming_cafe_api::models::deduction_profile::DeductionProfile =
        serde_json::from_value(balance.deduction_profile.clone().unwrap()).unwrap();
    let weighted = weighted_minutes_between(
        session_start,
        session_start + ChronoDuration::minutes(30),
        &profile,
        "UTC",
    )
    .ceil() as i32;
    assert_eq!(weighted, 45);
    // A foreign, absent, or concurrently closed shift cannot be attached to a session.
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    assert!(sessions
        .start(
            fixture.player_id,
            balance.id,
            device.id,
            fixture.location_id,
            Some(Uuid::now_v7()),
            session_start,
            Some(fixture.player_id),
            json!({})
        )
        .await
        .is_err());
    let closed_shift = Uuid::now_v7();
    let at = gaming_cafe_api::time::format_sqlite_timestamp(&session_start).unwrap();
    let owner = fixture.player_id;
    let location = fixture.location_id;
    fixture.db.with_immediate_writer(move |c|Box::pin(async move {
        sqlx::query("INSERT INTO shifts(id,user_id,location_id,clock_in,clock_out,status,created_at,updated_at) VALUES(?,?,?,?,?,'closed',?,?)")
            .bind(closed_shift.to_string()).bind(owner.to_string()).bind(location.to_string()).bind(&at).bind(&at).bind(&at).bind(&at).execute(c).await?;
        Ok(())
    })).await.unwrap();
    assert!(sessions
        .start(
            fixture.player_id,
            balance.id,
            device.id,
            fixture.location_id,
            Some(closed_shift),
            session_start,
            Some(fixture.player_id),
            json!({})
        )
        .await
        .is_err());
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        devices.find_by_id(device.id).await.unwrap().unwrap().status,
        "available"
    );
    let started = sessions
        .start(
            fixture.player_id,
            balance.id,
            device.id,
            fixture.location_id,
            None,
            session_start,
            None,
            serde_json::to_value(profile).unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(started.session.time_credits_consumed, Some(0));
    assert_eq!(
        devices.find_by_id(device.id).await.unwrap().unwrap().status,
        "in_use"
    );
    let started_payload: String = sqlx::query_scalar(
        "SELECT payload FROM outbox_events
         WHERE aggregate_id=? AND event_type='session.started'
         ORDER BY sequence DESC LIMIT 1",
    )
    .bind(started.session.id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    let started_payload: serde_json::Value = serde_json::from_str(&started_payload).unwrap();
    assert_eq!(started_payload["deviceName"], "PC-01");
    assert_eq!(started_payload["walletMinutesAtStart"], 120);
    assert_eq!(started_payload["remainingMinutes"], 120);
    assert_eq!(started_payload["cafeTimezone"], "UTC");
    assert!(started_payload["deductionProfile"].is_object());

    let first = sessions
        .charge(started.session.id, weighted, None, None)
        .await
        .unwrap();
    let retried = sessions
        .charge(started.session.id, weighted, None, None)
        .await
        .unwrap();
    assert_eq!(first.balance.remaining_minutes, 75);
    assert_eq!(retried.balance.remaining_minutes, 75);
    let usage_ledger: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM player_plan_ledger WHERE session_id=? AND reason='session_usage'",
    )
    .bind(started.session.id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(usage_ledger, 1, "heartbeat retry must not double charge");

    let ended = sessions
        .charge(
            started.session.id,
            50,
            Some((Utc::now(), 30, "voluntary".into())),
            None,
        )
        .await
        .unwrap();
    assert!(ended.session.end_time.is_some());
    assert_eq!(ended.balance.remaining_minutes, 70);
    assert_eq!(
        devices.find_by_id(device.id).await.unwrap().unwrap().status,
        "available"
    );
    let ended_payload: String = sqlx::query_scalar(
        "SELECT payload FROM outbox_events
         WHERE aggregate_id=? AND event_type='session.ended'
         ORDER BY sequence DESC LIMIT 1",
    )
    .bind(started.session.id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    let ended_payload: serde_json::Value = serde_json::from_str(&ended_payload).unwrap();
    assert_eq!(ended_payload["deviceName"], "PC-01");
    assert_eq!(ended_payload["walletMinutesAtStart"], 120);
    assert_eq!(ended_payload["remainingMinutes"], 70);
    assert_eq!(ended_payload["reason"], "voluntary");
    assert!(ended_payload["deductionProfile"].is_object());
    let outbox: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events WHERE aggregate_id IN (?,?,?)")
            .bind(device.id.to_string())
            .bind(balance.id.to_string())
            .bind(started.session.id.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert!(outbox >= 8);
    assert_eq!(
        sessions
            .list(&SessionFilterDto::default(), &[Uuid::now_v7()])
            .await
            .unwrap()
            .total,
        0
    );
    fixture.close().await;
}

#[tokio::test]
async fn session_service_activity_commits_locally_and_failure_rolls_back_the_session() {
    let fixture = Fixture::new().await;
    let device = fixture.device("PC-ACTIVITY").await;
    let balance = TenantBalanceRepository::new(fixture.db.clone())
        .purchase_or_recharge(
            &PurchaseBalanceDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: Some(fixture.transaction_id),
            },
            None,
        )
        .await
        .unwrap();
    // A closed pool fails immediately if the tenant service accidentally reaches PostgreSQL.
    let postgres = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    postgres.close().await;
    let cache = gaming_cafe_api::cache::create_cache(None).await;
    let events = EventService::new(Broadcaster::new(16));
    let outbox = OutboxService::new(postgres.clone());
    let notifications = NotificationService::new(postgres.clone(), outbox.clone(), cache.clone());
    let devices = DeviceService::new(
        postgres.clone(),
        events.clone(),
        outbox.clone(),
        notifications.clone(),
        cache.clone(),
    );
    let service = gaming_cafe_api::services::SessionService::new(
        postgres.clone(),
        devices,
        Arc::new(BalanceService::new(postgres, cache.clone())),
        events,
        outbox,
        notifications,
        "UTC".into(),
        cache,
    );
    // Use unrestricted wallet calendar to exercise the service on any test date.
    fixture.db.with_immediate_writer(move |c| Box::pin(async move {
        sqlx::query("UPDATE player_plan_balances SET allowed_days=NULL,allowed_months=NULL WHERE id=?")
            .bind(balance.id.to_string()).execute(c).await?; Ok(())
    })).await.unwrap();
    let started = tokio::time::timeout(
        Duration::from_secs(3),
        service.start_tenant(
            fixture.db.clone(),
            gaming_cafe_api::models::CreateSessionDto {
                balance_id: balance.id,
                device_id: device.id,
                shift_id: None,
                start_time: Some(Utc::now()),
            },
            fixture.player_id,
            None,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    let ended = tokio::time::timeout(
        Duration::from_secs(3),
        service.end_tenant(
            fixture.db.clone(),
            started.id,
            gaming_cafe_api::models::EndSessionDto::default(),
            None,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(ended.end_time.is_some());
    // Repeated end and heartbeat paths must not create duplicate activity.
    service
        .end_tenant(
            fixture.db.clone(),
            started.id,
            gaming_cafe_api::models::EndSessionDto::default(),
            None,
        )
        .await
        .unwrap();
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT kind,payload FROM activity_log WHERE entity_id=? ORDER BY kind")
            .bind(started.id.to_string())
            .fetch_all(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "session_ended");
    assert_eq!(rows[1].0, "session_started");
    for (_, payload) in rows {
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&payload).unwrap()["locationId"],
            fixture.location_id.to_string()
        );
    }
    fixture.db.with_immediate_writer(|c| Box::pin(async move {
        sqlx::query("CREATE TRIGGER reject_session_activity BEFORE INSERT ON activity_log WHEN NEW.kind='session_started' BEGIN SELECT RAISE(ABORT,'activity unavailable'); END")
            .execute(c).await?; Ok(())
    })).await.unwrap();
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    assert!(TenantSessionRepository::new(fixture.db.clone())
        .start(
            fixture.player_id,
            balance.id,
            device.id,
            fixture.location_id,
            None,
            Utc::now(),
            None,
            json!({})
        )
        .await
        .is_err());
    assert_eq!(
        TenantDeviceRepository::new(fixture.db.clone())
            .find_by_id(device.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "available"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM usage_sessions WHERE end_time IS NULL")
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_events")
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap(),
        before
    );
    fixture.close().await;
}

#[tokio::test]
async fn unique_open_sessions_and_lease_fencing_are_enforced() {
    let fixture = Fixture::new().await;
    let device = fixture.device("PC-02").await;
    let balance = TenantBalanceRepository::new(fixture.db.clone())
        .purchase_or_recharge(
            &PurchaseBalanceDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: Some(fixture.transaction_id),
            },
            None,
        )
        .await
        .unwrap();
    let sessions = TenantSessionRepository::new(fixture.db.clone());
    sessions
        .start(
            fixture.player_id,
            balance.id,
            device.id,
            fixture.location_id,
            None,
            Utc::now(),
            None,
            json!({}),
        )
        .await
        .unwrap();
    assert!(sessions
        .start(
            fixture.player_id,
            balance.id,
            device.id,
            fixture.location_id,
            None,
            Utc::now(),
            None,
            json!({}),
        )
        .await
        .is_err());
    fixture
        .lease
        .generations
        .write()
        .unwrap()
        .insert(fixture.tenant_id, 2);
    assert!(TenantDeviceRepository::new(fixture.db.clone())
        .update_status(device.id, "available")
        .await
        .is_err());
    fixture.close().await;
}

#[tokio::test]
async fn balance_service_tenant_path_does_not_touch_postgres() {
    let fixture = Fixture::new().await;
    let postgres = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let cache = gaming_cafe_api::cache::create_cache(None).await;
    let service = BalanceService::new(postgres, cache);
    let balance = service
        .purchase_or_recharge_tenant(
            fixture.db.clone(),
            PurchaseBalanceDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: Some(fixture.transaction_id),
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(balance.remaining_minutes, 120);
    fixture.close().await;
}

#[tokio::test]
async fn tenant_purchase_rejects_missing_or_foreign_transaction_without_mutation() {
    let fixture = Fixture::new().await;
    let repo = TenantBalanceRepository::new(fixture.db.clone());
    let before = tenant_row_counts(&fixture).await;
    let missing = repo
        .purchase_or_recharge(
            &PurchaseBalanceDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: None,
            },
            None,
        )
        .await;
    assert!(matches!(missing, Err(AppError::BadRequest(_))));
    let foreign = repo
        .purchase_or_recharge(
            &PurchaseBalanceDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: Some(Uuid::now_v7()),
            },
            None,
        )
        .await;
    assert!(matches!(foreign, Err(AppError::Conflict(_))));
    assert_eq!(tenant_row_counts(&fixture).await, before);
    fixture.close().await;
}

#[tokio::test]
async fn tenant_purchase_retry_returns_original_grant_without_recharging() {
    let fixture = Fixture::new().await;
    let repo = TenantBalanceRepository::new(fixture.db.clone());
    let dto = PurchaseBalanceDto {
        player_id: fixture.player_id,
        plan_id: fixture.plan_id,
        transaction_id: Some(fixture.transaction_id),
    };
    let first = repo.purchase_or_recharge(&dto, None).await.unwrap();
    let retried = repo.purchase_or_recharge(&dto, None).await.unwrap();
    assert_eq!(retried.id, first.id);
    assert_eq!(retried.remaining_minutes, first.remaining_minutes);
    let ledger_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM player_plan_ledger WHERE transaction_id=?")
            .bind(fixture.transaction_id.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(ledger_rows, 1);
    fixture.close().await;
}

#[tokio::test]
async fn device_service_refuses_to_delete_device_with_open_tenant_session() {
    let fixture = Fixture::new().await;
    let device = fixture.device("PC-IN-USE").await;
    let balance = TenantBalanceRepository::new(fixture.db.clone())
        .purchase_or_recharge(
            &PurchaseBalanceDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: Some(fixture.transaction_id),
            },
            None,
        )
        .await
        .unwrap();
    TenantSessionRepository::new(fixture.db.clone())
        .start(
            fixture.player_id,
            balance.id,
            device.id,
            fixture.location_id,
            None,
            Utc::now(),
            None,
            json!({}),
        )
        .await
        .unwrap();

    let postgres = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let cache = gaming_cafe_api::cache::create_cache(None).await;
    let outbox = OutboxService::new(postgres.clone());
    let service = DeviceService::new(
        postgres.clone(),
        EventService::new(Broadcaster::new(16)),
        outbox.clone(),
        NotificationService::new(postgres, outbox, cache.clone()),
        cache,
    );
    let result = service.delete_tenant(fixture.db.clone(), device.id).await;
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert!(TenantDeviceRepository::new(fixture.db.clone())
        .find_by_id(device.id)
        .await
        .unwrap()
        .is_some());
    fixture.close().await;
}

#[tokio::test]
async fn registered_mac_fingerprint_and_location_filters_use_tenant_storage() {
    let fixture = Fixture::new().await;
    let repo = TenantDeviceRepository::new(fixture.db.clone());
    let fingerprint = json!({"mac":"AA:BB:CC:DD","cpu":"old"}).to_string();
    let device = repo
        .provision(
            None,
            "Kiosk".into(),
            Some("serial".into()),
            "PC".into(),
            "HIGH_END_PCS".into(),
            None,
            fixture.location_id,
            fingerprint,
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        repo.find_registered_by_mac("aa:bb:cc:dd")
            .await
            .unwrap()
            .unwrap()
            .id,
        device.id
    );
    let refreshed = json!({"mac":"AA:BB:CC:DD","cpu":"new"}).to_string();
    repo.update_fingerprint(device.id, refreshed.clone())
        .await
        .unwrap();
    assert_eq!(
        repo.find_by_id(device.id)
            .await
            .unwrap()
            .unwrap()
            .registered_kiosk,
        Some(refreshed)
    );
    assert_eq!(
        repo.list(&DeviceFilterDto::default(), &[Uuid::now_v7()])
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        repo.list(&DeviceFilterDto::default(), &[fixture.location_id])
            .await
            .unwrap()
            .total,
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn player_plan_service_uses_tenant_storage_for_assignment_filter_update_and_expiry() {
    let fixture = Fixture::new().await;
    let postgres = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let service = PlayerPlanService::new(postgres);
    let assigned = service
        .assign_plan_to_player_tenant(
            fixture.db.clone(),
            AssignPlanDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: None,
                purchase_date: Some(Utc::now()),
            },
            None,
        )
        .await
        .unwrap();
    let listed = service
        .list_tenant(
            fixture.db.clone(),
            player_plan_filter(Some(fixture.player_id), Some(status::ACTIVE)),
        )
        .await
        .unwrap();
    assert_eq!(listed.total, 1);
    assert_eq!(listed.data[0].id, assigned.id);

    let deducted = service
        .deduct_time_credits_tenant(fixture.db.clone(), assigned.id, 20)
        .await
        .unwrap();
    assert_eq!(deducted.remaining_time_credits, Some(100));
    let exhausted = service
        .deduct_time_credits_tenant(fixture.db.clone(), assigned.id, 100)
        .await
        .unwrap();
    assert_eq!(exhausted.status, status::EXHAUSTED);

    TenantPlayerPlanRepository::new(fixture.db.clone())
        .update(
            assigned.id,
            &PlayerPlanUpdateValues {
                status: Some(status::ACTIVE.into()),
                remaining_time_credits: Some(5),
                remaining_usage_count: None,
                activation_date: None,
            },
            None,
        )
        .await
        .unwrap();
    let stale = service
        .assign_plan_to_player_tenant(
            fixture.db.clone(),
            AssignPlanDto {
                player_id: fixture.player_id,
                plan_id: fixture.plan_id,
                transaction_id: None,
                purchase_date: Some(Utc::now() - ChronoDuration::days(31)),
            },
            None,
        )
        .await
        .unwrap();
    let expired = service
        .list_tenant(fixture.db.clone(), player_plan_filter(None, None))
        .await
        .unwrap();
    assert_eq!(
        expired
            .data
            .iter()
            .find(|plan| plan.id == stale.id)
            .unwrap()
            .status,
        status::EXPIRED
    );
    fixture.close().await;
}

fn player_plan_filter(player_id: Option<Uuid>, plan_status: Option<&str>) -> PlayerPlanFilterDto {
    PlayerPlanFilterDto {
        player_id,
        plan_id: None,
        status: plan_status.map(str::to_owned),
        purchase_date_from: None,
        purchase_date_to: None,
        expiry_date_from: None,
        expiry_date_to: None,
        min_remaining_usage_count: None,
        min_remaining_time_credits: None,
        is_expired: None,
        page: Some(1),
        limit: Some(20),
        sort_by: Some("createdAt".into()),
        sort_order: Some("ASC".into()),
        device_type: None,
        device_sub_type: None,
    }
}

async fn tenant_row_counts(fixture: &Fixture) -> (i64, i64, i64) {
    let pool = fixture.db.read_pool().unwrap();
    let balances = sqlx::query_scalar("SELECT COUNT(*) FROM player_plan_balances")
        .fetch_one(&pool)
        .await
        .unwrap();
    let ledger = sqlx::query_scalar("SELECT COUNT(*) FROM player_plan_ledger")
        .fetch_one(&pool)
        .await
        .unwrap();
    let outbox = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    (balances, ledger, outbox)
}

struct Fixture {
    root: PathBuf,
    tenant_id: Uuid,
    location_id: Uuid,
    player_id: Uuid,
    plan_id: Uuid,
    transaction_id: Uuid,
    db: Arc<TenantDb>,
    lease: Arc<Lease>,
}

impl Fixture {
    async fn new() -> Self {
        let root = std::env::temp_dir().join(format!("arena360-venue-repo-{}", Uuid::now_v7()));
        let tenant_id = Uuid::now_v7();
        let path = tenant_path(&root, tenant_id);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .unwrap();
        gaming_cafe_api::tenancy::migrate(&pool).await.unwrap();
        sqlx::query("INSERT INTO tenant_runtime(singleton,timezone) VALUES(1,'Asia/Kolkata')")
            .execute(&pool)
            .await
            .unwrap();
        let at = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
        let location_id = Uuid::now_v7();
        let player_id = Uuid::now_v7();
        let plan_id = Uuid::now_v7();
        let transaction_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,?,?,?,?)",
        )
        .bind(location_id.to_string())
        .bind("alpha")
        .bind("Alpha")
        .bind(&at)
        .bind(&at)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO users(id,username,password_hash,role,created_at,updated_at) VALUES(?,?,'hash','player',?,?)")
            .bind(player_id.to_string()).bind("player").bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO plans(id,name,price,plan_type,validity_days,time_credits,is_active,device_type,device_sub_type,allowed_days,allowed_months,dynamic_deduction_enabled,deduction_profile,created_at,updated_at) VALUES(?,? ,10000,'time_based',30,120,1,'PC','HIGH_END_PCS',?,?,1,?,?,?)")
            .bind(plan_id.to_string()).bind("Two hours")
            .bind(json!(["monday", "tuesday"]).to_string())
            .bind(json!([1, 10, 12]).to_string())
            .bind(json!({
                "peakWindowStart":"00:00:00",
                "peakWindowEnd":"23:59:59",
                "peakRatio":1.5,
                "lowWindowStart":"00:00:00",
                "lowWindowEnd":"00:00:00",
                "lowRatio":1.0
            }).to_string())
            .bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO transactions(id,player_id,plan_id,location_id,transaction_type,amount,paid_amount,cash_amount,payment_method,payment_status,transaction_date,created_at,updated_at) VALUES(?,?,?,?, 'plan_purchase',10000,10000,10000,'cash','completed',?,?,?)")
            .bind(transaction_id.to_string()).bind(player_id.to_string()).bind(plan_id.to_string()).bind(location_id.to_string()).bind(&at).bind(&at).bind(&at).execute(&pool).await.unwrap();
        pool.close().await;
        let lease = Arc::new(Lease::default());
        lease.generations.write().unwrap().insert(tenant_id, 1);
        let manager = TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                read_connections: 2,
                busy_timeout: Duration::from_millis(250),
                idle_timeout: Duration::from_secs(60),
                reaper_interval: Duration::from_secs(1),
            },
            lease.clone(),
        )
        .unwrap();
        let db = manager.open(tenant_id).await.unwrap();
        Self {
            root,
            tenant_id,
            location_id,
            player_id,
            plan_id,
            transaction_id,
            db,
            lease,
        }
    }

    async fn device(&self, name: &str) -> gaming_cafe_api::models::Device {
        TenantDeviceRepository::new(self.db.clone())
            .create(
                &CreateDeviceDto {
                    name: name.into(),
                    location_id: Some(self.location_id),
                    serial_number: Some(name.into()),
                    local_ip_address: None,
                    device_type: Some("PC".into()),
                    device_sub_type: Some("HIGH_END_PCS".into()),
                    location: None,
                    status: None,
                    registration_status: Some("registered".into()),
                },
                None,
            )
            .await
            .unwrap()
    }

    async fn close(self) {
        self.db.close().await.unwrap();
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}
