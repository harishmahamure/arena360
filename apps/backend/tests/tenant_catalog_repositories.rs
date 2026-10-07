use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use gaming_cafe_api::error::AppError;
use gaming_cafe_api::models::{
    CreateGameDto, CreatePlanDto, CreateProductDto, CreateUnitDto, GameFilterDto, PlanFilterDto,
    ProductFilterDto, ProductOption, ProductOptionGroup, ProductRecipe, RecipeIngredient,
    SettingHistoryQuery, UnitFilterDto, UpsertSettingOverrideDto,
};
use gaming_cafe_api::repositories::{
    TenantGameRepository, TenantPlanCreateValues, TenantPlanRepository,
    TenantPricingPolicyRepository, TenantProductRecipeRepository, TenantProductRepository,
    TenantSettingsRepository, TenantUnitRepository,
};
use gaming_cafe_api::services::GameService;
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
async fn catalog_crud_filters_money_locations_and_recipe_replacement_are_tenant_local() {
    let fixture = Fixture::new().await;
    let location_a = fixture.location("alpha").await;
    let location_b = fixture.location("beta").await;

    let units = TenantUnitRepository::new(fixture.db.clone());
    let unit = units
        .create(
            &serde_json::from_value::<CreateUnitDto>(json!({
                "name": "Serving", "abbreviation": "srv", "type": "other"
            }))
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        units.list(&UnitFilterDto::default()).await.unwrap().total,
        1
    );
    units.soft_delete(unit.id).await.unwrap();
    let deactivated = units.find_by_id(unit.id).await.unwrap().unwrap();
    assert!(!deactivated.is_active);
    assert!(deactivated.deleted_at.is_none());

    let all_products = TenantProductRepository::new(fixture.db.clone());
    let scoped_products =
        TenantProductRepository::new(fixture.db.clone()).with_locations(vec![location_a]);
    let ingredient = all_products
        .create(
            &product("Coffee beans", "RAW-1", 0.1234, unit.id, true),
            None,
        )
        .await
        .unwrap();
    let drink = scoped_products
        .create(&product("Coffee", "SALE-1", 12.3456, unit.id, false), None)
        .await
        .unwrap();
    assert_eq!(drink.price, 12.3456);

    let at_a = all_products
        .list(&ProductFilterDto {
            location_id: Some(location_a),
            limit: Some(100),
            ..Default::default()
        })
        .await
        .unwrap();
    let at_b = all_products
        .list(&ProductFilterDto {
            location_id: Some(location_b),
            limit: Some(100),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(at_a.data.iter().any(|row| row.id == drink.id));
    assert!(!at_b.data.iter().any(|row| row.id == drink.id));
    assert!(at_b.data.iter().any(|row| row.id == ingredient.id));

    let beta_products =
        TenantProductRepository::new(fixture.db.clone()).with_locations(vec![location_b]);
    let beta_only = beta_products
        .create(&product("Beta snack", "SALE-B", 7.0, unit.id, false), None)
        .await
        .unwrap();
    let authorized_a_with_raw_b = all_products
        .list(&ProductFilterDto {
            location_id: Some(location_b),
            allowed_location_ids: Some(vec![location_a]),
            limit: Some(100),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(authorized_a_with_raw_b
        .data
        .iter()
        .any(|row| row.id == drink.id));
    assert!(!authorized_a_with_raw_b
        .data
        .iter()
        .any(|row| row.id == beta_only.id));

    all_products
        .replace_location_scope(
            drink.id,
            vec![location_a, location_b],
            vec![(location_a, 13.0), (location_b, 14.0)],
        )
        .await
        .unwrap();
    all_products
        .replace_authorized_location_prices(
            drink.id,
            vec![location_a, location_b],
            vec![(location_a, 15.0)],
            vec![location_a],
        )
        .await
        .unwrap();
    let (_, prices_after_staff_update) = all_products.location_scope(drink.id).await.unwrap();
    assert_eq!(
        prices_after_staff_update,
        vec![(location_a, Some(15.0)), (location_b, Some(14.0))]
    );

    all_products
        .replace_location_scope(drink.id, vec![location_b], vec![(location_b, 13.4567)])
        .await
        .unwrap();
    let (all, product_scope) = all_products.location_scope(drink.id).await.unwrap();
    assert!(!all);
    assert_eq!(product_scope, vec![(location_b, Some(13.4567))]);

    let plans = TenantPlanRepository::new(fixture.db.clone()).with_locations(vec![location_a]);
    let plan_dto: CreatePlanDto = serde_json::from_value(json!({
        "name": "Two hours", "price": 99.9999, "planType": "time_based"
    }))
    .unwrap();
    let plan = plans
        .create(
            TenantPlanCreateValues {
                dto: &plan_dto,
                validity_days: 30,
                time_credits: 120,
                time_window_start: None,
                time_window_end: None,
                dynamic_deduction_enabled: false,
                deduction_profile: None,
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(plan.price, 99.9999);
    assert_eq!(
        TenantPlanRepository::new(fixture.db.clone())
            .list(&PlanFilterDto {
                location_id: Some(location_b),
                ..Default::default()
            })
            .await
            .unwrap()
            .total,
        0
    );
    let all_plans = TenantPlanRepository::new(fixture.db.clone());
    all_plans
        .replace_location_scope(plan.id, vec![location_b], vec![(location_b, 88.7654)])
        .await
        .unwrap();
    let (all, plan_scope) = all_plans.location_scope(plan.id).await.unwrap();
    assert!(!all);
    assert_eq!(plan_scope, vec![(location_b, Some(88.7654))]);

    let games = TenantGameRepository::new(fixture.db.clone());
    let game = games
        .create(
            &serde_json::from_value::<CreateGameDto>(json!({
                "name": "Arena", "sortOrder": 2
            }))
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        games
            .list(&GameFilterDto {
                name: Some("ren".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .data[0]
            .id,
        game.id
    );

    let recipes = TenantProductRecipeRepository::new(fixture.db.clone());
    let group_id = Uuid::now_v7();
    let option_id = Uuid::now_v7();
    let original = ProductRecipe {
        items: vec![RecipeIngredient {
            ingredient_id: ingredient.id,
            quantity: 2,
        }],
        option_groups: vec![ProductOptionGroup {
            id: Some(group_id),
            name: "Size".into(),
            required: true,
            multiple: false,
            options: vec![ProductOption {
                id: Some(option_id),
                name: "Large".into(),
                price_delta: 1.2345,
                ingredients: vec![],
            }],
        }],
    };
    recipes.replace(drink.id, &original).await.unwrap();
    assert_eq!(recipes.get(drink.id).await.unwrap().items, original.items);
    assert_eq!(
        recipes.recipe_requirements().await.unwrap(),
        vec![(drink.id, ingredient.id, 2)]
    );
    let store_id = Uuid::now_v7();
    let warehouse_id = Uuid::now_v7();
    let timestamp = gaming_cafe_api::time::format_sqlite_timestamp(&Utc::now()).unwrap();
    fixture
        .db
        .with_writer(|connection| {
            Box::pin(async move {
                for (id, kind) in [(store_id, "store"), (warehouse_id, "warehouse")] {
                    sqlx::query(
                        "INSERT INTO inventory_locations(
                            id,venue_location_id,name,kind,created_at,updated_at
                         ) VALUES (?,?,?,?,?,?)",
                    )
                    .bind(id.to_string())
                    .bind(location_a.to_string())
                    .bind(kind)
                    .bind(kind)
                    .bind(&timestamp)
                    .bind(&timestamp)
                    .execute(&mut *connection)
                    .await?;
                }
                for (id, quantity) in [(store_id, 20), (warehouse_id, 4)] {
                    sqlx::query(
                        "INSERT INTO location_stock(
                            inventory_location_id,product_id,quantity_pieces,created_at,updated_at
                         ) VALUES (?,?,?,?,?)",
                    )
                    .bind(id.to_string())
                    .bind(ingredient.id.to_string())
                    .bind(quantity)
                    .bind(&timestamp)
                    .bind(&timestamp)
                    .execute(&mut *connection)
                    .await?;
                }
                Ok(())
            })
        })
        .await
        .unwrap();
    let direct_stock = recipes
        .made_to_order_stock(Some(warehouse_id), location_a)
        .await
        .unwrap();
    assert_eq!(direct_stock, vec![(drink.id, 2, 4)]);
    let venue_store_stock = recipes.made_to_order_stock(None, location_a).await.unwrap();
    assert_eq!(venue_store_stock, vec![(drink.id, 2, 20)]);

    let invalid = ProductRecipe {
        items: vec![RecipeIngredient {
            ingredient_id: Uuid::now_v7(),
            quantity: 1,
        }],
        option_groups: vec![],
    };
    assert!(recipes.replace(drink.id, &invalid).await.is_err());
    assert_eq!(recipes.get(drink.id).await.unwrap().items, original.items);
    assert!(fixture.outbox_count().await >= 6);
    fixture.close().await;
}

#[tokio::test]
async fn pricing_versions_and_setting_revisions_publish_atomically() {
    let fixture = Fixture::new().await;
    let location = fixture.location("main").await;
    let actor = Uuid::now_v7();
    let pricing = TenantPricingPolicyRepository::new(fixture.db.clone());
    let (set, draft) = pricing
        .create_set(
            fixture.tenant_id,
            Some(location),
            &[location],
            "Standard",
            None,
            &json!({"baseRate":"100.0000","rules":[]}),
            actor,
        )
        .await
        .unwrap();
    assert_eq!(set.location_ids, vec![location]);
    pricing
        .mark_validated(fixture.tenant_id, set.id, draft.id)
        .await
        .unwrap();
    pricing
        .record_simulation(fixture.tenant_id, set.id, draft.id, "abc123")
        .await
        .unwrap();
    let scheduled = pricing
        .publish(
            fixture.tenant_id,
            set.id,
            draft.id,
            Utc::now() + ChronoDuration::minutes(5),
            actor,
        )
        .await
        .unwrap();
    assert_eq!(scheduled.status, "scheduled");
    fixture
        .db
        .with_writer(|connection| {
            Box::pin(async move {
                sqlx::query(
                    "UPDATE pricing_rule_versions
                     SET effective_at='2020-01-01T00:00:00.000000Z' WHERE id=?",
                )
                .bind(draft.id.to_string())
                .execute(connection)
                .await?;
                Ok(())
            })
        })
        .await
        .unwrap();
    let activated = pricing.activate_due().await.unwrap();
    assert_eq!(activated.len(), 1);
    assert_eq!(
        pricing
            .active_policies(fixture.tenant_id, Some(location))
            .await
            .unwrap()
            .len(),
        1
    );
    let replacement = pricing
        .create_version(
            fixture.tenant_id,
            set.id,
            &json!({"baseRate":"120.0000","rules":[]}),
            actor,
        )
        .await
        .unwrap();
    pricing
        .mark_validated(fixture.tenant_id, set.id, replacement.id)
        .await
        .unwrap();
    pricing
        .record_simulation(fixture.tenant_id, set.id, replacement.id, "def456")
        .await
        .unwrap();
    pricing
        .publish(fixture.tenant_id, set.id, replacement.id, Utc::now(), actor)
        .await
        .unwrap();
    pricing
        .record_simulation(
            fixture.tenant_id,
            set.id,
            replacement.id,
            "ignored-after-publication",
        )
        .await
        .unwrap();
    assert_eq!(
        pricing
            .get_version(fixture.tenant_id, set.id, replacement.id)
            .await
            .unwrap()
            .simulation_hash
            .as_deref(),
        Some("def456")
    );
    assert_eq!(
        pricing
            .get_version(fixture.tenant_id, set.id, draft.id)
            .await
            .unwrap()
            .status,
        "superseded"
    );

    let settings = TenantSettingsRepository::new(fixture.db.clone());
    let first = settings
        .upsert_override(
            fixture.tenant_id,
            "pricing.currency",
            &setting(location, json!("USD"), Some(0), "initial"),
            actor,
            Some("req-1"),
            false,
        )
        .await
        .unwrap();
    assert_eq!(first.revision, 1);
    let conflict = settings
        .upsert_override(
            fixture.tenant_id,
            "pricing.currency",
            &setting(location, json!("EUR"), Some(0), "stale"),
            actor,
            None,
            false,
        )
        .await;
    assert!(matches!(
        conflict,
        Err(AppError::Conflict(_)) | Err(AppError::Api { .. })
    ));
    assert_eq!(
        settings
            .find_override(fixture.tenant_id, Some(location), "pricing.currency")
            .await
            .unwrap()
            .unwrap()
            .value,
        json!("USD")
    );

    let outbox_before = fixture.outbox_count().await;
    assert!(settings
        .upsert_override(
            fixture.tenant_id,
            "pricing.tax",
            &setting(location, json!(0.18), None, ""),
            actor,
            None,
            false,
        )
        .await
        .is_err());
    assert!(settings
        .find_override(fixture.tenant_id, Some(location), "pricing.tax")
        .await
        .unwrap()
        .is_none());
    assert_eq!(fixture.outbox_count().await, outbox_before);

    assert_eq!(
        settings
            .history(
                fixture.tenant_id,
                &SettingHistoryQuery {
                    location_id: Some(location),
                    key: Some("pricing.currency".into()),
                    limit: None,
                },
            )
            .await
            .unwrap()
            .len(),
        1
    );
    fixture.close().await;
}

#[tokio::test]
async fn tenant_service_path_does_not_touch_postgres() {
    let fixture = Fixture::new().await;
    let postgres = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let cache = gaming_cafe_api::cache::create_cache(None).await;
    let service = GameService::new(postgres, cache);
    let game = service
        .create_tenant(
            fixture.db.clone(),
            serde_json::from_value(json!({"name":"Tenant-only game"})).unwrap(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(
        service
            .get_tenant(fixture.db.clone(), game.id)
            .await
            .unwrap()
            .name,
        "Tenant-only game"
    );
    fixture.close().await;
}

fn product(name: &str, sku: &str, price: f64, unit_id: Uuid, raw: bool) -> CreateProductDto {
    serde_json::from_value(json!({
        "name": name,
        "price": price,
        "unitId": unit_id,
        "purchaseUnitId": unit_id,
        "sku": sku,
        "isRawMaterial": raw
    }))
    .unwrap()
}

fn setting(
    location_id: Uuid,
    value: serde_json::Value,
    expected_revision: Option<i64>,
    reason: &str,
) -> UpsertSettingOverrideDto {
    UpsertSettingOverrideDto {
        location_id: Some(location_id),
        value,
        reason: reason.into(),
        expected_revision,
    }
}

struct Fixture {
    root: PathBuf,
    tenant_id: Uuid,
    db: Arc<TenantDb>,
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
        sqlx::query("INSERT INTO tenant_runtime(singleton,timezone) VALUES(1,'Asia/Kolkata')")
            .execute(&pool)
            .await
            .unwrap();
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
            lease,
        )
        .unwrap();
        let db = manager.open(tenant_id).await.unwrap();
        Self {
            root,
            tenant_id,
            db,
        }
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

#[tokio::test]
async fn product_prices_and_settings_use_only_the_selected_tenant() {
    use gaming_cafe_api::services::{ConfigService, ProductService};
    let f = Fixture::new().await;
    let venue = f.location("priced").await;
    let unit = TenantUnitRepository::new(f.db.clone())
        .create(
            &serde_json::from_value(json!({"name":"Serving","abbreviation":"srv","type":"other"}))
                .unwrap(),
            None,
        )
        .await
        .unwrap();
    let item = TenantProductRepository::new(f.db.clone())
        .create(
            &product("Tenant coffee", "ONLY-1", 10.0, unit.id, false),
            None,
        )
        .await
        .unwrap();
    let unavailable = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    let cache = gaming_cafe_api::cache::create_cache(None).await;
    let settings = ConfigService::new(
        unavailable.clone(),
        cache.clone(),
        "America/New_York".into(),
    );
    let products = ProductService::new(unavailable, cache);
    let values = settings
        .effective_tenant(
            f.db.clone(),
            f.tenant_id,
            gaming_cafe_api::models::EffectiveSettingsQuery {
                location_id: Some(venue),
                category: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(
        values
            .iter()
            .find(|s| s.key == "venue.timezone")
            .unwrap()
            .value,
        json!("Asia/Kolkata")
    );
    let prices = products
        .current_prices_tenant(f.db.clone(), None, venue, &settings)
        .await
        .unwrap();
    let price = prices.iter().find(|p| p.product_id == item.id).unwrap();
    assert_eq!(price.price, 10.0);
    assert_eq!(price.made_to_order_available, None);
    f.close().await;
}
