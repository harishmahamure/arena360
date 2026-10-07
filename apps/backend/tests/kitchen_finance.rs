//! Tenant kitchen tickets are payment-gated, atomic, idempotent and use exact sale snapshots.
mod support;
use gaming_cafe_api::{
    repositories::{
        TenantInventoryRepository, TenantProductRepository, TenantTransactionRepository,
    },
    services::TenantKitchenService,
};
use serde_json::json;
#[tokio::test]
async fn tickets_are_atomic_idempotent_opt_in_and_totals_are_exact() {
    let f = support::SessionFixture::new().await;
    let db = f.tenant.db.clone();
    let product = TenantProductRepository::new(db.clone())
        .create(
            &serde_json::from_value(json!({"name":"Kitchen test","price":0.1,"category":"meal"}))
                .unwrap(),
            None,
        )
        .await
        .unwrap();
    let inventory = TenantInventoryRepository::new(db.clone());
    let store = inventory
        .create_location(
            &serde_json::from_value(
                json!({"name":"Kitchen","kind":"store","venueLocationId":f.venue}),
            )
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    inventory.create_receipt(&serde_json::from_value(json!({"locationId":store.id,"exceptionalReason":"Kitchen acceptance stock","lines":[{"productId":product.id,"boxQuantity":20}]})).unwrap(),None).await.unwrap();
    let kitchen = TenantKitchenService::new(db.clone());
    let sales = TenantTransactionRepository::new(db.clone());
    let sale = |status: &str, quantity: i32| {
        serde_json::from_value(json!({"playerId":f.player,"transactionType":"product_purchase","paymentMethod":"online","paymentStatus":status,"onlinePaymentRefLast4":"1234","saleLocationId":store.id,"venueLocationId":f.venue,"lineItems":[{"productId":product.id,"quantity":quantity}]})).unwrap()
    };
    let pending = sales.create(sale("completed", 3), None).await.unwrap();
    assert_eq!(
        kitchen.list(false).await.unwrap(),
        json!([]),
        "menu opt-in is required"
    );
    let actor = f
        .tenant
        .staff(Some(f.venue), vec!["kitchen:write".into()])
        .await;
    kitchen
        .save_menu(product.id, true, "Hot kitchen", 10, 0, actor)
        .await
        .unwrap();
    assert!(
        sales.create(sale("pending", 3), None).await.is_err(),
        "new unpaid product purchases are rejected"
    );
    // Stored pending rows remain supported by payment completion (e.g. migrated transactions).
    let pending_id = pending.id;
    db.with_immediate_writer(move|c|Box::pin(async move {sqlx::query("UPDATE transactions SET payment_status='pending',paid_amount=0,online_amount=0 WHERE id=?").bind(pending_id.to_string()).execute(c).await?;Ok(())})).await.unwrap();
    assert_eq!(pending.amount, 0.3);
    assert_eq!(
        kitchen.list(false).await.unwrap(),
        json!([]),
        "unpaid orders must not cook"
    );
    for _ in 0..2 {
        sales
            .update(
                pending.id,
                &serde_json::from_value(json!({"paymentStatus":"completed"})).unwrap(),
                None,
            )
            .await
            .unwrap();
    }
    let tickets = kitchen.list(false).await.unwrap();
    assert_eq!(tickets.as_array().unwrap().len(), 1);
    assert_eq!(tickets[0]["items"][0]["quantity"], 3);
    assert_eq!(tickets[0]["items"][0]["station"], "Hot kitchen");
    db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE products SET name='Changed name' WHERE id=?")
                .bind(product.id.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert_eq!(
        kitchen.list(false).await.unwrap()[0]["items"][0]["name"],
        "Kitchen test"
    );
    let amount: i64 = sqlx::query_scalar("SELECT amount FROM transactions WHERE id=?")
        .bind(pending.id.to_string())
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap();
    assert_eq!(amount, 3000);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox_events")
        .fetch_one(&db.read_pool().unwrap())
        .await
        .unwrap();
    assert!(sales.create(sale("completed", 99), None).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM outbox_events")
            .fetch_one(&db.read_pool().unwrap())
            .await
            .unwrap(),
        count
    );
    assert_eq!(
        kitchen.list(false).await.unwrap().as_array().unwrap().len(),
        1
    );
    f.close().await;
}
