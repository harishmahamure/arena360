//! DATABASE_URL=<migrated database> cargo test --test product_unit_conversion -- --ignored
//! Every fixture uses connection-local temporary tables; permanent data is untouched.
use gaming_cafe_api::models::{CreateStockReceiptDto, ProductRecipe, RecipeIngredient};
use gaming_cafe_api::repositories::InventoryRepository;
use gaming_cafe_api::services::ProductRecipeService;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires PostgreSQL; fixtures are temporary tables"]
async fn kilogram_receipts_and_twenty_gram_recipe_deductions_use_the_same_stock_unit() {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL"))
        .await
        .unwrap();
    for table in [
        "units",
        "products",
        "inventory_locations",
        "location_stock",
        "stock_movements",
        "stock_receipts",
        "stock_receipt_lines",
        "purchase_orders",
        "purchase_order_lines",
    ] {
        sqlx::query(&format!(
            "CREATE TEMP TABLE {table} (LIKE public.{table} INCLUDING ALL)"
        ))
        .execute(&pool)
        .await
        .unwrap();
    }
    let product = Uuid::new_v4();
    let location = Uuid::new_v4();
    let gram = Uuid::new_v4();
    let kilogram = Uuid::new_v4();
    sqlx::query("INSERT INTO units(id,name,abbreviation,type) VALUES($1,'Gram','g','gram'),($2,'Kilogram','kg','kilogram')")
        .bind(gram).bind(kilogram).execute(&pool).await.unwrap();
    sqlx::query(r#"INSERT INTO products(id,name,price,"dayPrice","nightPrice","unitId","purchaseUnitId","unitsPerPurchaseUnit","isRawMaterial") VALUES($1,'Paneer Topping',0,0,0,$2,$3,1,true)"#)
        .bind(product).bind(gram).bind(kilogram).execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO inventory_locations(id,name,kind) VALUES($1,'Kitchen test store','store')",
    )
    .bind(location)
    .execute(&pool)
    .await
    .unwrap();
    for status in ["ordered", "received"] {
        let order = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO purchase_orders(id,"poNumber","vendorId","destinationLocationId",status) VALUES($1,$2,$3,$4,$2::purchase_order_status)"#)
            .bind(order).bind(status).bind(Uuid::new_v4()).bind(location).execute(&pool).await.unwrap();
        sqlx::query(r#"INSERT INTO purchase_order_lines("purchaseOrderId","productId","orderedBoxes","unitsPerBoxSnapshot","boxCostSnapshot","lineSubtotal","lineTax","lineTotal") VALUES($1,$2,20,1,100,2000,0,2000)"#)
            .bind(order).bind(product).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql(include_str!(
        "../migrations/20261003080000_metric_purchase_conversions.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();

    let snapshots: Vec<(String, i32)> = sqlx::query_as(r#"SELECT po.status::text,line."unitsPerBoxSnapshot" FROM purchase_orders po JOIN purchase_order_lines line ON line."purchaseOrderId"=po.id ORDER BY po.status::text"#)
        .fetch_all(&pool).await.unwrap();
    assert_eq!(
        snapshots,
        vec![("ordered".to_string(), 1000), ("received".to_string(), 1)]
    );

    let receipt: CreateStockReceiptDto = serde_json::from_value(serde_json::json!({
        "locationId": location, "exceptionalReason": "Unit conversion test",
        "lines": [{ "productId": product, "boxQuantity": 20 }]
    }))
    .unwrap();
    let repo = InventoryRepository::new(pool.clone());
    let (_, lines) = repo.create_receipt(&receipt, None).await.unwrap();
    assert_eq!(lines[0].pieces_added, 20_000);

    let recipe = ProductRecipe {
        items: vec![RecipeIngredient {
            ingredient_id: product,
            quantity: 20,
        }],
        option_groups: vec![],
    };
    let selection = ProductRecipeService::select_options("Pizza", &recipe, &[]).unwrap();
    let mut tx = pool.begin().await.unwrap();
    for (ingredient, quantity) in selection.ingredients {
        InventoryRepository::deduct_sale_stock_in_tx(
            &mut tx,
            location,
            ingredient,
            quantity,
            Uuid::new_v4(),
            None,
        )
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    assert_eq!(
        repo.stock_quantity_at(location, product).await.unwrap(),
        19_980
    );
    let sale_delta: i32 =
        sqlx::query_scalar(r#"SELECT delta FROM stock_movements WHERE "movementType" = 'sale'"#)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sale_delta, -20);
    let catalog_stock: i32 =
        sqlx::query_scalar(r#"SELECT "stockQuantity" FROM products WHERE id=$1"#)
            .bind(product)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(catalog_stock, 19_980);
    pool.close().await;
}
