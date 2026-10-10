use chrono::Utc;
use gaming_cafe_api::{
    error::AppError,
    models::*,
    repositories::{TenantInventoryRepository, TenantVendorRepository},
    services::TenantProcurementService,
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
    async fn product(&self, name: &str) -> Uuid {
        let id = Uuid::now_v7();
        let name = name.to_string();
        self.db.with_immediate_writer(move|c|Box::pin(async move{
   let ts=gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
   sqlx::query("INSERT INTO products(id,name,day_price,night_price,units_per_purchase_unit,created_at,updated_at) VALUES(?,?,10000,10000,12,?,?)").bind(id.to_string()).bind(name).bind(&ts).bind(&ts).execute(c).await?;Ok(())
  })).await.unwrap();
        id
    }
    async fn staff(&self) -> Uuid {
        let id = Uuid::now_v7();
        self.db.with_immediate_writer(move|c|Box::pin(async move{
   let ts=gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
   sqlx::query("INSERT INTO users(id,username,role,created_at,updated_at) VALUES(?,?,'staff',?,?)").bind(id.to_string()).bind(id.to_string()).bind(&ts).bind(&ts).execute(c).await?;Ok(())
  })).await.unwrap();
        id
    }
    async fn inventory(&self, venue: Uuid, kind: &str) -> InventoryLocation {
        TenantInventoryRepository::new(self.db.clone())
            .create_location(
                &serde_json::from_value::<CreateInventoryLocationDto>(
                    json!({"name":kind,"kind":kind,"venueLocationId":venue}),
                )
                .unwrap(),
                None,
            )
            .await
            .unwrap()
    }
    async fn supplier(&self) -> Vendor {
        TenantVendorRepository::new(self.db.clone())
            .create(
                &serde_json::from_value::<CreateVendorDto>(json!({"name":"Supplier"})).unwrap(),
                None,
            )
            .await
            .unwrap()
    }
    async fn scalar(&self, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .fetch_one(&self.db.read_pool().unwrap())
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn inventory_receipt_transfer_waste_and_adjustment_are_atomic() {
    let f = Fixture::new().await;
    let other = Fixture::new().await;
    let venue = f.location("main").await;
    let warehouse = f.inventory(venue, "warehouse").await;
    let store = f.inventory(venue, "store").await;
    let product = f.product("Tea").await;
    let actor = f.staff().await;
    let repo = TenantInventoryRepository::new(f.db.clone());
    let receipt:CreateStockReceiptDto=serde_json::from_value(json!({"locationId":warehouse.id,"exceptionalReason":"Opening inventory","lines":[{"productId":product,"boxQuantity":2}]})).unwrap();
    let (r, lines) = repo.create_receipt(&receipt, Some(actor)).await.unwrap();
    assert_eq!(lines[0].pieces_added, 24);
    assert_eq!(repo.receipt_lines(r.id).await.unwrap()[0].pieces_added, 24);
    assert_eq!(
        repo.list_receipts(&Default::default()).await.unwrap().total,
        1
    );
    assert!(TenantInventoryRepository::new(other.db.clone())
        .find_location_by_id(store.id)
        .await
        .unwrap()
        .is_none());
    let (t, _) = repo
        .create_transfer_request(warehouse.id, store.id, &[(product, 20)], Some(actor))
        .await
        .unwrap();
    repo.approve_transfer(t.id, actor).await.unwrap();
    let (a, b) = tokio::join!(
        repo.fulfill_transfer(t.id, actor),
        repo.fulfill_transfer(t.id, actor)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(
        repo.stock_quantity_at(warehouse.id, product).await.unwrap(),
        4
    );
    assert_eq!(repo.stock_quantity_at(store.id, product).await.unwrap(), 20);
    let before = f.outbox_count().await;
    let bad:CreateStockReceiptDto=serde_json::from_value(json!({"locationId":warehouse.id,"exceptionalReason":"Bad product receipt","lines":[{"productId":product,"boxQuantity":1},{"productId":Uuid::now_v7(),"boxQuantity":1}]})).unwrap();
    assert!(repo.create_receipt(&bad, None).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(
        repo.stock_quantity_at(warehouse.id, product).await.unwrap(),
        4
    );
    let waste:CreateStockWasteEventDto=serde_json::from_value(json!({"locationId":store.id,"lines":[{"productId":product,"quantityPieces":4,"reasonCode":"Damaged"},{"productId":product,"quantityPieces":18,"reasonCode":"expired"}]})).unwrap();
    let (w, _) = repo.create_waste_event(&waste, Some(actor)).await.unwrap();
    let before = f.outbox_count().await;
    assert!(repo.approve_waste(w.id, actor).await.is_err());
    assert_eq!(repo.stock_quantity_at(store.id, product).await.unwrap(), 20);
    assert_eq!(
        repo.find_waste_by_id(w.id).await.unwrap().unwrap().status,
        "pending"
    );
    assert_eq!(f.outbox_count().await, before);
    let adjustment:CreateStockAdjustmentDto=serde_json::from_value(json!({"locationId":store.id,"notes":"Physical count","lines":[{"productId":product,"countedPieces":7}]})).unwrap();
    let (adj, lines) = repo
        .create_adjustment(&adjustment, Some(actor))
        .await
        .unwrap();
    assert_eq!(lines[0].delta_pieces, -13);
    assert_eq!(repo.adjustment_lines(adj.id).await.unwrap().len(), 1);
    assert!(repo
        .create_adjustment(&adjustment, Some(actor))
        .await
        .is_err());
    assert_eq!(repo.list_stock(&Default::default()).await.unwrap().total, 2);
    assert_eq!(
        repo.list_locations(&Default::default())
            .await
            .unwrap()
            .total,
        2
    );
    assert_eq!(
        repo.list_adjustments(&Default::default())
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        repo.list_transfer_requests(&Default::default())
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        repo.list_waste_events(&Default::default())
            .await
            .unwrap()
            .total,
        1
    );
    f.close().await;
    other.close().await;
}

#[tokio::test]
async fn procurement_snapshots_receipt_expense_idempotency_and_reorder() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let store = f.inventory(venue, "store").await;
    let product = f.product("Tea").await;
    let actor = f.staff().await;
    let vendor = f.supplier().await;
    let service = TenantProcurementService::new(f.db.clone(), "Asia/Kolkata".into());
    let before = f.outbox_count().await;
    for body in [
        json!({"vendorId":vendor.id,"destinationLocationId":store.id,"lines":[{"productId":product,"orderedBoxes":1,"boxCost":100},{"productId":product,"orderedBoxes":2,"boxCost":100}]}),
        json!({"vendorId":vendor.id,"destinationLocationId":store.id,"discount":-1,"lines":[{"productId":product,"orderedBoxes":1,"boxCost":100}]}),
        json!({"vendorId":vendor.id,"destinationLocationId":store.id,"freight":-1,"lines":[{"productId":product,"orderedBoxes":1,"boxCost":100}]})
    ] {
        assert!(service.create_order(serde_json::from_value(body).unwrap(),actor).await.is_err());
        assert_eq!(f.outbox_count().await,before);
        assert_eq!(f.scalar("SELECT COUNT(*) FROM purchase_orders").await,0);
    }
    let create:CreatePurchaseOrderDto=serde_json::from_value(json!({"vendorId":vendor.id,"destinationLocationId":store.id,"lines":[{"productId":product,"orderedBoxes":4,"boxCost":10.1234,"taxRate":5}]})).unwrap();
    let order = service.create_order(create, actor).await.unwrap();
    assert_eq!(order.order.subtotal, 40.4936);
    assert_eq!(order.order.tax, 2.0247);
    assert_eq!(order.lines[0].units_per_box_snapshot, 12);
    let id = order.order.id;
    let edit: UpdatePurchaseOrderDto =
        serde_json::from_value(json!({"version":99,"notes":"Stale"})).unwrap();
    assert!(service.update_order(id, edit, actor).await.is_err());
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE products SET units_per_purchase_unit=99 WHERE id=?")
                .bind(product.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let edit: UpdatePurchaseOrderDto =
        serde_json::from_value(json!({"version":1,"notes":"Header only"})).unwrap();
    let changed = service.update_order(id, edit, actor).await.unwrap();
    assert_eq!(changed.lines[0].units_per_box_snapshot, 12);
    service.transition(id, "submit", None, actor).await.unwrap();
    service
        .transition(id, "approve", None, actor)
        .await
        .unwrap();
    let line = changed.lines[0].id;
    let receive = |invoice: &str, accepted: i32, rejected: i32| {
        serde_json::from_value::<ReceivePurchaseOrderDto>(json!({"invoiceReference":invoice,"paymentMethod":"online","paymentAccount":"Bank","lines":[{"purchaseOrderLineId":line,"acceptedBoxes":accepted,"rejectedBoxes":rejected}]})).unwrap()
    };
    let result = service
        .receive(id, receive("INV-1", 2, 1), actor)
        .await
        .unwrap();
    assert_eq!(result.purchase_order.order.status, "partially_received");
    assert!(service.transition(id,"cancel",None,actor).await.is_err());
    assert!(service.transition(id,"receive",None,actor).await.is_err());
    assert_eq!(f.scalar("SELECT amount FROM expenses").await, 212591);
    assert_eq!(
        f.scalar("SELECT quantity_pieces FROM location_stock").await,
        24
    );
    let receipt_lines = TenantInventoryRepository::new(f.db.clone())
        .receipt_lines(result.receipt_id)
        .await
        .unwrap();
    assert_eq!(receipt_lines[0].box_quantity, 2);
    let before = f.outbox_count().await;
    assert!(service
        .receive(id, receive("INV-1", 1, 0), actor)
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM expenses").await, 1);
    assert!(service
        .receive(id, receive("OVER", 3, 0), actor)
        .await
        .is_err());
    assert!(service.receive(id,receive("REJECTED-OVER",1,2),actor).await.is_err());
    assert!(service.receive(id,receive("ZERO-ACCEPTED",0,1),actor).await.is_err());
    let mut cash = receive("CASH", 1, 0);
    cash.payment_method = "cash".into();
    assert!(service.receive(id, cash, actor).await.is_err());
    assert_eq!(f.scalar("SELECT COUNT(*) FROM stock_receipts").await, 1);
    let final_receipt = service
        .receive(id, receive("INV-2", 2, 0), actor)
        .await
        .unwrap();
    assert_eq!(final_receipt.purchase_order.order.status, "received");
    assert!(service.transition(id,"cancel",None,actor).await.is_err());
    let historical_expense =
        gaming_cafe_api::repositories::TenantExpenseRepository::new(f.db.clone());
    assert_eq!(
        historical_expense
            .location_id(result.expense_id)
            .await
            .unwrap(),
        Some(venue)
    );
    let moved_venue = f.location("moved-store").await;
    let store_id = store.id;
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE inventory_locations SET venue_location_id=? WHERE id=?")
                .bind(moved_venue.to_string())
                .bind(store_id.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert_eq!(
        historical_expense
            .location_id(result.expense_id)
            .await
            .unwrap(),
        Some(venue)
    );
    historical_expense
        .soft_delete(result.expense_id)
        .await
        .unwrap();
    let deleted_venue: String = sqlx::query_scalar("SELECT location_id FROM outbox_events WHERE aggregate_id=? AND event_type='expense.deleted'")
        .bind(result.expense_id.to_string()).fetch_one(&f.db.read_pool().unwrap()).await.unwrap();
    assert_eq!(deleted_venue, venue.to_string());

    assert_eq!(f.scalar("SELECT COUNT(*) FROM expenses").await, 2);
    let rule:UpsertInventoryReorderRuleDto=serde_json::from_value(json!({"locationId":store.id,"productId":product,"minimumPieces":50,"targetPieces":100,"preferredVendorId":vendor.id})).unwrap();
    service.upsert_reorder_rule(rule, actor).await.unwrap();
    assert_eq!(service.list_reorder_rules().await.unwrap().len(), 1);
    assert_eq!(
        service.reorder_suggestions().await.unwrap()[0].suggested_pieces,
        52
    );
    assert_eq!(
        service
            .list_movements(Default::default())
            .await
            .unwrap()
            .total,
        2
    );
    assert_eq!(
        service.list_orders(Default::default()).await.unwrap().total,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn suppliers_enforce_uniqueness_under_concurrent_writes() {
    let f = Fixture::new().await;
    let repo = TenantVendorRepository::new(f.db.clone());
    let dto: CreateVendorDto = serde_json::from_value(json!({"name":"Supplier"})).unwrap();
    let (a, b) = tokio::join!(repo.create(&dto, None), repo.create(&dto, None));
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let vendor = a.or(b).unwrap();
    assert_eq!(repo.list(&Default::default()).await.unwrap().total, 1);
    let update: UpdateVendorDto =
        serde_json::from_value(json!({"name":"Updated","isActive":false})).unwrap();
    let updated = repo.update(vendor.id, &update, None).await.unwrap();
    assert!(!updated.is_active);
    assert_eq!(
        repo.find_by_id(vendor.id).await.unwrap().unwrap().name,
        "Updated"
    );
    repo.delete(vendor.id).await.unwrap();
    assert!(repo.find_by_id(vendor.id).await.unwrap().is_none());
    assert_eq!(f.outbox_count().await, 3);
    f.close().await;
}

#[tokio::test]
async fn cash_receipts_require_same_venue_and_commit_all_financial_links() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let other_venue = f.location("other").await;
    let store = f.inventory(venue, "store").await;
    let product = f.product("Tea").await;
    let actor = f.staff().await;
    let vendor = f.supplier().await;
    let service = TenantProcurementService::new(f.db.clone(), "Asia/Kolkata".into());
    let order=service.create_order(serde_json::from_value(json!({"vendorId":vendor.id,"destinationLocationId":store.id,"lines":[{"productId":product,"orderedBoxes":1,"boxCost":10}]})).unwrap(),actor).await.unwrap();
    service
        .transition(order.order.id, "submit", None, actor)
        .await
        .unwrap();
    service
        .transition(order.order.id, "approve", None, actor)
        .await
        .unwrap();
    let shift = Uuid::now_v7();
    let register = Uuid::now_v7();
    f.db.with_immediate_writer(move|c|Box::pin(async move{
  let ts=gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
  sqlx::query("INSERT INTO shifts(id,user_id,location_id,clock_in,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(shift.to_string()).bind(actor.to_string()).bind(other_venue.to_string()).bind(&ts).bind(&ts).bind(&ts).execute(&mut *c).await?;
  sqlx::query("INSERT INTO cash_registers(id,shift_id,opened_by,opening_balance,created_at,updated_at) VALUES(?,?,?,1000000,?,?)").bind(register.to_string()).bind(shift.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(c).await?;Ok(())
 })).await.unwrap();
    let dto = || {
        serde_json::from_value::<ReceivePurchaseOrderDto>(json!({"invoiceReference":"CASH","paymentMethod":"cash","lines":[{"purchaseOrderLineId":order.lines[0].id,"acceptedBoxes":1}]})).unwrap()
    };
    let before = f.outbox_count().await;
    assert!(service.receive(order.order.id, dto(), actor).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(f.scalar("SELECT COUNT(*) FROM expenses").await, 0);
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE shifts SET location_id=? WHERE id=?")
                .bind(venue.to_string())
                .bind(shift.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    let result = service.receive(order.order.id, dto(), actor).await.unwrap();
    assert_eq!(result.purchase_order.order.status, "received");
    assert_eq!(
        f.scalar("SELECT amount FROM cash_register_entries").await,
        100000
    );
    assert_eq!(f.scalar("SELECT COUNT(*) FROM expenses e JOIN cash_register_entries ce ON ce.id=e.cash_register_entry_id AND ce.reference_id=e.id JOIN stock_receipts r ON r.id=e.source_id WHERE e.shift_id IS NOT NULL AND e.amount=ce.amount AND e.amount=r.total").await,1);
    f.close().await;
}

#[tokio::test]
async fn invalid_money_and_fenced_ownership_never_write() {
    let f = Fixture::new().await;
    let venue = f.location("main").await;
    let store = f.inventory(venue, "store").await;
    let product = f.product("Tea").await;
    let actor = f.staff().await;
    let vendor = f.supplier().await;
    let service = TenantProcurementService::new(f.db.clone(), "Asia/Kolkata".into());
    for cost in [f64::NAN, f64::INFINITY, -1.0, 0.00001, f64::MAX] {
        let dto = CreatePurchaseOrderDto {
            vendor_id: vendor.id,
            destination_location_id: store.id,
            expected_delivery_date: None,
            discount: None,
            freight: None,
            notes: None,
            lines: vec![PurchaseOrderLineInput {
                product_id: product,
                ordered_boxes: 1,
                box_cost: cost,
                tax_rate: None,
            }],
        };
        assert!(service.create_order(dto, actor).await.is_err());
    }
    assert_eq!(f.scalar("SELECT COUNT(*) FROM purchase_orders").await, 0);
    assert_eq!(
        f.scalar("SELECT next_value FROM purchase_order_number_counter")
            .await,
        1
    );
    let before = f.outbox_count().await;
    f.lease
        .generations
        .write()
        .unwrap()
        .insert(f.db.tenant_id(), 2);
    let vendors = TenantVendorRepository::new(f.db.clone());
    let dto: CreateVendorDto = serde_json::from_value(json!({"name":"Fenced"})).unwrap();
    assert!(vendors.create(&dto, None).await.is_err());
    // Fenced handles cannot open readers; inspect the file through an independent read-only connection.
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(tenant_path(&f.root, f.db.tenant_id()))
                .read_only(true),
        )
        .await
        .unwrap();
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(after, before);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vendors")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    pool.close().await;
    f.close().await;
}

#[tokio::test]
async fn scoped_inventory_and_procurement_deny_other_venues_before_side_effects() {
    let f = Fixture::new().await;
    let venue = f.location("permitted").await;
    let other = f.location("other-venue").await;
    let warehouse = f.inventory(venue, "warehouse").await;
    let store = f.inventory(venue, "store").await;
    let remote = f.inventory(other, "store").await;
    let product = f.product("Coffee").await;
    let actor = f.staff().await;
    let vendor = f.supplier().await;
    let trusted = TenantInventoryRepository::new(f.db.clone());
    let scoped = TenantInventoryRepository::scoped(f.db.clone(), vec![venue]);
    let empty = TenantInventoryRepository::scoped(f.db.clone(), vec![]);
    let receipt = |location| {
        serde_json::from_value::<CreateStockReceiptDto>(json!({"locationId":location,"exceptionalReason":"Opening stock","lines":[{"productId":product,"boxQuantity":2}]})).unwrap()
    };
    let (local_receipt, _) = trusted
        .create_receipt(&receipt(warehouse.id), Some(actor))
        .await
        .unwrap();
    let (remote_receipt, _) = trusted
        .create_receipt(&receipt(remote.id), Some(actor))
        .await
        .unwrap();
    let local = scoped
        .list_locations(&serde_json::from_value(json!({"limit":1})).unwrap())
        .await
        .unwrap();
    assert_eq!(local.total, 2);
    assert_eq!(local.data.len(), 1);
    assert_eq!(
        scoped.list_stock(&Default::default()).await.unwrap().total,
        1
    );
    assert_eq!(
        scoped
            .list_receipts(&Default::default())
            .await
            .unwrap()
            .total,
        1
    );
    assert_eq!(
        empty
            .list_locations(&Default::default())
            .await
            .unwrap()
            .total,
        0
    );
    assert_eq!(
        empty
            .list_receipts(&Default::default())
            .await
            .unwrap()
            .total,
        0
    );
    assert!(scoped.get_receipt(local_receipt.id).await.is_ok());
    assert!(scoped.get_location(remote.id).await.is_err());
    assert!(scoped.get_receipt(remote_receipt.id).await.is_err());
    assert!(scoped.receipt_lines(remote_receipt.id).await.is_err());
    assert!(scoped.stock_quantity_at(remote.id, product).await.is_err());
    let before = f.outbox_count().await;
    assert!(scoped
        .create_receipt(&receipt(remote.id), Some(actor))
        .await
        .is_err());
    assert!(scoped
        .update_location(
            store.id,
            &serde_json::from_value(json!({"venueLocationId":other})).unwrap(),
            Some(actor)
        )
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before);
    let remote_id = remote.id;
    let store_id = store.id;
    f.db.with_immediate_writer(move|c|Box::pin(async move {
        let at = gaming_cafe_api::tenancy::format_sqlite_timestamp(&Utc::now()).unwrap();
        for (key, location) in [("inventory.default_warehouse_id", remote_id), ("inventory.default_store_id", store_id)] {
            sqlx::query("INSERT INTO setting_overrides(id,key,value,created_at,updated_at) VALUES(?,?,?,?,?)")
                .bind(Uuid::now_v7().to_string()).bind(key).bind(serde_json::json!(location).to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        }
        Ok(())
    })).await.unwrap();
    let before_default_transfer = f.outbox_count().await;
    assert!(scoped
        .request_transfer(
            serde_json::from_value(json!({"lines":[{"productId":product,"quantityPieces":1}]}))
                .unwrap(),
            Some(actor)
        )
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before_default_transfer);
    let (cross, _) = trusted
        .create_transfer_request(warehouse.id, remote.id, &[(product, 1)], Some(actor))
        .await
        .unwrap();
    let (within, _) = trusted
        .create_transfer_request(warehouse.id, store.id, &[(product, 1)], Some(actor))
        .await
        .unwrap();
    assert_eq!(
        scoped
            .list_transfer_requests(&Default::default())
            .await
            .unwrap()
            .total,
        1
    );
    assert!(scoped.get_transfer_request(within.id).await.is_ok());
    assert!(scoped.get_transfer_request(cross.id).await.is_err());
    assert!(scoped.transfer_lines(cross.id).await.is_err());
    let before = f.outbox_count().await;
    assert!(scoped.approve_transfer(cross.id, actor).await.is_err());
    assert!(scoped
        .reject_transfer(cross.id, "Outside venue", actor)
        .await
        .is_err());
    assert!(scoped
        .create_transfer_request(warehouse.id, remote.id, &[(product, 1)], Some(actor))
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before);
    trusted.approve_transfer(cross.id, actor).await.unwrap();
    let before = f.outbox_count().await;
    assert!(scoped.fulfill_transfer(cross.id, actor).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    assert_eq!(
        trusted
            .stock_quantity_at(warehouse.id, product)
            .await
            .unwrap(),
        24
    );
    let (remote_adjustment, _) = trusted.create_adjustment(&serde_json::from_value(json!({"locationId":remote.id,"notes":"Stock count","lines":[{"productId":product,"countedPieces":25}]})).unwrap(), Some(actor)).await.unwrap();
    assert_eq!(
        scoped
            .list_adjustments(&Default::default())
            .await
            .unwrap()
            .total,
        0
    );
    assert!(scoped.get_adjustment(remote_adjustment.id).await.is_err());
    assert!(scoped.adjustment_lines(remote_adjustment.id).await.is_err());
    let (waste, _) = trusted.create_waste_event(&serde_json::from_value(json!({"locationId":remote.id,"lines":[{"productId":product,"quantityPieces":1,"reasonCode":"damaged"}]})).unwrap(), Some(actor)).await.unwrap();
    assert_eq!(
        scoped
            .list_waste_events(&Default::default())
            .await
            .unwrap()
            .total,
        0
    );
    assert!(scoped.get_waste_event(waste.id).await.is_err());
    assert!(scoped.waste_lines(waste.id).await.is_err());
    let before = f.outbox_count().await;
    assert!(scoped.approve_waste(waste.id, actor).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    let procurement = TenantProcurementService::new(f.db.clone(), "Asia/Kolkata".into());
    let allowed =
        TenantProcurementService::scoped(f.db.clone(), "Asia/Kolkata".into(), vec![venue]);
    let no_venues = TenantProcurementService::scoped(f.db.clone(), "Asia/Kolkata".into(), vec![]);
    let order = |destination| {
        serde_json::from_value::<CreatePurchaseOrderDto>(json!({"vendorId":vendor.id,"destinationLocationId":destination,"lines":[{"productId":product,"orderedBoxes":1,"boxCost":10}]})).unwrap()
    };
    let own_order = procurement
        .create_order(order(store.id), actor)
        .await
        .unwrap();
    let other_order = procurement
        .create_order(order(remote.id), actor)
        .await
        .unwrap();
    let page = allowed
        .list_orders(serde_json::from_value(json!({"limit":1})).unwrap())
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.data[0].id, own_order.order.id);
    assert_eq!(
        no_venues
            .list_orders(Default::default())
            .await
            .unwrap()
            .total,
        0
    );
    assert!(allowed.get_order(other_order.order.id).await.is_err());
    let before = f.outbox_count().await;
    assert!(allowed.create_order(order(remote.id), actor).await.is_err());
    assert!(allowed
        .transition(other_order.order.id, "submit", None, actor)
        .await
        .is_err());
    assert!(allowed
        .update_order(
            other_order.order.id,
            serde_json::from_value(json!({"version":1,"notes":"Blocked"})).unwrap(),
            actor
        )
        .await
        .is_err());
    assert!(allowed
        .update_order(
            own_order.order.id,
            serde_json::from_value(json!({"version":1,"destinationLocationId":remote.id})).unwrap(),
            actor
        )
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before);
    procurement
        .transition(other_order.order.id, "submit", None, actor)
        .await
        .unwrap();
    procurement
        .transition(other_order.order.id, "approve", None, actor)
        .await
        .unwrap();
    let before = f.outbox_count().await;
    assert!(allowed.receive(other_order.order.id, serde_json::from_value(json!({"invoiceReference":"Denied","paymentMethod":"online","paymentAccount":"Bank","lines":[{"purchaseOrderLineId":other_order.lines[0].id,"acceptedBoxes":1}]})).unwrap(), actor).await.is_err());
    assert_eq!(f.outbox_count().await, before);
    for location in [store.id, remote.id] {
        procurement.upsert_reorder_rule(serde_json::from_value(json!({"locationId":location,"productId":product,"minimumPieces":30,"targetPieces":50})).unwrap(), actor).await.unwrap();
    }
    assert_eq!(allowed.list_reorder_rules().await.unwrap().len(), 1);
    assert_eq!(allowed.reorder_suggestions().await.unwrap().len(), 1);
    assert!(no_venues.list_reorder_rules().await.unwrap().is_empty());
    assert!(no_venues.reorder_suggestions().await.unwrap().is_empty());
    let page = allowed
        .list_movements(serde_json::from_value(json!({"limit":1})).unwrap())
        .await
        .unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.data[0].location_id, warehouse.id);
    assert_eq!(
        no_venues
            .list_movements(Default::default())
            .await
            .unwrap()
            .total,
        0
    );
    // A scope resolved before a location move cannot write to its new venue.
    trusted
        .update_location(
            store.id,
            &serde_json::from_value(json!({"venueLocationId":other})).unwrap(),
            Some(actor),
        )
        .await
        .unwrap();
    let before = f.outbox_count().await;
    assert!(scoped
        .create_receipt(&receipt(store.id), Some(actor))
        .await
        .is_err());
    assert!(allowed
        .transition(own_order.order.id, "submit", None, actor)
        .await
        .is_err());
    assert_eq!(f.outbox_count().await, before);
    f.close().await;
}
