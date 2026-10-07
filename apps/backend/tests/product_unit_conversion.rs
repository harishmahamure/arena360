//! Purchase units and recipe sale deductions share the tenant's base stock unit.
mod support;
use gaming_cafe_api::{
    repositories::{TenantInventoryRepository, TenantTransactionRepository, TenantUnitRepository},
    services::{ProductRecipeService, ProductService},
};
use serde_json::json;
#[tokio::test]
async fn kilogram_receipts_and_twenty_gram_recipe_deductions_use_the_same_stock_unit() {
    let f = support::TenantFixture::new().await;
    let venue = f.venue("kitchen").await;
    let player = f.player("customer").await;
    let units = TenantUnitRepository::new(f.db.clone());
    let gram = units
        .create(
            &serde_json::from_value(json!({"name":"Gram","abbreviation":"g","type":"gram"}))
                .unwrap(),
            None,
        )
        .await
        .unwrap();
    let kilogram = units
        .create(
            &serde_json::from_value(
                json!({"name":"Kilogram","abbreviation":"kg","type":"kilogram"}),
            )
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    let products = ProductService::new();
    let ingredient = products.create_tenant(f.db.clone(),vec![venue],serde_json::from_value(json!({"name":"Paneer","price":0,"category":"other","unitId":gram.id,"purchaseUnitId":kilogram.id,"unitsPerPurchaseUnit":1,"isRawMaterial":true})).unwrap(),None).await.unwrap();
    assert_eq!(
        ingredient.units_per_purchase_unit, 1000,
        "metric conversion must override an obsolete factor"
    );
    let pizza = products
        .create_tenant(
            f.db.clone(),
            vec![venue],
            serde_json::from_value(json!({"name":"Pizza","price":10,"category":"meal"})).unwrap(),
            None,
        )
        .await
        .unwrap();
    ProductRecipeService::new()
        .save_tenant(
            f.db.clone(),
            pizza.id,
            serde_json::from_value(
                json!({"items":[{"ingredientId":ingredient.id,"quantity":20}],"optionGroups":[]}),
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let inventory = TenantInventoryRepository::new(f.db.clone());
    let store = inventory
        .create_location(
            &serde_json::from_value(
                json!({"name":"Kitchen","kind":"store","venueLocationId":venue}),
            )
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    let (_,lines)=inventory.create_receipt(&serde_json::from_value(json!({"locationId":store.id,"exceptionalReason":"Unit conversion test","lines":[{"productId":ingredient.id,"boxQuantity":20}]})).unwrap(),None).await.unwrap();
    assert_eq!(lines[0].pieces_added, 20_000);
    TenantTransactionRepository::new(f.db.clone()).create(serde_json::from_value(json!({"playerId":player,"transactionType":"product_purchase","paymentMethod":"online","paymentStatus":"completed","onlinePaymentRefLast4":"1234","saleLocationId":store.id,"venueLocationId":venue,"lineItems":[{"productId":pizza.id,"quantity":1}]})).unwrap(),None).await.unwrap();
    assert_eq!(
        inventory
            .stock_quantity_at(store.id, ingredient.id)
            .await
            .unwrap(),
        19_980
    );
    let delta: i32 =
        sqlx::query_scalar("SELECT delta FROM stock_movements WHERE movement_type='sale'")
            .fetch_one(&f.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(delta, -20);
    let stock: i32 =
        sqlx::query_scalar("SELECT SUM(quantity_pieces) FROM location_stock WHERE product_id=?")
            .bind(ingredient.id.to_string())
            .fetch_one(&f.db.read_pool().unwrap())
            .await
            .unwrap();
    assert_eq!(stock, 19_980);
    f.close().await;
}
