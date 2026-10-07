mod support;
use gaming_cafe_api::{
    access::{self, scope::LocationScope},
    cache::NoopCache,
    dto::JwtUserClaims,
    models::ProductFilterDto,
    repositories::{TenantProductRepository, TenantSettingsRepository},
    services::catalog_scope::{self, CatalogScope, LocationPrice},
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use support::TenantFixture;
use uuid::Uuid;

#[tokio::test]
async fn location_grants_catalog_visibility_prices_and_shared_write_boundary() {
    let fixture = TenantFixture::new().await;
    let db = fixture.db.clone();
    let org = db.tenant_id();
    let user = Uuid::now_v7();
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    let role = Uuid::now_v7();
    db.with_immediate_writer(move |c| Box::pin(async move {
        let at = gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();
        sqlx::query("INSERT INTO users(id,username,role,created_at,updated_at) VALUES(?,'catalog-staff','staff',?,?)").bind(user.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        for id in [a,b] {
            sqlx::query("INSERT INTO venue_locations(id,slug,name,created_at,updated_at) VALUES(?,?,?,?,?)").bind(id.to_string()).bind(id.to_string()).bind(format!("Venue {id}")).bind(&at).bind(&at).execute(&mut *c).await?;
        }
        sqlx::query("INSERT INTO access_roles(id,name,permissions,created_at,updated_at) VALUES(?,'Location test','[\"finance:read\",\"products:read\",\"products:write\",\"plans:read\",\"plans:write\"]',?,?)").bind(role.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO access_assignments(user_id,role_id,created_at) VALUES(?,?,?)").bind(user.to_string()).bind(role.to_string()).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO location_role_assignments(user_id,location_id,role_id,created_at) VALUES(?,?,?,?)").bind(user.to_string()).bind(a.to_string()).bind(role.to_string()).bind(&at).execute(&mut *c).await?;
        for (key, value) in [("plans.default_validity_days","9"),("plans.default_time_credits","90")] {
            sqlx::query("INSERT INTO setting_overrides(id,key,value,created_at,updated_at) VALUES(?,?,?,?,?)").bind(Uuid::now_v7().to_string()).bind(key).bind(value).bind(&at).bind(&at).execute(&mut *c).await?;
        }
        Ok(())
    })).await.unwrap();
    let permissions = TenantSettingsRepository::new(db.clone())
        .effective_permissions(user)
        .await
        .unwrap();
    let claims:JwtUserClaims=serde_json::from_value(json!({"sub":user,"userId":user,"tenantId":org,"permissions":permissions,"roles":["staff"],"allowedTenants":[org],"orgIds":[org],"iss":"gamezone","aud":"gamezone","appId":"admin"})).unwrap();
    let scope = LocationScope::resolve_tenant(db.clone(), &claims, "products:write", None)
        .await
        .unwrap();
    assert_eq!(scope.locations, vec![a]);
    assert!(!scope.organization_admin);
    assert!(
        LocationScope::resolve_tenant(db.clone(), &claims, "finance:read", Some(a))
            .await
            .is_ok()
    );
    assert!(
        LocationScope::resolve_tenant(db.clone(), &claims, "finance:read", Some(b))
            .await
            .is_err()
    );
    assert!(access::scope::require_tenant_admin(db.clone(), org, user)
        .await
        .is_err());
    let product = TenantProductRepository::new(db.clone())
        .with_locations(vec![a])
        .create(
            &serde_json::from_value(json!({"name":"Local coffee","price":20,"category":"other"}))
                .unwrap(),
            Some(user),
        )
        .await
        .unwrap();
    assert!(
        catalog_scope::available(db.clone(), "products", product.id, a)
            .await
            .is_ok()
    );
    assert!(
        catalog_scope::available(db.clone(), "products", product.id, b)
            .await
            .is_err()
    );
    assert!(catalog_scope::save(
        db.clone(),
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
        db.clone(),
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
        catalog_scope::available(db.clone(), "products", product.id, b)
            .await
            .unwrap(),
        Some(30.0)
    );
    assert_eq!(
        catalog_scope::available(db.clone(), "products", product.id, a)
            .await
            .unwrap(),
        None
    );
    assert!(
        catalog_scope::get(db.clone(), "products", product.id, &scope, true)
            .await
            .is_err()
    );
    let result = TenantProductRepository::new(db.clone())
        .with_locations(vec![b])
        .list(&ProductFilterDto {
            organization_id: Some(org),
            allowed_location_ids: Some(vec![b]),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(result.total, 1);
    catalog_scope::save(
        db.clone(),
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
    let result = TenantProductRepository::new(db.clone())
        .with_locations(vec![b])
        .list(&ProductFilterDto {
            organization_id: Some(org),
            allowed_location_ids: Some(vec![b]),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(result.total, 0);
    catalog_scope::save(
        db.clone(),
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
        db.clone(),
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
        catalog_scope::available(db.clone(), "products", product.id, a)
            .await
            .unwrap(),
        Some(22.0)
    );
    assert_eq!(
        catalog_scope::available(db.clone(), "products", product.id, b)
            .await
            .unwrap(),
        Some(30.0)
    );
    assert!(catalog_scope::save(
        db.clone(),
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
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    let config = Arc::new(gaming_cafe_api::services::ConfigService::new(
        pool.clone(),
        Arc::new(NoopCache),
        "Asia/Kolkata".into(),
    ));
    let plans = gaming_cafe_api::services::PlanService::new(config);
    let plan = plans
        .create_tenant(
            db.clone(),
            vec![b],
            serde_json::from_value(json!({"name":"South pass","price":50,"planType":"time_based"}))
                .unwrap(),
            Some(user),
        )
        .await
        .unwrap();
    assert_eq!(plan.validity_days, 9);
    assert_eq!(plan.time_credits, 90);
    plans
        .update_tenant(
            db.clone(),
            plan.id,
            serde_json::from_value(json!({"name":"South pass updated"})).unwrap(),
            Some(user),
        )
        .await
        .unwrap();
    assert!(plans.active_tenant(db.clone(), a).await.unwrap().is_empty());
    assert_eq!(plans.active_tenant(db.clone(), b).await.unwrap().len(), 1);
    catalog_scope::save(
        db.clone(),
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
            .get_tenant(db.clone(), plan.id, Some(b))
            .await
            .unwrap()
            .current_price,
        Some(75.0)
    );
    let pricing = gaming_cafe_api::services::PricingPolicyService::new(pool.clone());
    let (set,version)=pricing.create_tenant(db.clone(),org,serde_json::from_value(json!({"name":"Both venues","locationIds":[a,b],"policy":{"baseRate":"60","rules":[],"roundingScale":0,"minimumPrice":"80","maximumPrice":"90"}})).unwrap(),user).await.unwrap();
    assert_eq!(set.location_ids.len(), 2);
    assert!(pricing
        .list_tenant(db.clone(), org, Some(b))
        .await
        .unwrap()
        .iter()
        .any(|s| s.id == set.id));
    pricing
        .validate_tenant(db.clone(), org, set.id, version.id)
        .await
        .unwrap();
    pricing
        .simulate_tenant(
            db.clone(),
            org,
            set.id,
            version.id,
            serde_json::from_value(json!({"locationId":a,"at":chrono::Utc::now()})).unwrap(),
            "UTC",
            "INR",
        )
        .await
        .unwrap();
    pricing
        .publish_tenant(
            db.clone(),
            org,
            set.id,
            version.id,
            serde_json::from_value(json!({})).unwrap(),
            user,
        )
        .await
        .unwrap();
    plans
        .update_tenant(
            db.clone(),
            plan.id,
            serde_json::from_value(json!({"price":88.6})).unwrap(),
            Some(user),
        )
        .await
        .unwrap();
    assert_eq!(
        plans
            .get_tenant(db.clone(), plan.id, Some(a))
            .await
            .unwrap()
            .current_price,
        Some(89.0)
    );
    assert_eq!(
        plans
            .get_tenant(db.clone(), plan.id, Some(b))
            .await
            .unwrap()
            .current_price,
        Some(80.0)
    );
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
    fixture.close().await;
}
