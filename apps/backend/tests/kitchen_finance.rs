//! DATABASE_URL=<isolated QA database> cargo test --test kitchen_finance -- --ignored
use gaming_cafe_api::services::kitchen_service::enqueue;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires an isolated migrated DATABASE_URL; all fixtures roll back"]
async fn tickets_are_atomic_idempotent_opt_in_and_reports_use_exact_totals() {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&std::env::var("DATABASE_URL").expect("isolated DATABASE_URL"))
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let user = Uuid::new_v4();
    let product = Uuid::new_v4();
    let sale = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,role) VALUES($1,$2,'unused','player')",
    )
    .bind(user)
    .bind(format!("kitchen-{user}"))
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query(r#"INSERT INTO products(id,name,price,"dayPrice","nightPrice") VALUES($1,'Kitchen test',0.10,0.10,0.10)"#)
        .bind(product)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(r#"INSERT INTO transactions(id,"playerId",amount,"paymentMethod","paymentStatus","transactionDate","transactionType") VALUES($1,$2,0.30,'cash','pending','1901-01-01T12:00:00Z','product_purchase')"#).bind(sale).bind(user).execute(&mut *tx).await.unwrap();
    sqlx::query(r#"INSERT INTO transaction_products(id,"transactionId","productId",quantity,"unitPrice","priceAtPurchase",subtotal) VALUES(gen_random_uuid(),$1,$2,3,0.10,0.10,0.30)"#).bind(sale).bind(product).execute(&mut *tx).await.unwrap();
    enqueue(&mut tx, sale, Some(user)).await.unwrap();
    let count = |id| {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM kitchen_tickets WHERE transaction_id=$1")
            .bind(id)
    };
    assert_eq!(count(sale).fetch_one(&mut *tx).await.unwrap(), 0);
    sqlx::query("INSERT INTO kitchen_menu_settings(product_id,enabled,station,prep_minutes) VALUES($1,true,'Hot kitchen',10)").bind(product).execute(&mut *tx).await.unwrap();
    enqueue(&mut tx, sale, Some(user)).await.unwrap();
    assert_eq!(
        count(sale).fetch_one(&mut *tx).await.unwrap(),
        0,
        "unpaid orders must not cook"
    );
    sqlx::query(r#"UPDATE transactions SET "paymentStatus"='completed' WHERE id=$1"#)
        .bind(sale)
        .execute(&mut *tx)
        .await
        .unwrap();
    enqueue(&mut tx, sale, Some(user)).await.unwrap();
    enqueue(&mut tx, sale, Some(user)).await.unwrap();
    assert_eq!(count(sale).fetch_one(&mut *tx).await.unwrap(), 1);
    let items: Value =
        sqlx::query_scalar("SELECT items FROM kitchen_tickets WHERE transaction_id=$1")
            .bind(sale)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(items[0]["quantity"], 3);
    assert_eq!(items[0]["station"], "Hot kitchen");
    sqlx::query("UPDATE products SET name='Changed name' WHERE id=$1")
        .bind(product)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert_eq!(items[0]["name"], "Kitchen test");
    let category = Uuid::new_v4();
    sqlx::query("INSERT INTO expense_categories(id,name) VALUES($1,'Kitchen QA ingredients')")
        .bind(category)
        .execute(&mut *tx)
        .await
        .unwrap();
    for (status, amount, at) in [
        ("approved", "0.20", "1901-01-01T12:00:00Z"),
        ("pending", "0.40", "1901-01-01T12:00:00Z"),
        ("approved", "99.00", "1901-01-03T00:00:00Z"),
    ] {
        sqlx::query(r#"INSERT INTO expenses("categoryId",amount,"paymentMethod","approvalStatus","expenseDate")
            VALUES($1,$2::text::numeric,'cash',$3,$4::text::timestamptz)"#)
            .bind(category).bind(amount).bind(status).bind(at).execute(&mut *tx).await.unwrap();
    }
    let start = "1901-01-01T00:00:00Z"
        .parse::<chrono::DateTime<chrono::Utc>>()
        .unwrap();
    let end = start + chrono::Duration::days(2);
    let report: Value = sqlx::query_scalar(include_str!("fixtures/legacy_finance_report.sql"))
        .bind(start)
        .bind(end)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(report["sales"], "0.30");
    assert_eq!(report["saleCount"], 1);
    assert_eq!(report["approvedExpenses"], "0.2000");
    assert_eq!(report["pendingExpenses"], "0.4000");
    assert_eq!(report["expensesByCategory"][0]["count"], 1);
    assert_eq!(report["daily"][0]["expenses"], "0.2000");
    assert_eq!(report["daily"].as_array().unwrap().len(), 2);
    assert_eq!(report["daily"][1]["sales"], "0");
    sqlx::query(r#"UPDATE transactions SET "paymentStatus"='refunded' WHERE id=$1"#)
        .bind(sale)
        .execute(&mut *tx)
        .await
        .unwrap();
    let report: Value = sqlx::query_scalar(include_str!("fixtures/legacy_finance_report.sql"))
        .bind(start)
        .bind(end)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(report["sales"], "0");
    assert_eq!(report["refundedSales"], "0.30");
    tx.rollback().await.unwrap();
    assert_eq!(
        count(sale).fetch_one(&pool).await.unwrap(),
        0,
        "rollback must remove ticket with sale"
    );
}
