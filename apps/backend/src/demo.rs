//! Demo provisioning and operational writes use the same fenced commands as the API.
use crate::{
    cache::{CacheService, NoopCache},
    control::{CreateTenant, LeaseClient, LeaseConfig, Repository},
    error::AppError,
    models::*,
    repositories::*,
    services::*,
    tenancy::{
        NewOutboxEvent, PostgresProvisioningControl, ProvisionTenant, TenantDb, TenantDbConfig,
        TenantDbManager, TenantProvisioner,
    },
};
use chrono::{Duration, NaiveDate, TimeZone, Utc};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use uuid::Uuid;

pub const VERSION: &str = "arena360-demo-v2";
#[derive(Clone)]
pub struct DemoOptions {
    pub slug: String,
    pub date: NaiveDate,
    pub root: PathBuf,
    pub cell_id: Uuid,
    pub owner_user_id: Option<Uuid>,
    pub player_password_hash: String,
}
fn dto<T: DeserializeOwned>(v: Value) -> Result<T, AppError> {
    serde_json::from_value(v).map_err(|e| AppError::Internal(format!("Demo command: {e}")))
}
pub fn validate(slug: &str, date: NaiveDate) -> Result<(), AppError> {
    if !slug.starts_with("arena360-demo")
        || slug.len() > 40
        || !slug
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(AppError::BadRequest("Demo slug must start with arena360-demo and use at most 40 lowercase letters, digits or hyphens".into()));
    }
    if date > Utc::now().date_naive() {
        return Err(AppError::BadRequest(
            "Demo date must not be in the future".into(),
        ));
    }
    Ok(())
}
pub fn preview(slug: &str, date: NaiveDate) -> Value {
    json!({"status":"planned","version":VERSION,"tenantSlug":slug,"date":date.to_string(),"counts":{"players":32,"devices":12,"plans":3,"products":5,"venues":1}})
}

pub async fn seed(pool: PgPool, opts: DemoOptions) -> Result<Value, AppError> {
    validate(&opts.slug, opts.date)?;
    let leases = Arc::new(LeaseClient::new(
        pool.clone(),
        opts.cell_id,
        LeaseConfig::default(),
    )?);
    let repo = Repository::new(pool.clone());
    let tenant = if let Some(tenant) = repo.tenant_by_slug(&opts.slug).await? {
        if tenant.owner_cell != Some(opts.cell_id) || tenant.state != "ACTIVE" {
            return Err(AppError::Conflict(
                "Demo tenant must be ACTIVE on the selected cell".into(),
            ));
        }
        leases.acquire_assigned(tenant.id).await?;
        tenant
    } else {
        TenantProvisioner::new(
            opts.root.clone(),
            Arc::new(PostgresProvisioningControl::new(repo, leases.clone())),
        )
        .provision(ProvisionTenant {
            tenant: CreateTenant {
                slug: opts.slug.clone(),
                name: "Arena360 demo".into(),
                timezone: "UTC".into(),
                owner_cell: Some(opts.cell_id),
                subscription_plan: "trial".into(),
                entitlements: json!({}),
                trial_ends_at: Utc::now() + Duration::days(30),
                entitlement_grace_until: Utc::now() + Duration::days(37),
            },
            settings: vec![],
        })
        .await?
        .tenant
    };
    let manager = TenantDbManager::new(
        TenantDbConfig {
            root: opts.root.clone(),
            ..Default::default()
        },
        leases.clone(),
    )?;
    let db = manager.open(tenant.id).await?;
    let renew = leases.spawn_renewal();
    let result = seed_open(pool, db.clone(), &opts).await;
    renew.abort();
    db.close().await?;
    result
}
async fn seed_open(pool: PgPool, db: Arc<TenantDb>, opts: &DemoOptions) -> Result<Value, AppError> {
    let prior: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT state,summary FROM demo_seed_runs WHERE version=?")
            .bind(VERSION)
            .fetch_optional(&db.read_pool()?)
            .await?;
    if let Some((state, summary)) = prior {
        if state != "COMPLETE" {
            return Err(AppError::Conflict("This demo seed was interrupted. Choose a new demo slug; existing data is preserved".into()));
        }
        let mut value: Value = serde_json::from_str(
            &summary.ok_or_else(|| AppError::Internal("Missing completed demo summary".into()))?,
        )
        .map_err(|e| AppError::Internal(e.to_string()))?;
        value["status"] = json!("already-seeded");
        return Ok(value);
    }
    let occupied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE role='player') OR EXISTS(SELECT 1 FROM devices) OR EXISTS(SELECT 1 FROM products)").fetch_one(&db.read_pool()?).await?;
    if occupied {
        return Err(AppError::Conflict(
            "Demo target contains operational data. Choose a new demo slug".into(),
        ));
    }
    let date = opts.date.to_string();
    db.with_immediate_writer(move|c|Box::pin(async move {
        sqlx::query("INSERT INTO demo_seed_runs(version,state,requested_date,started_at) VALUES(?,'RUNNING',?,?)").bind(VERSION).bind(date).bind(crate::time::format_sqlite_timestamp(&Utc::now()).map_err(|e|AppError::Internal(e.to_string()))?).execute(c).await?;Ok(())
    })).await?;
    let (owner, staff) = identities(&pool, db.tenant_id(), opts).await?;
    crate::control::staff_projection::sync_tenant(&pool, db.clone()).await?;
    let mut result = populate(db.clone(), opts, owner, staff).await?;
    crate::control::staff_projection::sync_tenant(&pool, db.clone()).await?;
    // Include the final identity projection and the completion event in the summary.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&db.read_pool()?)
        .await?;
    result["counts"]["outbox_events"] = json!(count + 1);
    result["status"] = json!("seeded");
    result["version"] = json!(VERSION);
    let payload = result.clone();
    let summary = result.to_string();
    let tenant = db.tenant_id();
    db.with_immediate_writer(move|c|Box::pin(async move {
        sqlx::query("UPDATE demo_seed_runs SET state='COMPLETE',summary=?,completed_at=? WHERE version=? AND state='RUNNING'").bind(summary).bind(crate::time::format_sqlite_timestamp(&Utc::now()).map_err(|e|AppError::Internal(e.to_string()))?).bind(VERSION).execute(&mut *c).await?;
        crate::tenancy::write_outbox_event_on_connection(c,NewOutboxEvent {aggregate_type:"tenant".into(),aggregate_id:tenant,event_type:"tenant.demo_seeded".into(),schema_version:1,location_id:None,deleted:false,payload}).await?;Ok(())
    })).await?;
    Ok(result)
}

/// Global demo staff credentials stay disabled. An explicitly selected existing owner is preserved.
async fn identities(
    pool: &PgPool,
    tenant: Uuid,
    opts: &DemoOptions,
) -> Result<(Uuid, Uuid), AppError> {
    let mut tx = pool.begin().await?;
    let owner = if let Some(user) = opts.owner_user_id {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id=$1 AND is_active AND deleted_at IS NULL)",
        )
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
        if !valid {
            return Err(AppError::BadRequest(
                "Demo owner must be an existing active control-plane user".into(),
            ));
        }
        user
    } else {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name) VALUES($1,$2,'!demo-login-disabled!','Demo','Owner')").bind(id).bind(format!("{}.owner",opts.slug)).execute(&mut *tx).await?;
        id
    };
    let staff = Uuid::now_v7();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name) VALUES($1,$2,'!demo-login-disabled!','Demo','Counter')").bind(staff).bind(format!("{}.counter",opts.slug)).execute(&mut *tx).await?;
    for (user, role) in [(owner, "admin"), (staff, "staff")] {
        sqlx::query("INSERT INTO organization_memberships(tenant_id,user_id,role,is_active) VALUES($1,$2,$3,true)").bind(tenant).bind(user).bind(role).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok((owner, staff))
}

async fn populate(
    db: Arc<TenantDb>,
    opts: &DemoOptions,
    owner: Uuid,
    staff: Uuid,
) -> Result<Value, AppError> {
    let venue = TenantSettingsRepository::new(db.clone())
        .save_location(
            db.tenant_id(),
            None,
            dto(
                json!({"slug":"main","name":"Demo gaming floor","timezone":"UTC","currency":"INR"}),
            )?,
        )
        .await?
        .id;
    let staff_role:Uuid=sqlx::query_scalar("SELECT unhex(replace(id,'-','')) FROM access_roles WHERE system_key='staff' AND is_template=0").fetch_one(&db.read_pool()?).await?;
    TenantAccessRepository::new(db.clone()).save_member_assignments(staff,dto(json!({"roleIds":[staff_role],"locationIds":[venue],"active":true,"expectedRevision":0}))?,owner).await?;
    let cache: Arc<dyn CacheService> = Arc::new(NoopCache);
    let config = Arc::new(ConfigService::new(cache.clone(), "UTC".into()));
    let events = EventService::new(crate::sse::Broadcaster::new(16));
    let devices = DeviceService::new(events.clone(), cache.clone());
    let balances = Arc::new(BalanceService::new(cache.clone()));
    let sessions = SessionService::new(
        devices.clone(),
        balances,
        events,
        config.clone(),
        PricingPolicyService::new(),
        cache,
    );
    let plans = PlanService::new(config.clone());
    let mut player_ids = Vec::new();
    for i in 0..32 {
        let id = TenantUserRepository::new(db.clone())
            .create_player(TenantCreatePlayer {
                username: format!("demo.player.{:02}", i + 1),
                password_hash: opts.player_password_hash.clone(),
                phone_number: format!("90000{:05}", i),
                first_name: Some(format!("Demo {}", i + 1)),
                last_name: Some("Player".into()),
                actor_id: Some(owner),
            })
            .await?
            .id;
        CreditService::new()
            .set_limit_tenant(
                db.clone(),
                id,
                dto(json!({"creditLimit":5000}))?,
                Some(owner),
            )
            .await?;
        player_ids.push(id);
    }
    let mut plan_ids = Vec::new();
    for (name, minutes, price) in [
        ("Quick Play", 60, 120),
        ("Ranked Duo", 120, 220),
        ("Squad Marathon", 180, 300),
    ] {
        plan_ids.push(plans.create_tenant(db.clone(),vec![venue],dto(json!({"name":format!("DEMO {name}"),"price":price,"planType":"time_based","validityDays":365,"timeCredits":minutes,"deviceType":"PC","deviceSubType":"HIGH_END_PCS"}))?,Some(owner)).await?.id);
    }
    let mut device_ids = Vec::new();
    for i in 0..12 {
        device_ids.push(devices.create_tenant(db.clone(),dto(json!({"name":format!("DEMO PC {:02}",i+1),"locationId":venue,"deviceType":"PC","deviceSubType":"HIGH_END_PCS","registrationStatus":"registered","status":if i==11 {"under_maintenance"}else{"available"}}))?,Some(owner)).await?.id);
    }
    let inventory = TenantInventoryRepository::new(db.clone());
    let store = inventory
        .create_location(
            &dto(json!({"name":"Demo counter store","kind":"store","venueLocationId":venue}))?,
            Some(owner),
        )
        .await?
        .id;
    let vendor = TenantVendorRepository::new(db.clone())
        .create(&dto(json!({"name":"Demo supplies"}))?, Some(owner))
        .await?
        .id;
    let products = ProductService::new();
    let mut product_ids = Vec::new();
    for (name, price, category) in [
        ("Cola", 60, "beverage"),
        ("Energy Drink", 140, "beverage"),
        ("Chips", 50, "snack"),
        ("Sandwich", 120, "meal"),
        ("Water", 30, "beverage"),
    ] {
        let p=products.create_tenant(db.clone(),vec![venue],dto(json!({"name":format!("DEMO {name}"),"price":price,"category":category,"unitsPerPurchaseUnit":12}))?,Some(owner)).await?;
        inventory.create_receipt(&dto(json!({"locationId":store,"vendorId":vendor,"exceptionalReason":"Demo opening stock","lines":[{"productId":p.id,"boxQuantity":100}]}))?,Some(staff)).await?;
        product_ids.push(p.id);
    }
    let kitchen = TenantKitchenService::new(db.clone());
    kitchen
        .save_menu(product_ids[3], true, "Demo hot kitchen", 10, 0, owner)
        .await?;
    let shifts = TenantShiftRepository::new(db.clone());
    let shift = shifts
        .start_confirmed(
            staff,
            dto(json!({"openingBalance":1000,"venueLocationId":venue}))?,
            staff,
        )
        .await?;
    let shift_id = shift.shift.id;
    let transactions = TenantTransactionRepository::new(db.clone());
    let mut wallet_ids = Vec::new();
    for player in &player_ids {
        for _ in 0..5 {
            transactions.create(dto(json!({"playerId":player,"transactionType":"plan_purchase","planId":plan_ids[2],"shiftId":shift_id,"venueLocationId":venue,"paymentMethod":"online","paymentStatus":"completed","onlinePaymentRefLast4":"1234","transactionDate":Utc::now()}))?,Some(staff)).await?;
        }
        wallet_ids.push(
            TenantBalanceRepository::new(db.clone())
                .list(&dto(json!({"playerId":player}))?)
                .await?
                .data[0]
                .id,
        );
    }
    let end = Utc.from_utc_datetime(
        &opts
            .date
            .and_hms_opt(12, 0, 0)
            .ok_or_else(|| AppError::BadRequest("Invalid date".into()))?,
    );
    for day in 0..60_usize {
        let at = (end - Duration::days((59 - day) as i64)).min(Utc::now() - Duration::minutes(60));
        for n in 0..if day < 30 { 3 } else { 6 } {
            let p = (day + n) % 32;
            let method = if (day + n) % 4 == 0 {
                "credit"
            } else if (day + n) % 2 == 0 {
                "online"
            } else {
                "cash"
            };
            let sale=transactions.create(dto(json!({"playerId":player_ids[p],"transactionType":"product_purchase","shiftId":shift_id,"venueLocationId":venue,"saleLocationId":store,"paymentMethod":method,"paymentStatus":if method=="credit" {"credit"}else{"completed"},"onlinePaymentRefLast4":if method=="online" {Some("1234")}else{None},"transactionDate":at,"lineItems":[{"productId":product_ids[(day+n)%5],"quantity":2}]}))?,Some(staff)).await?;
            if method == "credit" && day % 3 == 0 {
                CreditService::new().settle_tenant(db.clone(),dto(json!({"playerId":player_ids[p],"items":[{"transactionId":sale.id,"amount":sale.amount/2.0}],"paymentMethod":"cash","cashAmount":sale.amount/2.0}))?,shift_id,staff).await?;
            }
            let session = sessions
                .start_tenant(
                    db.clone(),
                    CreateSessionDto {
                        balance_id: wallet_ids[p],
                        device_id: device_ids[n],
                        shift_id: None,
                        start_time: Some(at),
                    },
                    player_ids[p],
                    Some(staff),
                )
                .await?;
            sessions
                .end_tenant(
                    db.clone(),
                    session.id,
                    dto(json!({"reason":"voluntary","endTime":at+Duration::minutes(30)}))?,
                    Some(staff),
                )
                .await?;
        }
    }
    let category = TenantExpenseCategoryRepository::new(db.clone())
        .create(&dto(json!({"name":"Demo operating costs"}))?, Some(owner))
        .await?
        .id;
    let expenses = TenantExpenseRepository::new(db.clone());
    for day in [0, 20, 40, 59] {
        let expense=expenses.create_at(&dto(json!({"categoryId":category,"vendorId":vendor,"amount":250,"paymentMethod":"online","paymentAccount":"Demo bank","description":"Demo operating expense","expenseDate":end-Duration::days(59-day),"shiftId":shift_id}))?,Some(staff),Some(venue)).await?;
        if day != 59 {
            expenses.approve(expense.id, owner).await?;
        }
    }
    let register = shift.cash_register.id;
    let deposit=TenantCashDepositRepository::new(db.clone()).create(&dto(json!({"cashRegisterId":register,"shiftId":shift_id,"amount":500,"denominations":{"500":1},"notes":"Demo bank deposit"}))?,staff).await?;
    TenantCashDepositRepository::new(db.clone())
        .approve(deposit.id, "bank", owner)
        .await?;
    // Closure variance accounts for approved deposits separately; carry-forward removes them.
    let closing = TenantCashRegisterRepository::new(db.clone())
        .get_expected_closing(register)
        .await?;
    shifts
        .close_with_register(
            shift_id,
            dto(json!({"closingBalance":closing + deposit.amount}))?,
            staff,
        )
        .await?;
    shifts
        .start_confirmed(
            staff,
            dto(json!({"openingBalance":closing,"venueLocationId":venue}))?,
            staff,
        )
        .await?;
    let procurement = TenantProcurementService::new(db.clone(), "UTC".into());
    let order=procurement.create_order(dto(json!({"vendorId":vendor,"destinationLocationId":store,"lines":[{"productId":product_ids[0],"orderedBoxes":4,"boxCost":300,"taxRate":5}]}))?,owner).await?;
    procurement
        .transition(order.order.id, "submit", None, owner)
        .await?;
    procurement
        .transition(order.order.id, "approve", None, owner)
        .await?;
    procurement.receive(order.order.id,dto(json!({"invoiceReference":"DEMO-INV-1","paymentMethod":"online","paymentAccount":"Demo bank","lines":[{"purchaseOrderLineId":order.lines[0].id,"acceptedBoxes":2,"rejectedBoxes":1}]}))?,owner).await?;
    for product in &product_ids {
        procurement.upsert_reorder_rule(dto(json!({"locationId":store,"productId":product,"minimumPieces":1200,"targetPieces":2400,"preferredVendorId":vendor}))?,owner).await?;
    }
    let mut active = Vec::new();
    for i in 0..5 {
        active.push(
            sessions
                .start_tenant(
                    db.clone(),
                    CreateSessionDto {
                        balance_id: wallet_ids[i],
                        device_id: device_ids[i],
                        shift_id: None,
                        start_time: None,
                    },
                    player_ids[i],
                    Some(staff),
                )
                .await?
                .id,
        );
    }
    for i in 3..5 {
        TenantKioskOrderRepository::new(db.clone()).place(player_ids[i],device_ids[i],dto(json!({"lineItems":[{"productId":product_ids[i],"quantity":1}],"note":"Demo player order"}))?).await?;
    }
    let mut counts = BTreeMap::new();
    for table in [
        "users",
        "devices",
        "plans",
        "products",
        "transactions",
        "usage_sessions",
        "player_plan_ledger",
        "stock_movements",
        "shifts",
        "cash_registers",
        "expenses",
        "credit_settlements",
        "purchase_orders",
        "kitchen_tickets",
        "kiosk_orders",
        "outbox_events",
    ] {
        counts.insert(
            table,
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&db.read_pool()?)
                .await?,
        );
    }
    Ok(
        json!({"tenantId":db.tenant_id(),"tenantSlug":opts.slug,"date":opts.date.to_string(),"ownerUserId":owner,"staffUserId":staff,"venueId":venue,"deviceIds":device_ids,"playerIds":player_ids,"activeSessionIds":active,"playersCanLogin":opts.player_password_hash.starts_with("$2"),"counts":counts}),
    )
}
