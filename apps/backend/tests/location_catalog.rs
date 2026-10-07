use gaming_cafe_api::{
    access::{self, scope::LocationScope},
    cache::NoopCache,
    dto::JwtUserClaims,
    models::{ProductFilterDto, DEFAULT_ORGANIZATION_ID},
    repositories::ProductRepository,
    services::catalog_scope::{self, CatalogScope, LocationPrice},
};
use serde_json::json;
use std::sync::Arc;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Executor,
};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires local PostgreSQL CREATE DATABASE; creates and removes an isolated database"]
async fn location_grants_catalog_visibility_prices_and_shared_write_boundary() {
    gaming_cafe_api::config::load_dotenv();
    let url = std::env::var("ANALYTICS_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap();
    let options: PgConnectOptions = url.parse().unwrap();
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(options.clone())
        .await
        .unwrap();
    let database = format!("scope_test_{}", Uuid::new_v4().simple());
    admin
        .execute(format!("CREATE DATABASE {database}").as_str())
        .await
        .unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect_with(options.database(&database))
        .await
        .unwrap();
    sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    let org = DEFAULT_ORGANIZATION_ID;
    let user = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let role = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,role) VALUES($1,$2,'unused','staff')")
        .bind(user)
        .bind(user.to_string())
        .execute(&pool)
        .await
        .unwrap();
    let membership:Uuid=sqlx::query_scalar(r#"INSERT INTO organization_memberships("organizationId","userId",role) VALUES($1,$2,'staff') ON CONFLICT("organizationId","userId") DO UPDATE SET role='staff' RETURNING id"#).bind(org).bind(user).fetch_one(&pool).await.unwrap();
    sqlx::query("DELETE FROM access_assignments WHERE organization_id=$1 AND user_id=$2")
        .bind(org)
        .bind(user)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(r#"DELETE FROM location_access_assignments WHERE "membershipId"=$1"#)
        .bind(membership)
        .execute(&pool)
        .await
        .unwrap();
    for id in [a, b] {
        sqlx::query(r#"INSERT INTO venue_locations(id,"organizationId",slug,name) VALUES($1,$2,$3,'Test location')"#).bind(id).bind(org).bind(id.to_string()).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO access_roles(id,organization_id,name,permissions) VALUES($1,$2,'Location test','[\"finance:read\",\"products:read\",\"products:write\"]')").bind(role).bind(org).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO access_assignments(organization_id,user_id,role_id) VALUES($1,$2,$3)")
        .bind(org)
        .bind(user)
        .bind(role)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(r#"INSERT INTO location_access_assignments("organizationId","membershipId","locationId") VALUES($1,$2,$3)"#).bind(org).bind(membership).bind(a).execute(&pool).await.unwrap();
    let claims:JwtUserClaims=serde_json::from_value(json!({"sub":user,"userId":user,"tenantId":org,"permissions":access::effective(&pool,org,user).await.unwrap(),"roles":["staff"],"allowedTenants":[org],"orgIds":[org],"iss":"gamezone","aud":"gamezone","appId":"admin"})).unwrap();
    let scope = LocationScope::resolve(&pool, &claims, "products:write", None)
        .await
        .unwrap();
    assert_eq!(scope.locations, vec![a]);
    assert!(!scope.organization_admin);
    assert!(
        LocationScope::resolve(&pool, &claims, "finance:read", Some(a))
            .await
            .is_ok()
    );
    assert!(
        LocationScope::resolve(&pool, &claims, "finance:read", Some(b))
            .await
            .is_err()
    );
    assert!(access::scope::require_organization_admin(&pool, org, user)
        .await
        .is_err());
    let product = ProductRepository::new(pool.clone())
        .with_locations(vec![a])
        .create(
            &serde_json::from_value(json!({"name":"Local coffee","price":20,"category":"other"}))
                .unwrap(),
            Some(user),
        )
        .await
        .unwrap();
    assert!(catalog_scope::available(&pool, "products", product.id, a)
        .await
        .is_ok());
    assert!(catalog_scope::available(&pool, "products", product.id, b)
        .await
        .is_err());
    assert!(catalog_scope::save(
        &pool,
        "products",
        product.id,
        &scope,
        CatalogScope {
            location_ids: vec![],
            prices: vec![]
        }
    )
    .await
    .is_err());
    let owner = LocationScope {
        organization_admin: true,
        locations: vec![a, b],
        ..scope.clone()
    };
    catalog_scope::save(
        &pool,
        "products",
        product.id,
        &owner,
        CatalogScope {
            location_ids: vec![a, b],
            prices: vec![LocationPrice {
                location_id: b,
                price: 30.0,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        catalog_scope::available(&pool, "products", product.id, b)
            .await
            .unwrap(),
        Some(30.0)
    );
    assert_eq!(
        catalog_scope::available(&pool, "products", product.id, a)
            .await
            .unwrap(),
        None
    );
    assert!(
        catalog_scope::get(&pool, "products", product.id, &scope, true)
            .await
            .is_err()
    );
    let result = ProductRepository::new(pool.clone())
        .list(&ProductFilterDto {
            organization_id: Some(org),
            allowed_location_ids: Some(vec![b]),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(result.total, 1);
    catalog_scope::save(
        &pool,
        "products",
        product.id,
        &owner,
        CatalogScope {
            location_ids: vec![a],
            prices: vec![],
        },
    )
    .await
    .unwrap();
    let result = ProductRepository::new(pool.clone())
        .list(&ProductFilterDto {
            organization_id: Some(org),
            allowed_location_ids: Some(vec![b]),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(result.total, 0);
    catalog_scope::save(
        &pool,
        "products",
        product.id,
        &owner,
        CatalogScope {
            location_ids: vec![],
            prices: vec![LocationPrice {
                location_id: b,
                price: 30.0,
            }],
        },
    )
    .await
    .unwrap();
    catalog_scope::save(
        &pool,
        "products",
        product.id,
        &scope,
        CatalogScope {
            location_ids: vec![],
            prices: vec![LocationPrice {
                location_id: a,
                price: 22.0,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        catalog_scope::available(&pool, "products", product.id, a)
            .await
            .unwrap(),
        Some(22.0)
    );
    assert_eq!(
        catalog_scope::available(&pool, "products", product.id, b)
            .await
            .unwrap(),
        Some(30.0)
    );
    assert!(catalog_scope::save(
        &pool,
        "products",
        product.id,
        &scope,
        CatalogScope {
            location_ids: vec![],
            prices: vec![LocationPrice {
                location_id: b,
                price: 5.0
            }]
        }
    )
    .await
    .is_err());
    let config = Arc::new(gaming_cafe_api::services::ConfigService::new(
        pool.clone(),
        Arc::new(NoopCache),
        "Asia/Kolkata".into(),
    ));
    let plans =
        gaming_cafe_api::services::PlanService::new(pool.clone(), Arc::new(NoopCache), config)
            .with_locations(vec![b]);
    let plan=plans.create(serde_json::from_value(json!({"name":"South pass","price":50,"planType":"time_based","timeCredits":60,"validityDays":30})).unwrap(),Some(user)).await.unwrap();
    plans
        .update(
            plan.id,
            serde_json::from_value(json!({"name":"South pass updated"})).unwrap(),
            Some(user),
        )
        .await
        .unwrap();
    assert!(plans.get_active_for(Some(a)).await.unwrap().is_empty());
    assert_eq!(plans.get_active_for(Some(b)).await.unwrap().len(), 1);
    catalog_scope::save(
        &pool,
        "plans",
        plan.id,
        &owner,
        CatalogScope {
            location_ids: vec![a, b],
            prices: vec![LocationPrice {
                location_id: b,
                price: 75.0,
            }],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        plans
            .get_by_id_for(plan.id, Some(b))
            .await
            .unwrap()
            .current_price,
        Some(75.0)
    );
    let pricing = gaming_cafe_api::services::PricingPolicyService::new(pool.clone());
    let (set,_)=pricing.create(org,serde_json::from_value(json!({"name":"Both venues","locationIds":[a,b],"policy":{"baseRate":"60","rules":[],"roundingScale":2}})).unwrap(),user).await.unwrap();
    assert_eq!(set.location_ids.len(), 2);
    assert!(pricing
        .list(org, Some(b))
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == set.id));
    // Client query parameters cannot broaden the server's scope, and cache keys include it.
    let parsed: ProductFilterDto =
        serde_json::from_value(json!({"organizationId":Uuid::new_v4(),"allowedLocationIds":[b]}))
            .unwrap();
    assert!(parsed.organization_id.is_none() && parsed.allowed_location_ids.is_none());
    let scoped = ProductFilterDto {
        organization_id: Some(org),
        allowed_location_ids: Some(vec![a]),
        ..Default::default()
    };
    assert_ne!(
        gaming_cafe_api::cache::keys::filter_hash(&scoped),
        gaming_cafe_api::cache::keys::filter_hash(&parsed)
    );
    pool.close().await;
    admin
        .execute(format!("DROP DATABASE {database}").as_str())
        .await
        .unwrap();
}
