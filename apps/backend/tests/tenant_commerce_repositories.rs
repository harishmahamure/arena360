use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::Utc;
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::models::{
    CreateKioskOrderDto, CreateKioskOrderLineItemDto, CreateLineItemDto, CreateTransactionDto,
    CreditAccountFilterDto, SettleCreditDto, SettleItemDto, UpdateTransactionDto,
};
use gaming_cafe_api::repositories::{
    TenantCreditRepository, TenantKioskOrderRepository, TenantTransactionRepository,
};
use gaming_cafe_api::tenancy::{
    tenant_path, TenantDb, TenantDbConfig, TenantDbManager, TenantLease,
};
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
async fn checkout_credit_plan_and_fencing_are_atomic() {
    let fixture = Fixture::new().await;
    let transactions = TenantTransactionRepository::new(fixture.db.clone());

    let sale = transactions
        .create(
            fixture.sale("cash", "completed", 2, None),
            Some(fixture.staff),
        )
        .await
        .unwrap();
    assert_eq!(sale.amount, 25.0);
    assert_eq!(sale.paid_amount, 25.0);
    let stock: i32 = sqlx::query_scalar(
        "SELECT quantity_pieces FROM location_stock
         WHERE inventory_location_id=? AND product_id=?",
    )
    .bind(fixture.store.to_string())
    .bind(fixture.product.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    assert_eq!(stock, 3);
    let lines = transactions.list_line_items(sale.id).await.unwrap();
    assert_eq!(lines[0].product_name, "Cola");
    assert_eq!(lines[0].product_sku.as_deref(), Some("COLA"));

    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    let outbox_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    assert!(transactions
        .create(fixture.sale("cash", "completed", 4, None), None)
        .await
        .is_err());
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(before, after, "stock shortfall must roll back the sale");
    let outbox_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(
        outbox_before, outbox_after,
        "failed sale must not leave an outbox event"
    );

    let credit = transactions
        .create(fixture.sale("credit", "credit", 1, None), None)
        .await
        .unwrap();
    let credit_repo = TenantCreditRepository::new(fixture.db.clone());
    assert_eq!(
        credit_repo
            .summary(fixture.player)
            .await
            .unwrap()
            .outstanding,
        12.5
    );
    credit_repo
        .settle(
            SettleCreditDto {
                player_id: fixture.player,
                items: vec![SettleItemDto {
                    transaction_id: credit.id,
                    amount: 5.0,
                }],
                payment_method: "cash".into(),
                cash_amount: Some(5.0),
                online_amount: None,
                notes: None,
                online_payment_ref_last4: None,
            },
            fixture.shift,
            fixture.staff,
        )
        .await
        .unwrap();
    assert_eq!(
        credit_repo
            .summary(fixture.player)
            .await
            .unwrap()
            .outstanding,
        7.5
    );

    let pending = transactions
        .create(fixture.plan_purchase("pending"), None)
        .await
        .unwrap();
    assert_eq!(fixture.balance_count().await, 1);
    transactions
        .update(
            pending.id,
            &UpdateTransactionDto {
                payment_status: Some("completed".into()),
                notes: None,
            },
            None,
        )
        .await
        .unwrap();
    transactions
        .update(
            pending.id,
            &UpdateTransactionDto {
                payment_status: Some("completed".into()),
                notes: Some("retry".into()),
            },
            None,
        )
        .await
        .unwrap();
    let grants: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM player_plan_ledger WHERE transaction_id=?")
            .bind(pending.id.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(grants, 1);

    fixture
        .lease
        .generations
        .write()
        .unwrap()
        .insert(fixture.tenant, 2);
    assert!(transactions
        .create(fixture.sale("cash", "completed", 1, None), None)
        .await
        .is_err());
    fixture.close().await;
}

#[tokio::test]
async fn kiosk_placement_does_not_deduct_and_conversion_is_atomic() {
    let fixture = Fixture::new().await;
    let kiosk = TenantKioskOrderRepository::new(fixture.db.clone());
    let transactions = TenantTransactionRepository::new(fixture.db.clone());
    let order = kiosk
        .place(
            fixture.player,
            fixture.device,
            CreateKioskOrderDto {
                line_items: vec![CreateKioskOrderLineItemDto {
                    product_id: fixture.product,
                    quantity: 2,
                }],
                note: Some("cold".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(fixture.stock().await, 5);
    let placed_payload: String = sqlx::query_scalar(
        "SELECT payload FROM outbox_events
         WHERE aggregate_id=? AND event_type='kiosk_order.placed'
         ORDER BY sequence DESC LIMIT 1",
    )
    .bind(order.id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    let placed_payload: serde_json::Value = serde_json::from_str(&placed_payload).unwrap();
    assert_eq!(placed_payload["deviceName"], "PC-1");
    assert_eq!(placed_payload["playerUsername"], "player");
    assert_eq!(placed_payload["items"][0]["productName"], "Cola");
    assert_eq!(placed_payload["items"][0]["quantity"], 2);
    assert_eq!(placed_payload["items"][0]["unitPrice"], 12.5);
    assert!(kiosk
        .place(
            fixture.player,
            fixture.device,
            CreateKioskOrderDto {
                line_items: vec![CreateKioskOrderLineItemDto {
                    product_id: fixture.product,
                    quantity: 1,
                }],
                note: None,
            },
        )
        .await
        .is_err());

    let pending_before = fixture.atomic_counts().await;
    let error = transactions
        .create(
            fixture.sale("cash", "pending", 2, Some(order.id)),
            Some(fixture.staff),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::BadRequest(_)));
    assert_eq!(fixture.atomic_counts().await, pending_before);
    assert_eq!(fixture.stock().await, 5);
    assert_eq!(kiosk.get(order.id).await.unwrap().status, "pending");

    let outbox_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    assert!(transactions
        .create(
            fixture.sale("cash", "completed", 6, Some(order.id)),
            Some(fixture.staff),
        )
        .await
        .is_err());
    assert_eq!(kiosk.get(order.id).await.unwrap().status, "pending");
    let outbox_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&fixture.db.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(outbox_before, outbox_after);

    sqlx::query(
        "UPDATE products SET name='Current Cola',sku='CURRENT',day_price=990000,
         night_price=990000,updated_at=? WHERE id=?",
    )
    .bind(Fixture::now())
    .bind(fixture.product.to_string())
    .execute(&fixture.admin)
    .await
    .unwrap();
    let transaction = transactions
        .create(
            fixture.sale_with_tender(
                "cash",
                "completed",
                2,
                Some(order.id),
                Some(25.0),
                None,
                Some(999.0),
                Vec::new(),
            ),
            Some(fixture.staff),
        )
        .await
        .unwrap();
    assert_eq!(transaction.amount, 25.0);
    let sale_payload: String = sqlx::query_scalar(
        "SELECT payload FROM outbox_events
         WHERE aggregate_id=? AND event_type='transaction.created'
         ORDER BY sequence DESC LIMIT 1",
    )
    .bind(transaction.id.to_string())
    .fetch_one(&fixture.db.read_pool().unwrap())
    .await
    .unwrap();
    let sale_payload: serde_json::Value = serde_json::from_str(&sale_payload).unwrap();
    assert_eq!(sale_payload["paymentMethod"], "cash");
    assert_eq!(sale_payload["actorId"], fixture.staff.to_string());
    assert_eq!(sale_payload["actorRole"], "staff");
    assert_eq!(sale_payload["transactionType"], "product_purchase");
    let lines = transactions.list_line_items(transaction.id).await.unwrap();
    assert_eq!(lines[0].unit_price, 12.5);
    assert_eq!(lines[0].product_name, "Cola");
    assert_eq!(lines[0].product_sku.as_deref(), Some("COLA"));
    let fulfilled = kiosk.get(order.id).await.unwrap();
    assert_eq!(fulfilled.status, "fulfilled");
    assert_eq!(fulfilled.transaction_id, Some(transaction.id));
    assert_eq!(fixture.stock().await, 3);
    assert!(transactions
        .create(
            fixture.sale("cash", "completed", 2, Some(order.id)),
            Some(fixture.staff),
        )
        .await
        .is_err());
    fixture.close().await;
}

#[tokio::test]
async fn payment_status_matrix_and_credit_plan_failures_roll_back() {
    let fixture = Fixture::new().await;
    let transactions = TenantTransactionRepository::new(fixture.db.clone());

    for dto in [
        fixture.sale("cash", "credit", 1, None),
        fixture.sale("credit", "completed", 1, None),
        fixture.sale("cash", "failed", 1, None),
        fixture.sale("cash", "refunded", 1, None),
    ] {
        assert!(transactions.create(dto, None).await.is_err());
    }
    assert_eq!(fixture.stock().await, 5);
    assert_eq!(fixture.transaction_count().await, 0);

    let split = transactions
        .create(
            fixture.sale_with_tender(
                "split_payment",
                "completed",
                1,
                None,
                Some(5.0),
                Some(7.5),
                Some(0.0001),
                Vec::new(),
            ),
            Some(fixture.staff),
        )
        .await
        .unwrap();
    assert_eq!(split.amount, 12.5);
    assert_eq!(split.cash_amount, Some(5.0));
    assert_eq!(split.online_amount, Some(7.5));

    let mut online = fixture.sale("online", "completed", 1, None);
    online.online_payment_ref_last4 = Some(" 9876 ".into());
    let online = transactions.create(online, None).await.unwrap();
    assert_eq!(online.online_payment_ref_last4.as_deref(), Some("9876"));
    let stored_online_ref: Option<String> =
        sqlx::query_scalar("SELECT online_payment_ref_last4 FROM transactions WHERE id=?")
            .bind(online.id.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(stored_online_ref.as_deref(), Some("9876"));

    let mut cash = fixture.sale("cash", "completed", 1, None);
    cash.online_payment_ref_last4 = Some(" 1234 ".into());
    let cash = transactions.create(cash, None).await.unwrap();
    assert_eq!(cash.online_payment_ref_last4, None);
    let stored_cash_ref: Option<String> =
        sqlx::query_scalar("SELECT online_payment_ref_last4 FROM transactions WHERE id=?")
            .bind(cash.id.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(stored_cash_ref, None);

    assert!(transactions
        .create(
            fixture.sale_with_tender(
                "split_payment",
                "completed",
                1,
                None,
                Some(5.0),
                Some(7.4999),
                None,
                Vec::new(),
            ),
            None,
        )
        .await
        .is_err());
    assert!(transactions
        .create(
            fixture.sale_with_tender(
                "cash",
                "completed",
                1,
                None,
                Some(12.50001),
                None,
                None,
                Vec::new(),
            ),
            None,
        )
        .await
        .is_err());

    assert!(transactions
        .update(
            split.id,
            &UpdateTransactionDto {
                payment_status: Some("refunded".into()),
                notes: None,
            },
            None,
        )
        .await
        .is_err());
    transactions
        .update(
            split.id,
            &UpdateTransactionDto {
                payment_status: None,
                notes: Some("terminal note".into()),
            },
            None,
        )
        .await
        .unwrap();

    let pending_before = fixture.atomic_counts().await;
    let pending_stock = fixture.stock().await;
    let error = transactions
        .create(fixture.sale("cash", "pending", 1, None), None)
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::BadRequest(_)));
    let mut omitted = fixture.sale("cash", "completed", 1, None);
    omitted.payment_status = None;
    let error = transactions.create(omitted, None).await.unwrap_err();
    assert!(matches!(error, AppError::BadRequest(_)));
    assert_eq!(fixture.atomic_counts().await, pending_before);
    assert_eq!(fixture.stock().await, pending_stock);

    sqlx::query("UPDATE users SET credit_limit=50000 WHERE id=?")
        .bind(fixture.player.to_string())
        .execute(&fixture.admin)
        .await
        .unwrap();
    let before = fixture.atomic_counts().await;
    let minutes_before: i32 =
        sqlx::query_scalar("SELECT remaining_minutes FROM player_plan_balances WHERE player_id=?")
            .bind(fixture.player.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    let mut credit_plan = fixture.plan_purchase("credit");
    credit_plan.payment_method = "credit".into();
    assert!(transactions.create(credit_plan, None).await.is_err());
    assert_eq!(fixture.atomic_counts().await, before);
    let minutes_after: i32 =
        sqlx::query_scalar("SELECT remaining_minutes FROM player_plan_balances WHERE player_id=?")
            .bind(fixture.player.to_string())
            .fetch_one(&fixture.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(minutes_before, minutes_after);
    fixture.close().await;
}

#[tokio::test]
async fn recipes_options_snapshots_and_location_guards_are_enforced() {
    let fixture = Fixture::new().await;
    let transactions = TenantTransactionRepository::new(fixture.db.clone());
    let recipe = fixture.seed_recipe_product().await;
    let sale = transactions
        .create(
            fixture.product_sale(
                recipe.product,
                2,
                fixture.store,
                fixture.venue,
                vec![recipe.option],
            ),
            None,
        )
        .await
        .unwrap();
    assert_eq!(sale.amount, 42.0);
    assert_eq!(fixture.stock_for(recipe.base_ingredient).await, 18);
    assert_eq!(fixture.stock_for(recipe.extra_ingredient).await, 18);

    sqlx::query("UPDATE products SET name='Renamed Meal',sku='NEW',updated_at=? WHERE id=?")
        .bind(Fixture::now())
        .bind(recipe.product.to_string())
        .execute(&fixture.admin)
        .await
        .unwrap();
    sqlx::query("UPDATE product_option_groups SET name='Renamed Group' WHERE id=?")
        .bind(recipe.group.to_string())
        .execute(&fixture.admin)
        .await
        .unwrap();
    sqlx::query("UPDATE product_options SET name='Renamed Option',price_delta=90000 WHERE id=?")
        .bind(recipe.option.to_string())
        .execute(&fixture.admin)
        .await
        .unwrap();
    let lines = transactions.list_line_items(sale.id).await.unwrap();
    assert_eq!(lines[0].product_name, "Meal");
    assert_eq!(lines[0].product_sku.as_deref(), Some("MEAL"));
    assert_eq!(lines[0].option_names, vec!["Less base + extra"]);

    assert!(transactions
        .create(
            fixture.product_sale(
                recipe.base_ingredient,
                1,
                fixture.store,
                fixture.venue,
                Vec::new(),
            ),
            None,
        )
        .await
        .is_err());
    assert!(transactions
        .create(
            fixture.product_sale(
                fixture.product,
                1,
                fixture.warehouse,
                fixture.venue,
                Vec::new(),
            ),
            None,
        )
        .await
        .is_err());
    assert!(transactions
        .create(
            fixture.product_sale(
                fixture.product,
                1,
                fixture.store,
                fixture.other_venue,
                Vec::new(),
            ),
            None,
        )
        .await
        .is_err());
    fixture.close().await;
}

#[tokio::test]
async fn concurrent_sales_cannot_oversell() {
    let fixture = Fixture::new().await;
    let first = TenantTransactionRepository::new(fixture.db.clone());
    let second = first.clone();
    let first_dto = fixture.sale("cash", "completed", 3, None);
    let second_dto = fixture.sale("cash", "completed", 3, None);
    let (first_result, second_result) = tokio::join!(
        first.create(first_dto, None),
        second.create(second_dto, None)
    );
    assert_eq!(
        usize::from(first_result.is_ok()) + usize::from(second_result.is_ok()),
        1
    );
    assert_eq!(fixture.stock().await, 2);
    assert_eq!(fixture.transaction_count().await, 1);
    fixture.close().await;
}

#[tokio::test]
async fn settlement_history_is_stable_and_overapplication_rolls_back() {
    let fixture = Fixture::new().await;
    let transactions = TenantTransactionRepository::new(fixture.db.clone());
    let credit = transactions
        .create(fixture.sale("credit", "credit", 2, None), None)
        .await
        .unwrap();
    let credits = TenantCreditRepository::new(fixture.db.clone());
    let first = credits
        .settle(
            fixture.settlement(credit.id, 10.0),
            fixture.shift,
            fixture.staff,
        )
        .await
        .unwrap();
    assert_eq!(
        credits.settlement_detail(first.id).await.unwrap().items[0].remaining_after,
        15.0
    );

    let before = fixture.atomic_counts().await;
    assert!(credits
        .settle(
            fixture.settlement(credit.id, 15.0001),
            fixture.shift,
            fixture.staff,
        )
        .await
        .is_err());
    assert_eq!(fixture.atomic_counts().await, before);

    let second = credits
        .settle(
            fixture.settlement(credit.id, 15.0),
            fixture.shift,
            fixture.staff,
        )
        .await
        .unwrap();
    assert_eq!(
        credits.settlement_detail(second.id).await.unwrap().items[0].remaining_after,
        0.0
    );
    assert_eq!(
        credits.settlement_detail(first.id).await.unwrap().items[0].remaining_after,
        15.0
    );
    assert_eq!(
        credits.summary(fixture.player).await.unwrap().outstanding,
        0.0
    );
    assert!(credits
        .settle(
            fixture.settlement(credit.id, 1.0),
            fixture.shift,
            fixture.staff,
        )
        .await
        .is_err());

    let matching = credits
        .list_players(&CreditAccountFilterDto {
            search: Some("player".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        matching.total, 0,
        "fully settled players are not outstanding"
    );
    fixture.close().await;
}

#[tokio::test]
async fn deleted_kiosk_product_fails_without_fulfillment_or_stock_change() {
    let fixture = Fixture::new().await;
    let kiosk = TenantKioskOrderRepository::new(fixture.db.clone());
    let transactions = TenantTransactionRepository::new(fixture.db.clone());
    let order = kiosk
        .place(
            fixture.player,
            fixture.device,
            CreateKioskOrderDto {
                line_items: vec![CreateKioskOrderLineItemDto {
                    product_id: fixture.product,
                    quantity: 1,
                }],
                note: None,
            },
        )
        .await
        .unwrap();
    sqlx::query("UPDATE products SET deleted_at=?,updated_at=? WHERE id=?")
        .bind(Fixture::now())
        .bind(Fixture::now())
        .bind(fixture.product.to_string())
        .execute(&fixture.admin)
        .await
        .unwrap();
    let before = fixture.atomic_counts().await;
    assert!(transactions
        .create(fixture.sale("cash", "completed", 1, Some(order.id)), None,)
        .await
        .is_err());
    assert_eq!(fixture.atomic_counts().await, before);
    assert_eq!(fixture.stock().await, 5);
    assert_eq!(kiosk.get(order.id).await.unwrap().status, "pending");
    fixture.close().await;
}

struct RecipeFixture {
    product: Uuid,
    base_ingredient: Uuid,
    extra_ingredient: Uuid,
    group: Uuid,
    option: Uuid,
}

struct Fixture {
    root: PathBuf,
    tenant: Uuid,
    venue: Uuid,
    player: Uuid,
    staff: Uuid,
    shift: Uuid,
    store: Uuid,
    warehouse: Uuid,
    other_venue: Uuid,
    product: Uuid,
    plan: Uuid,
    device: Uuid,
    db: Arc<TenantDb>,
    admin: sqlx::SqlitePool,
    lease: Arc<Lease>,
}

impl Fixture {
    async fn new() -> Self {
        let root = std::env::temp_dir().join(format!("arena360-commerce-{}", Uuid::now_v7()));
        let tenant = Uuid::now_v7();
        let path = tenant_path(&root, tenant);
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
        let venue = Uuid::now_v7();
        let player = Uuid::now_v7();
        let staff = Uuid::now_v7();
        let shift = Uuid::now_v7();
        let store = Uuid::now_v7();
        let warehouse = Uuid::now_v7();
        let other_venue = Uuid::now_v7();
        let product = Uuid::now_v7();
        let plan = Uuid::now_v7();
        let device = Uuid::now_v7();
        let balance = Uuid::now_v7();
        let session = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,?,?,?,?)",
        )
        .bind(venue.to_string())
        .bind("main")
        .bind("Main")
        .bind(&at)
        .bind(&at)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,?,?,?,?)",
        )
        .bind(other_venue.to_string())
        .bind("other")
        .bind("Other")
        .bind(&at)
        .bind(&at)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO users(id,username,password_hash,role,credit_limit,created_at,updated_at) VALUES(?,?,'hash','player',500000,?,?)")
            .bind(player.to_string()).bind("player").bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO users(id,username,role,created_at,updated_at) VALUES(?,?,'staff',?,?)",
        )
        .bind(staff.to_string())
        .bind("staff")
        .bind(&at)
        .bind(&at)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO shifts(id,user_id,location_id,clock_in,status,created_at,updated_at) VALUES(?,?,?,?,'active',?,?)")
            .bind(shift.to_string()).bind(staff.to_string()).bind(venue.to_string()).bind(&at).bind(&at).bind(&at)
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO cash_registers(id,shift_id,opened_by,opening_balance,created_at,updated_at) VALUES(?,?,?,0,?,?)")
            .bind(Uuid::now_v7().to_string()).bind(shift.to_string()).bind(staff.to_string()).bind(&at).bind(&at)
            .execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO inventory_locations(id,venue_location_id,name,kind,created_at,updated_at) VALUES(?,?,'Counter','store',?,?)")
            .bind(store.to_string()).bind(venue.to_string()).bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO inventory_locations(id,venue_location_id,name,kind,created_at,updated_at) VALUES(?,?,'Warehouse','warehouse',?,?)")
            .bind(warehouse.to_string()).bind(venue.to_string()).bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO products(id,name,day_price,night_price,category,sku,created_at,updated_at) VALUES(?,'Cola',125000,125000,'beverage','COLA',?,?)")
            .bind(product.to_string()).bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO location_stock(inventory_location_id,product_id,quantity_pieces,created_at,updated_at) VALUES(?,?,5,?,?)")
            .bind(store.to_string()).bind(product.to_string()).bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO plans(id,name,price,plan_type,validity_days,time_credits,created_at,updated_at) VALUES(?,'Hour',100000,'time_based',30,60,?,?)")
            .bind(plan.to_string()).bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO devices(id,location_id,name,status,created_at,updated_at) VALUES(?,?,'PC-1','in_use',?,?)")
            .bind(device.to_string()).bind(venue.to_string()).bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO player_plan_balances(id,player_id,kind,remaining_minutes,expiry_date,status,source_plan_id,created_at,updated_at) VALUES(?,?,'time',60,'2099-01-01T00:00:00.000000Z','active',?,?,?)")
            .bind(balance.to_string()).bind(player.to_string()).bind(plan.to_string()).bind(&at).bind(&at).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO usage_sessions(id,player_id,balance_id,device_id,location_id,start_time,wallet_minutes_at_start,created_at,updated_at) VALUES(?,?,?,?,?,?,60,?,?)")
            .bind(session.to_string()).bind(player.to_string()).bind(balance.to_string()).bind(device.to_string()).bind(venue.to_string()).bind(&at).bind(&at).bind(&at)
            .execute(&pool).await.unwrap();
        pool.close().await;
        let lease = Arc::new(Lease::default());
        lease.generations.write().unwrap().insert(tenant, 1);
        let manager = TenantDbManager::new(
            TenantDbConfig {
                root: root.clone(),
                read_connections: 3,
                busy_timeout: Duration::from_millis(250),
                idle_timeout: Duration::from_secs(60),
                reaper_interval: Duration::from_secs(1),
            },
            lease.clone(),
        )
        .unwrap();
        let db = manager.open(tenant).await.unwrap();
        let admin = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
        Self {
            root,
            tenant,
            venue,
            player,
            staff,
            shift,
            store,
            warehouse,
            other_venue,
            product,
            plan,
            device,
            db,
            admin,
            lease,
        }
    }

    fn sale(
        &self,
        method: &str,
        status: &str,
        quantity: i32,
        kiosk: Option<Uuid>,
    ) -> CreateTransactionDto {
        self.sale_with_tender(
            method,
            status,
            quantity,
            kiosk,
            None,
            None,
            None,
            Vec::new(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn sale_with_tender(
        &self,
        method: &str,
        status: &str,
        quantity: i32,
        kiosk: Option<Uuid>,
        cash_amount: Option<f64>,
        online_amount: Option<f64>,
        quoted_unit_price: Option<f64>,
        option_ids: Vec<Uuid>,
    ) -> CreateTransactionDto {
        CreateTransactionDto {
            player_id: self.player,
            transaction_type: "product_purchase".into(),
            plan_id: None,
            shift_id: Some(self.shift),
            amount: None,
            payment_method: method.into(),
            payment_status: Some(status.into()),
            notes: None,
            online_payment_ref_last4: (method == "online"
                || (method == "split_payment" && online_amount.unwrap_or(0.0) > 0.0))
                .then(|| "1234".into()),
            transaction_date: None,
            cash_amount,
            online_amount,
            line_items: Some(vec![CreateLineItemDto {
                product_id: self.product,
                quantity,
                unit_price: quoted_unit_price,
                option_ids,
            }]),
            sale_location_id: Some(self.store),
            venue_location_id: Some(self.venue),
            kiosk_order_id: kiosk,
        }
    }

    fn product_sale(
        &self,
        product_id: Uuid,
        quantity: i32,
        store: Uuid,
        venue: Uuid,
        option_ids: Vec<Uuid>,
    ) -> CreateTransactionDto {
        CreateTransactionDto {
            player_id: self.player,
            transaction_type: "product_purchase".into(),
            plan_id: None,
            shift_id: Some(self.shift),
            amount: None,
            payment_method: "cash".into(),
            payment_status: Some("completed".into()),
            notes: None,
            online_payment_ref_last4: None,
            transaction_date: None,
            cash_amount: None,
            online_amount: None,
            line_items: Some(vec![CreateLineItemDto {
                product_id,
                quantity,
                unit_price: Some(999.0),
                option_ids,
            }]),
            sale_location_id: Some(store),
            venue_location_id: Some(venue),
            kiosk_order_id: None,
        }
    }

    fn plan_purchase(&self, status: &str) -> CreateTransactionDto {
        CreateTransactionDto {
            player_id: self.player,
            transaction_type: "plan_purchase".into(),
            plan_id: Some(self.plan),
            shift_id: Some(self.shift),
            amount: Some(10.0),
            payment_method: "cash".into(),
            payment_status: Some(status.into()),
            notes: None,
            online_payment_ref_last4: None,
            transaction_date: None,
            cash_amount: None,
            online_amount: None,
            line_items: None,
            sale_location_id: None,
            venue_location_id: Some(self.venue),
            kiosk_order_id: None,
        }
    }

    fn settlement(&self, transaction_id: Uuid, amount: f64) -> SettleCreditDto {
        SettleCreditDto {
            player_id: self.player,
            items: vec![SettleItemDto {
                transaction_id,
                amount,
            }],
            payment_method: "cash".into(),
            cash_amount: Some(amount),
            online_amount: None,
            notes: None,
            online_payment_ref_last4: None,
        }
    }

    async fn seed_recipe_product(&self) -> RecipeFixture {
        let product = Uuid::now_v7();
        let base_ingredient = Uuid::now_v7();
        let extra_ingredient = Uuid::now_v7();
        let group = Uuid::now_v7();
        let option = Uuid::now_v7();
        let at = Self::now();
        let pool = self.admin.clone();
        for (id, name, sku) in [
            (base_ingredient, "Base ingredient", "BASE"),
            (extra_ingredient, "Extra ingredient", "EXTRA"),
        ] {
            sqlx::query(
                "INSERT INTO products(id,name,day_price,night_price,category,sku,is_raw_material,
                 created_at,updated_at) VALUES(?,?,0,0,'other',?,1,?,?)",
            )
            .bind(id.to_string())
            .bind(name)
            .bind(sku)
            .bind(&at)
            .bind(&at)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO location_stock(inventory_location_id,product_id,quantity_pieces,
                 created_at,updated_at) VALUES(?,?,20,?,?)",
            )
            .bind(self.store.to_string())
            .bind(id.to_string())
            .bind(&at)
            .bind(&at)
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO products(id,name,day_price,night_price,category,sku,created_at,updated_at)
             VALUES(?,'Meal',200000,200000,'meal','MEAL',?,?)",
        )
        .bind(product.to_string())
        .bind(&at)
        .bind(&at)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO product_recipe_items(product_id,ingredient_id,quantity) VALUES(?,?,2)",
        )
        .bind(product.to_string())
        .bind(base_ingredient.to_string())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO product_option_groups(id,product_id,name,required,multiple,sort_order)
             VALUES(?,?,'Adjustment',0,0,0)",
        )
        .bind(group.to_string())
        .bind(product.to_string())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO product_options(id,group_id,name,price_delta,sort_order)
             VALUES(?,?,'Less base + extra',10000,0)",
        )
        .bind(option.to_string())
        .bind(group.to_string())
        .execute(&pool)
        .await
        .unwrap();
        for (ingredient, quantity) in [(base_ingredient, -1), (extra_ingredient, 1)] {
            sqlx::query(
                "INSERT INTO product_option_ingredients(option_id,ingredient_id,quantity)
                 VALUES(?,?,?)",
            )
            .bind(option.to_string())
            .bind(ingredient.to_string())
            .bind(quantity)
            .execute(&pool)
            .await
            .unwrap();
        }
        RecipeFixture {
            product,
            base_ingredient,
            extra_ingredient,
            group,
            option,
        }
    }

    fn now() -> String {
        gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap()
    }

    async fn stock(&self) -> i32 {
        sqlx::query_scalar("SELECT quantity_pieces FROM location_stock WHERE inventory_location_id=? AND product_id=?")
            .bind(self.store.to_string()).bind(self.product.to_string())
            .fetch_one(&self.db.read_pool().unwrap()).await.unwrap()
    }

    async fn stock_for(&self, product: Uuid) -> i32 {
        sqlx::query_scalar(
            "SELECT quantity_pieces FROM location_stock
             WHERE inventory_location_id=? AND product_id=?",
        )
        .bind(self.store.to_string())
        .bind(product.to_string())
        .fetch_one(&self.db.read_pool().unwrap())
        .await
        .unwrap()
    }

    async fn transaction_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
            .fetch_one(&self.db.read_pool().unwrap())
            .await
            .unwrap()
    }

    async fn atomic_counts(&self) -> (i64, i64, i64, i64, i64) {
        let pool = self.db.read_pool().unwrap();
        (
            sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
                .fetch_one(&pool)
                .await
                .unwrap(),
            sqlx::query_scalar("SELECT COUNT(*) FROM player_plan_balances")
                .fetch_one(&pool)
                .await
                .unwrap(),
            sqlx::query_scalar("SELECT COUNT(*) FROM player_plan_ledger")
                .fetch_one(&pool)
                .await
                .unwrap(),
            sqlx::query_scalar("SELECT COUNT(*) FROM credit_settlements")
                .fetch_one(&pool)
                .await
                .unwrap(),
            sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
                .fetch_one(&pool)
                .await
                .unwrap(),
        )
    }

    async fn balance_count(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM player_plan_balances")
            .fetch_one(&self.db.read_pool().unwrap())
            .await
            .unwrap()
    }

    async fn close(self) {
        self.db.close().await.unwrap();
        self.admin.close().await;
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}

#[tokio::test]
async fn sales_and_settlements_update_cash_atomically() {
    let f = Fixture::new().await;
    let transactions = TenantTransactionRepository::new(f.db.clone());
    let sale = transactions
        .create(f.sale("cash", "completed", 1, None), Some(f.staff))
        .await
        .unwrap();
    transactions
        .update(
            sale.id,
            &UpdateTransactionDto {
                payment_status: None,
                notes: Some("note only".into()),
            },
            Some(f.staff),
        )
        .await
        .unwrap();
    let entries:i64=sqlx::query_scalar("SELECT COUNT(*) FROM cash_register_entries WHERE reference_id=? AND reference_type='transaction'").bind(sale.id.to_string()).fetch_one(&f.db.read_pool().unwrap()).await.unwrap();
    assert_eq!(entries, 1);
    let credit = transactions
        .create(f.sale("credit", "credit", 1, None), Some(f.staff))
        .await
        .unwrap();
    let credits = TenantCreditRepository::new(f.db.clone());
    credits
        .settle(f.settlement(credit.id, 5.0), f.shift, f.staff)
        .await
        .unwrap();
    let cash: i64 = sqlx::query_scalar("SELECT SUM(amount) FROM cash_register_entries")
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(cash, 175000);
    sqlx::query("UPDATE cash_registers SET status='closed',closing_balance=175000,expected_closing=175000,variance=0 WHERE shift_id=?").bind(f.shift.to_string()).execute(&f.admin).await.unwrap();
    let before = f.atomic_counts().await;
    let stock = f.stock().await;
    assert!(transactions
        .create(f.sale("cash", "completed", 1, None), Some(f.staff))
        .await
        .is_err());
    assert_eq!(f.atomic_counts().await, before);
    assert_eq!(f.stock().await, stock);
    assert!(credits
        .settle(f.settlement(credit.id, 7.5), f.shift, f.staff)
        .await
        .is_err());
    assert_eq!(f.atomic_counts().await, before);
    assert_eq!(credits.summary(f.player).await.unwrap().outstanding, 7.5);
    f.close().await;
}

#[tokio::test]
async fn kitchen_queue_is_opt_in_atomic_and_uses_sale_snapshots() {
    let f = Fixture::new().await;
    let kitchen = gaming_cafe_api::services::TenantKitchenService::new(f.db.clone());
    let tx = TenantTransactionRepository::new(f.db.clone());
    tx.create(f.sale("cash", "completed", 1, None), None)
        .await
        .unwrap();
    assert_eq!(kitchen.list(false).await.unwrap(), serde_json::json!([]));
    let (a, b) = tokio::join!(
        kitchen.save_menu(f.product, true, "Counter", 5, 0, f.staff),
        kitchen.save_menu(f.product, true, "Kitchen", 15, 0, f.staff)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert_eq!(kitchen.menu().await.unwrap()[0]["enabled"], true);
    let sale = tx
        .create(f.sale("credit", "credit", 1, None), Some(f.staff))
        .await
        .unwrap();
    let tickets = kitchen.list(false).await.unwrap();
    assert_eq!(tickets.as_array().unwrap().len(), 1);
    assert_eq!(tickets[0]["transactionId"], sale.id.to_string());
    assert_eq!(tickets[0]["items"][0]["name"], "Cola");
    let id = Uuid::parse_str(tickets[0]["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE products SET name='Renamed' WHERE id=?")
        .bind(f.product.to_string())
        .execute(&f.admin)
        .await
        .unwrap();
    assert_eq!(
        kitchen.list(false).await.unwrap()[0]["items"][0]["name"],
        "Cola"
    );
    assert!(kitchen
        .advance(id, "served", 1, None, f.staff)
        .await
        .is_err());
    kitchen
        .advance(id, "preparing", 1, None, f.staff)
        .await
        .unwrap();
    assert!(kitchen
        .advance(id, "ready", 1, None, f.staff)
        .await
        .is_err());
    sqlx::query("UPDATE transactions SET payment_status='failed' WHERE id=?")
        .bind(sale.id.to_string())
        .execute(&f.admin)
        .await
        .unwrap();
    assert!(kitchen
        .advance(id, "ready", 2, None, f.staff)
        .await
        .is_err());
    assert!(kitchen
        .advance(id, "cancelled", 2, None, f.staff)
        .await
        .is_err());
    kitchen
        .advance(id, "cancelled", 2, Some("payment failed"), f.staff)
        .await
        .unwrap();
    assert!(kitchen
        .list(false)
        .await
        .unwrap()
        .as_array()
        .unwrap()
        .is_empty());
    let history = kitchen.list(true).await.unwrap();
    assert_eq!(history[0]["events"].as_array().unwrap().len(), 3);
    assert_eq!(history[0]["events"][1]["actor"], "staff");
    let count = f.atomic_counts().await;
    assert!(tx
        .create(f.sale("cash", "completed", 99, None), Some(f.staff))
        .await
        .is_err());
    assert_eq!(f.atomic_counts().await, count);
    assert_eq!(
        kitchen.list(true).await.unwrap().as_array().unwrap().len(),
        1
    );
    f.close().await;
}

#[tokio::test]
async fn transaction_read_scopes_limit_staff_venues_and_player_history() {
    use gaming_cafe_api::{
        access::scope::TransactionReadScope, dto::JwtUserClaims, models::TransactionFilterDto,
    };
    use serde_json::json;
    let f = Fixture::new().await;
    let remote = Uuid::now_v7();
    let other_player = Uuid::now_v7();
    let role = Uuid::now_v7();
    let user = f.staff;
    let venue = f.venue;
    let device = f.device;
    f.db.with_immediate_writer(move|c|Box::pin(async move {
        let at = gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
        sqlx::query("INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,'remote','Remote',?,?)")
            .bind(remote.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO users(id,username,role,password_hash,created_at,updated_at) VALUES(?,'other-player','player','test-hash',?,?)")
            .bind(other_player.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("UPDATE devices SET registration_status='registered' WHERE id=?").bind(device.to_string()).execute(&mut *c).await?;
        sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) VALUES(?,'Sales reader','[\"transactions:read\",\"transactions:write\",\"credit:read\",\"credit:write\"]',?,?)")
            .bind(role.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)")
            .bind(user.to_string()).bind(role.to_string()).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO location_role_assignments(user_id,location_id,role_id,created_at) VALUES(?,?,?,?)")
            .bind(user.to_string()).bind(venue.to_string()).bind(role.to_string()).bind(&at).execute(c).await?;
        Ok(())
    })).await.unwrap();
    let claims = |user, role: &str| {
        serde_json::from_value::<JwtUserClaims>(json!({"sub":user,"userId":user,"tenantId":f.tenant,
        "roles":[role],"permissions":[],"allowedTenants":[f.tenant],"iss":"gamezone","aud":"gamezone",
        "appId":"game-zone-kiosk","orgIds":[f.tenant],"deviceId":f.device,"locationId":f.venue,"exp":Utc::now().timestamp()+3600})).unwrap()
    };
    let repo = TenantTransactionRepository::new(f.db.clone());
    let remote_sale = repo
        .create(f.plan_purchase("pending"), Some(f.staff))
        .await
        .unwrap();
    let remote_id = remote_sale.id;
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE transactions SET location_id=? WHERE id=?")
                .bind(remote.to_string())
                .bind(remote_id.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let own_sale = repo
        .create(f.plan_purchase("pending"), Some(f.staff))
        .await
        .unwrap();
    let mut foreign = f.plan_purchase("pending");
    foreign.player_id = other_player;
    let foreign_sale = repo.create(foreign, Some(f.staff)).await.unwrap();
    let mut staff_claims = claims(f.staff, "staff");
    staff_claims.appId = "admin".into();
    staff_claims.deviceId = None;
    let staff = TransactionReadScope::resolve_tenant(f.db.clone(), &staff_claims, None)
        .await
        .unwrap();
    let page = repo
        .list_scoped(
            &serde_json::from_value(json!({"limit":1})).unwrap(),
            staff.locations.as_deref(),
        )
        .await
        .unwrap();
    assert_eq!(page.total, 2);
    assert_eq!(page.data.len(), 1);
    let (player, venue) = repo.access_context(remote_sale.id).await.unwrap();
    assert!(staff.ensure_resource(player, venue).is_err());
    assert!(
        TransactionReadScope::resolve_tenant(f.db.clone(), &staff_claims, Some(remote))
            .await
            .is_err()
    );
    assert_eq!(
        repo.list_scoped(&TransactionFilterDto::default(), Some(&[]))
            .await
            .unwrap()
            .total,
        0
    );
    let player_claims = claims(f.player, "player");
    let player_scope = TransactionReadScope::resolve_tenant(f.db.clone(), &player_claims, None)
        .await
        .unwrap();
    let mut filters = TransactionFilterDto::default();
    player_scope.apply_player_filter(&mut filters).unwrap();
    let page = repo
        .list_scoped(&filters, player_scope.locations.as_deref())
        .await
        .unwrap();
    assert_eq!(page.total, 2);
    assert!(page.data.iter().all(|row| row.player_id == f.player));
    assert!(player_scope.ensure_resource(f.player, remote).is_ok());
    let (player, venue) = repo.access_context(foreign_sale.id).await.unwrap();
    assert!(player_scope.ensure_resource(player, venue).is_err());
    filters.player_id = Some(other_player);
    assert!(player_scope.apply_player_filter(&mut filters).is_err());
    let (player, venue) = repo.access_context(own_sale.id).await.unwrap();
    assert!(player_scope.ensure_resource(player, venue).is_ok());
    assert!(
        TransactionReadScope::resolve_tenant(f.db.clone(), &claims(f.player, "device"), None)
            .await
            .is_err()
    );
    let id = f.player;
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET is_active=0 WHERE id=?")
                .bind(id.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(
        TransactionReadScope::resolve_tenant(f.db.clone(), &player_claims, None)
            .await
            .is_err()
    );
    f.close().await;
}

#[tokio::test]
async fn settlement_scope_uses_the_collecting_shift_venue_before_pagination() {
    let f = Fixture::new().await;
    let repo = TenantTransactionRepository::new(f.db.clone());
    let sale = repo
        .create(f.sale("credit", "credit", 2, None), None)
        .await
        .unwrap();
    let credits = TenantCreditRepository::new(f.db.clone());
    let first = credits
        .settle(f.settlement(sale.id, 5.0), f.shift, f.staff)
        .await
        .unwrap();
    let remote = Uuid::now_v7();
    let actor = Uuid::now_v7();
    let shift = Uuid::now_v7();
    f.db.with_immediate_writer(move |c| Box::pin(async move {
        let at = gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
        sqlx::query("INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,'remote','Remote',?,?)").bind(remote.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO users(id,username,role,created_at,updated_at) VALUES(?,'remote-staff','staff',?,?)").bind(actor.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO shifts(id,user_id,location_id,clock_in,status,created_at,updated_at) VALUES(?,?,?,?,'active',?,?)").bind(shift.to_string()).bind(actor.to_string()).bind(remote.to_string()).bind(&at).bind(&at).bind(&at).execute(c).await?;
        Ok(())
    })).await.unwrap();
    let mut tender = f.settlement(sale.id, 5.0);
    tender.payment_method = "online".into();
    tender.cash_amount = None;
    tender.online_amount = Some(5.0);
    tender.online_payment_ref_last4 = Some("1234".into());
    let second = credits.settle(tender, shift, actor).await.unwrap();
    let filter = serde_json::from_value(serde_json::json!({"limit":1})).unwrap();
    let local = credits
        .list_settlements_scoped(&filter, Some(&[f.venue]))
        .await
        .unwrap();
    assert_eq!(local.total, 1);
    assert_eq!(local.data[0].id, first.id);
    let remote_page = credits
        .list_settlements_scoped(&filter, Some(&[remote]))
        .await
        .unwrap();
    assert_eq!(remote_page.total, 1);
    assert_eq!(remote_page.data[0].id, second.id);
    assert_eq!(
        credits.settlement_location_id(second.id).await.unwrap(),
        remote
    );
    assert_eq!(
        credits
            .list_settlements_scoped(&filter, Some(&[]))
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(credits.list_settlements(&filter).await.unwrap().total, 2);
    f.close().await;
}
