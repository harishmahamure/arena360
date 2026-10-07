use chrono::Utc;
use gaming_cafe_api::{
    error::AppError,
    models::*,
    repositories::{
        TenantCashDepositRepository, TenantCashRegisterRepository, TenantExpenseCategoryRepository,
        TenantExpenseRepository, TenantShiftRepository,
    },
    tenancy::{tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease},
};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};
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

struct Fixture {
    root: PathBuf,
    db: Arc<TenantDb>,
    lease: Arc<Lease>,
}

impl Fixture {
    async fn new() -> Self {
        let root = std::env::temp_dir().join(format!("arena360-catalog-repo-{}", Uuid::now_v7()));
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
        Self { root, db, lease }
    }

    async fn location(&self, slug: &str) -> Uuid {
        let id = Uuid::now_v7();
        let slug = slug.to_owned();
        let timestamp = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
        self.db
            .with_writer(|connection| {
                Box::pin(async move {
                    sqlx::query("INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES (?,?,?,?,?)")
                        .bind(id.to_string())
                        .bind(&slug)
                        .bind(&slug)
                        .bind(&timestamp)
                        .bind(&timestamp)
                        .execute(connection)
                        .await?;
                    Ok(())
                })
            })
            .await
            .unwrap();
        id
    }

    async fn outbox_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
            .fetch_one(&self.db.read_pool().unwrap())
            .await
            .unwrap()
    }

    async fn close(self) {
        self.db.close().await.unwrap();
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}

impl Fixture {
    async fn staff(&self) -> Uuid {
        let id = Uuid::now_v7();
        self.db.with_immediate_writer(move|c|Box::pin(async move{
   let ts=gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
   sqlx::query("INSERT INTO users(id,username,role,created_at,updated_at) VALUES(?,?,'staff',?,?)").bind(id.to_string()).bind(id.to_string()).bind(&ts).bind(&ts).execute(c).await?;Ok(())
  })).await.unwrap();
        id
    }
    async fn scalar(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.db.read_pool().unwrap())
            .await
            .unwrap()
    }
}

fn dto<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> T {
    serde_json::from_value(v).unwrap()
}
#[tokio::test]
async fn concurrent_start_and_handover_are_atomic() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let other = f.location("other").await;
    let staff = f.staff().await;
    let incoming = f.staff().await;
    let shifts = TenantShiftRepository::new(f.db.clone());
    let start: StartShiftDto = dto(json!({"openingBalance":100.0001,"venueLocationId":venue}));
    let (a, b) = tokio::join!(
        shifts.start_confirmed(staff, start.clone(), staff),
        shifts.start_confirmed(staff, start, staff)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.shift.id, b.shift.id);
    assert_ne!(a.resumed, b.resumed);
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM activity_log WHERE kind='shift_clock_in'")
            .await,
        1
    );
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM activity_log WHERE kind='cash_register_opened'")
            .await,
        1
    );
    let activity: (String, String, String) = sqlx::query_as(
        "SELECT actor_user_id,location_id,payload FROM activity_log WHERE kind='shift_clock_in'",
    )
    .fetch_one(&f.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(activity.0, staff.to_string());
    assert_eq!(activity.1, venue.to_string());
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&activity.2).unwrap()["shiftId"],
        a.shift.id.to_string()
    );

    assert!(shifts
        .start_confirmed(
            staff,
            dto(json!({"openingBalance":0,"venueLocationId":other})),
            staff
        )
        .await
        .is_err());
    let occupied = shifts
        .start_confirmed(
            incoming,
            dto(json!({"openingBalance":1,"venueLocationId":other})),
            incoming,
        )
        .await
        .unwrap();
    let close: ShiftCloseDto =
        dto(json!({"closingBalance":100.0001,"deposit":{"amount":20,"denominations":{"20":1}}}));
    let count = f.outbox_count().await;
    assert!(shifts
        .handover(a.shift.id, incoming, close.clone(), staff)
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, count);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM cash_deposits").await, 0);
    assert_eq!(
        shifts.find_by_id(a.shift.id).await.unwrap().unwrap().status,
        "active"
    );
    shifts
        .force_close(occupied.shift.id, incoming)
        .await
        .unwrap();
    assert_eq!(
        shifts
            .find_by_id(occupied.shift.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "force_closed"
    );
    let (closed, next, register) = shifts
        .handover(a.shift.id, incoming, close, staff)
        .await
        .unwrap();
    assert_eq!(closed.closedShift.status, "completed");
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM activity_log WHERE kind='shift_handover'")
            .await,
        1
    );
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM activity_log WHERE kind='cash_deposit_initiated'")
            .await,
        1
    );
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM activity_log WHERE kind='approval_requested'")
            .await,
        1
    );

    let approval_payload: String =
        sqlx::query_scalar("SELECT payload FROM activity_log WHERE kind='approval_requested'")
            .fetch_one(&f.db.read_pool().unwrap())
            .await
            .unwrap();
    let approval: serde_json::Value = serde_json::from_str(&approval_payload).unwrap();
    assert_eq!(approval["amount"], 20.0);
    assert_eq!(approval["staff_id"], staff.to_string());
    assert_eq!(approval["entity_type"], "cash_deposit");

    assert_eq!(register.opening_balance, 80.0001);
    assert_eq!(
        f.scalar(&format!(
            "SELECT COUNT(*) FROM shifts WHERE id='{}' AND location_id='{}'",
            next.id, venue
        ))
        .await,
        1
    );
    assert_eq!(
        shifts
            .list(&dto(json!({"status":"completed"})))
            .await
            .unwrap()
            .total,
        1
    );
    let cash = TenantCashRegisterRepository::new(f.db.clone());
    assert_eq!(
        cash.preview_carry_forward_balance_for(other).await.unwrap(),
        1.0
    );
    assert_eq!(
        cash.preview_carry_forward_balance_for(venue).await.unwrap(),
        80.0001
    );
    f.close().await;
}
#[tokio::test]
async fn deposit_decisions_after_closure_do_not_create_phantom_cash() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let staff = f.staff().await;
    let admin = f.staff().await;
    let shifts = TenantShiftRepository::new(f.db.clone());
    let cash = TenantCashRegisterRepository::new(f.db.clone());
    let deposits = TenantCashDepositRepository::new(f.db.clone());
    let start = shifts
        .start_confirmed(
            staff,
            dto(json!({"openingBalance":660,"venueLocationId":venue})),
            staff,
        )
        .await
        .unwrap();
    cash.add_entry(
        start.cash_register.id,
        &dto(json!({"entryType":"cash_in","amount":35})),
        staff,
    )
    .await
    .unwrap();
    let dep=deposits.create(&dto(json!({"cashRegisterId":start.cash_register.id,"shiftId":start.shift.id,"amount":500,"denominations":{"500":1}})),staff).await.unwrap();
    assert_eq!(
        cash.get_expected_closing(start.cash_register.id)
            .await
            .unwrap(),
        695.0
    );
    let closed = cash
        .close_register(
            start.cash_register.id,
            &dto(json!({"closingBalance":1195})),
            staff,
        )
        .await
        .unwrap();
    assert_eq!(closed.variance, Some(500.0));
    deposits.approve(dep.id, "bank", admin).await.unwrap();
    let updated = cash.get_by_id(start.cash_register.id).await.unwrap();
    assert_eq!(updated.register.variance, Some(0.0));
    assert_eq!(updated.register.total_deposited, Some(500.0));
    assert!(deposits.reject(dep.id, "duplicate", admin).await.is_err());
    cash.reconcile(start.cash_register.id, Some("counted".into()), admin)
        .await
        .unwrap();
    shifts.close(start.shift.id, None, staff).await.unwrap();
    let second = shifts
        .start_confirmed(
            staff,
            dto(json!({"openingBalance":100,"venueLocationId":venue})),
            staff,
        )
        .await
        .unwrap();
    let dep=deposits.create(&dto(json!({"cashRegisterId":second.cash_register.id,"shiftId":second.shift.id,"amount":20,"denominations":{}})),staff).await.unwrap();
    cash.close_register(
        second.cash_register.id,
        &dto(json!({"closingBalance":100})),
        staff,
    )
    .await
    .unwrap();
    deposits.reject(dep.id, "returned", admin).await.unwrap();
    assert_eq!(
        cash.get_expected_closing(second.cash_register.id)
            .await
            .unwrap(),
        100.0
    );
    assert_eq!(
        cash.find_by_id(second.cash_register.id)
            .await
            .unwrap()
            .unwrap()
            .variance,
        Some(0.0)
    );
    assert_eq!(
        cash.preview_carry_forward_balance_for(venue).await.unwrap(),
        100.0
    );
    assert_eq!(
        cash.list_entries(second.cash_register.id)
            .await
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        deposits
            .list(&dto(json!({"status":"rejected","sortBy":"amount"})))
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        cash.list(&CashRegisterFilterDto::default())
            .await
            .unwrap()
            .total,
        2
    );
    f.close().await;
}
#[tokio::test]
async fn expense_approval_requires_register_and_one_cash_entry() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let staff = f.staff().await;
    let categories = TenantExpenseCategoryRepository::new(f.db.clone());
    let expenses = TenantExpenseRepository::new(f.db.clone());
    let category = categories
        .create(
            &dto(json!({"name":"Repairs","budgetAmount":100.0001})),
            Some(staff),
        )
        .await
        .unwrap();
    let child = categories
        .create(
            &dto(json!({"name":"Parts","parentId":category.id})),
            Some(staff),
        )
        .await
        .unwrap();
    assert!(categories
        .update(category.id, &dto(json!({"parentId":child.id})), Some(staff))
        .await
        .is_err());
    assert!(categories
        .create(&dto(json!({"name":"repairs"})), Some(staff))
        .await
        .is_err());
    let expense = expenses
        .create(
            &dto(json!({"categoryId":category.id,"amount":10.0001,"paymentMethod":"cash"})),
            Some(staff),
        )
        .await
        .unwrap();
    let before = f.outbox_count().await;
    assert!(expenses.approve(expense.id, staff).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(
        expenses
            .find_by_id(expense.id)
            .await
            .unwrap()
            .unwrap()
            .approval_status,
        "pending"
    );
    let shifts = TenantShiftRepository::new(f.db.clone());
    let start = shifts
        .start_confirmed(
            staff,
            dto(json!({"openingBalance":100,"venueLocationId":venue})),
            staff,
        )
        .await
        .unwrap();
    let (a, b) = tokio::join!(
        expenses.approve(expense.id, staff),
        expenses.approve(expense.id, staff)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let approved = expenses.find_by_id(expense.id).await.unwrap().unwrap();
    assert_eq!(approved.shift_id, Some(start.shift.id));
    assert!(approved.cash_register_entry_id.is_some());
    assert_eq!(
        f.scalar("SELECT amount FROM cash_register_entries WHERE reference_type='expense'")
            .await,
        100001
    );
    assert!(expenses
        .update(expense.id, &dto(json!({"amount":99})), Some(staff))
        .await
        .is_err());
    let cash = TenantCashRegisterRepository::new(f.db.clone());
    assert_eq!(
        cash.get_expected_closing(start.cash_register.id)
            .await
            .unwrap(),
        89.9999
    );
    assert_eq!(expenses.list(&dto(json!({"approvalStatus":"approved","minAmount":10,"dateFrom":"2000-01-01T00:00:00Z"}))).await.unwrap().total,1);
    expenses.soft_delete(expense.id).await.unwrap();
    assert!(expenses.find_by_id(expense.id).await.unwrap().is_none());
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM cash_register_entries").await,
        1
    );
    assert_eq!(
        categories
            .list(&ExpenseCategoryFilterDto::default())
            .await
            .unwrap()
            .total,
        2
    );
    let other = Fixture::new().await;
    assert!(TenantExpenseCategoryRepository::new(other.db.clone())
        .find_by_id(category.id)
        .await
        .unwrap()
        .is_none());
    other.close().await;
    f.close().await;
}
#[tokio::test]
async fn invalid_money_and_fencing_leave_no_records() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let staff = f.staff().await;
    let shifts = TenantShiftRepository::new(f.db.clone());
    for amount in [-1.0, f64::NAN, f64::INFINITY, 0.00001] {
        assert!(shifts
            .start_confirmed(
                staff,
                StartShiftDto {
                    opening_balance: amount,
                    opening_denominations: None,
                    notes: None,
                    venue_location_id: Some(venue)
                },
                staff
            )
            .await
            .is_err());
    }
    assert_eq!(f.scalar("SELECT COUNT(*) FROM shifts").await, 0);
    let before = f.outbox_count().await;
    f.lease.generations.write().unwrap().clear();
    assert!(shifts
        .start_confirmed(
            staff,
            dto(json!({"openingBalance":0,"venueLocationId":venue})),
            staff
        )
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM cash_registers").await, 0);
    f.close().await;
}

#[tokio::test]
async fn financial_lists_and_cash_approval_keep_the_actual_venue() {
    use gaming_cafe_api::{access::scope::LocationScope, dto::JwtUserClaims};
    let f = Fixture::new().await;
    let venue = f.location("allowed").await;
    let other = f.location("elsewhere").await;
    let staff = f.staff().await;
    let outsider = f.staff().await;
    let role = Uuid::now_v7();
    f.db.with_immediate_writer(move |c| Box::pin(async move {
        let at = gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
        sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) VALUES(?,'Finance','[\"cash-registers:read\",\"cash-registers:write\",\"cash-deposits:read\",\"cash-deposits:approve\",\"expenses:read\",\"expenses:write\",\"expenses:approve\"]',?,?)")
            .bind(role.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)")
            .bind(staff.to_string()).bind(role.to_string()).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO location_role_assignments(user_id,location_id,role_id,created_at) VALUES(?,?,?,?)")
            .bind(staff.to_string()).bind(venue.to_string()).bind(role.to_string()).bind(&at).execute(c).await?;
        Ok(())
    })).await.unwrap();
    let claims: JwtUserClaims = dto(
        json!({"sub":staff,"userId":staff,"tenantId":f.db.tenant_id(),
        "roles":["staff"],"permissions":[],"allowedTenants":[f.db.tenant_id()],
        "iss":"gamezone","aud":"gamezone","appId":"admin","orgIds":[f.db.tenant_id()]}),
    );
    let shifts = TenantShiftRepository::new(f.db.clone());
    let here = shifts
        .start_confirmed(
            staff,
            dto(json!({"openingBalance":100,"venueLocationId":venue})),
            staff,
        )
        .await
        .unwrap();
    let there = shifts
        .start_confirmed(
            outsider,
            dto(json!({"openingBalance":100,"venueLocationId":other})),
            outsider,
        )
        .await
        .unwrap();
    let shift_page = shifts
        .list_scoped(&dto(json!({"limit":1})), &[venue])
        .await
        .unwrap();
    assert_eq!(shift_page.total, 1);
    assert_eq!(shift_page.data[0].id, here.shift.id);
    assert_eq!(
        shifts
            .list_scoped(&dto(json!({"userId":outsider})), &[venue])
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        shifts
            .list_scoped(&dto(json!({})), &[])
            .await
            .unwrap()
            .total,
        0
    );
    let cash = TenantCashRegisterRepository::new(f.db.clone());
    let deposits = TenantCashDepositRepository::new(f.db.clone());
    let here_deposit = deposits.create(&dto(json!({"cashRegisterId":here.cash_register.id,"shiftId":here.shift.id,"amount":10,"denominations":{"10":1}})), staff).await.unwrap();
    let there_deposit = deposits.create(&dto(json!({"cashRegisterId":there.cash_register.id,"shiftId":there.shift.id,"amount":10,"denominations":{"10":1}})), outsider).await.unwrap();
    let scope = LocationScope::resolve_tenant(f.db.clone(), &claims, "cash-registers:read", None)
        .await
        .unwrap();
    let page = cash
        .list_scoped(&dto(json!({"limit":1})), &scope.locations)
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.data[0].id, here.cash_register.id);
    assert_eq!(
        cash.list_scoped(&dto(json!({"shiftId":there.shift.id})), &scope.locations)
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        cash.list_scoped(&dto(json!({})), &[]).await.unwrap().total,
        0
    );
    assert_eq!(
        cash.location_id(there.cash_register.id).await.unwrap(),
        other
    );
    assert!(LocationScope::resolve_tenant(
        f.db.clone(),
        &claims,
        "cash-registers:write",
        Some(cash.location_id(there.cash_register.id).await.unwrap())
    )
    .await
    .is_err());
    let scope = LocationScope::resolve_tenant(f.db.clone(), &claims, "cash-deposits:read", None)
        .await
        .unwrap();
    let page = deposits
        .list_scoped(&dto(json!({"limit":1})), &scope.locations)
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.data[0].id, here_deposit.id);
    assert_eq!(
        deposits
            .list_scoped(
                &dto(json!({"cashRegisterId":there.cash_register.id})),
                &scope.locations
            )
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        deposits
            .list_scoped(&dto(json!({})), &[])
            .await
            .unwrap()
            .total,
        0
    );
    assert!(LocationScope::resolve_tenant(
        f.db.clone(),
        &claims,
        "cash-deposits:approve",
        Some(deposits.location_id(there_deposit.id).await.unwrap())
    )
    .await
    .is_err());

    let category = TenantExpenseCategoryRepository::new(f.db.clone())
        .create(&dto(json!({"name":"Venue repairs"})), Some(staff))
        .await
        .unwrap();
    let expenses = TenantExpenseRepository::new(f.db.clone());
    let scoped = expenses
        .create_at(
            &dto(json!({"categoryId":category.id,"amount":3,"paymentMethod":"cash"})),
            Some(staff),
            Some(other),
        )
        .await
        .unwrap();
    let card = expenses
        .create_at(
            &dto(json!({"categoryId":category.id,"amount":4,"paymentMethod":"card"})),
            Some(staff),
            Some(venue),
        )
        .await
        .unwrap();
    let unscoped = expenses
        .create(
            &dto(json!({"categoryId":category.id,"amount":5,"paymentMethod":"card"})),
            Some(staff),
        )
        .await
        .unwrap();
    let before = f.outbox_count().await;
    let entries = f.scalar("SELECT COUNT(*) FROM cash_register_entries").await;
    assert!(expenses.approve(scoped.id, staff).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM cash_register_entries").await,
        entries
    );
    assert_eq!(
        expenses
            .find_by_id(scoped.id)
            .await
            .unwrap()
            .unwrap()
            .approval_status,
        "pending"
    );
    let page = expenses
        .list_scoped(&dto(json!({"limit":1})), &[venue], false)
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.data[0].id, card.id);
    assert_eq!(
        expenses
            .list_scoped(&dto(json!({})), &[venue], true)
            .await
            .unwrap()
            .total,
        2
    );
    assert_eq!(
        expenses
            .list_scoped(&dto(json!({})), &[], false)
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        expenses
            .list_scoped(&dto(json!({})), &[], true)
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(expenses.location_id(unscoped.id).await.unwrap(), None);
    assert!(expenses.create_at(&dto(json!({"categoryId":category.id,"amount":1,"paymentMethod":"card","shiftId":here.shift.id})), Some(staff), Some(other)).await.is_err());
    expenses
        .update(
            card.id,
            &dto(json!({"description":"New description"})),
            Some(staff),
        )
        .await
        .unwrap();
    assert_eq!(expenses.location_id(card.id).await.unwrap(), Some(venue));
    expenses
        .update(
            card.id,
            &dto(json!({"shiftId":there.shift.id})),
            Some(staff),
        )
        .await
        .unwrap();
    assert_eq!(expenses.location_id(card.id).await.unwrap(), Some(other));
    assert!(expenses
        .find_by_id_if_location(card.id, Some(venue))
        .await
        .unwrap()
        .is_none());
    assert!(expenses
        .find_by_id_if_location(card.id, Some(other))
        .await
        .unwrap()
        .is_some());
    let before_stale_action = f.outbox_count().await;
    assert!(expenses
        .approve_if_location(card.id, staff, Some(venue))
        .await
        .is_err());
    assert!(expenses
        .reject_if_location(card.id, "Stale", staff, Some(venue))
        .await
        .is_err());
    assert!(expenses
        .update_if_location(
            card.id,
            &dto(json!({"amount":99})),
            Some(staff),
            Some(venue)
        )
        .await
        .is_err());
    assert!(expenses
        .soft_delete_if_location(card.id, Some(venue))
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before_stale_action);
    assert_eq!(
        expenses.find_by_id(card.id).await.unwrap().unwrap().amount,
        4.0
    );

    expenses.approve(scoped.id, outsider).await.unwrap();
    assert_eq!(expenses.location_id(scoped.id).await.unwrap(), Some(other));
    expenses.soft_delete(scoped.id).await.unwrap();
    let last: String = sqlx::query_scalar("SELECT location_id FROM outbox_events WHERE aggregate_id=? AND event_type='expense.deleted'")
        .bind(scoped.id.to_string()).fetch_one(&f.db.read_pool().unwrap()).await.unwrap();
    assert_eq!(last, other.to_string());
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("DELETE FROM location_role_assignments WHERE user_id=?")
                .bind(staff.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(LocationScope::resolve_tenant(
        f.db.clone(),
        &claims,
        "cash-registers:read",
        Some(venue)
    )
    .await
    .is_err());
    f.close().await;
}

#[tokio::test]
async fn financial_activity_failure_rolls_back_the_whole_shift() {
    let f = Fixture::new().await;
    let venue = f.location("counter").await;
    let staff = f.staff().await;
    let shifts = TenantShiftRepository::new(f.db.clone());
    f.db.with_immediate_writer(move |c| Box::pin(async move {
        sqlx::query("CREATE TRIGGER reject_register_activity BEFORE INSERT ON activity_log WHEN NEW.kind='cash_register_opened' BEGIN SELECT RAISE(ABORT,'activity unavailable'); END").execute(c).await?;
        Ok(())
    })).await.unwrap();
    let before = f.outbox_count().await;
    let start: StartShiftDto = dto(json!({"openingBalance":100,"venueLocationId":venue}));
    assert!(shifts
        .start_confirmed(staff, start.clone(), staff)
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM shifts").await, 0);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM cash_registers").await, 0);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM activity_log").await, 0);
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("DROP TRIGGER reject_register_activity")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let first = shifts
        .start_confirmed(staff, start.clone(), staff)
        .await
        .unwrap();
    let retry = shifts.start_confirmed(staff, start, staff).await.unwrap();
    assert_eq!(first.shift.id, retry.shift.id);
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM activity_log WHERE kind='shift_clock_in'")
            .await,
        1
    );
    assert_eq!(
        f.scalar("SELECT COUNT(*) FROM activity_log WHERE kind='cash_register_opened'")
            .await,
        1
    );
    f.close().await;
}
