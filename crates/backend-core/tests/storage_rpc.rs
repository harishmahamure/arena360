//! Network boundary gates: service auth, tenant auth, real SQLite writes and provisioning.
mod support;
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use chrono::{Duration, Utc};
use gaming_cafe_api::{
    app::build_business_router,
    control::{CreateTenant, Tenant},
    error::AppError,
    storage_rpc::{router, StorageClient, StorageRpc},
    tenancy::{ProvisionTenant, ProvisioningControl, TenantProvisioner},
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use tenant_protocol::storage::{
    tenant_storage_service_client::TenantStorageServiceClient, ApiRequest,
};
use uuid::Uuid;

const SERVICE_TOKEN: &str = "arena360-private-service-token-thirty-two-characters";

#[tokio::test]
async fn private_rpc_merges_with_public_grpc_and_health_routes() {
    let fixture = support::SessionFixture::new().await;
    let state = fixture.app().await;
    let root = std::env::temp_dir().join(format!("arena360-rpc-merge-{}", Uuid::now_v7()));
    let cell = Uuid::now_v7();
    let provisioner = Arc::new(TenantProvisioner::new(
        root.clone(),
        Arc::new(Control {
            tenant: Mutex::new(None),
            cell,
        }),
    ));
    let private = router(
        StorageRpc::new(build_business_router(state.clone()), provisioner),
        SERVICE_TOKEN,
    )
    .unwrap();
    // Match production composition, including the public Tonic fallback.
    let app = gaming_cafe_api::app::build_router(state).merge(private);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let response = reqwest::get(format!("{address}/health/ready"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let client = StorageClient::new(SERVICE_TOKEN).unwrap();
    let response = client
        .invoke(
            &address,
            Request::builder()
                .uri("/health/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut unauthenticated = TenantStorageServiceClient::connect(address).await.unwrap();
    let error = unauthenticated
        .invoke(ApiRequest {
            method: "GET".into(),
            path: "/health/ready".into(),
            ..Default::default()
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), tonic::Code::Unauthenticated);
    server.abort();
    let _ = tokio::fs::remove_dir_all(root).await;
    fixture.close().await;
}

struct Control {
    tenant: Mutex<Option<Tenant>>,
    cell: Uuid,
}
#[async_trait]
impl ProvisioningControl for Control {
    async fn register(&self, input: CreateTenant) -> Result<Tenant, AppError> {
        if input.owner_cell != Some(self.cell) {
            return Err(AppError::BadRequest("Wrong owner".into()));
        }
        let tenant = Tenant {
            id: Uuid::now_v7(),
            slug: input.slug,
            name: input.name,
            owner_cell: input.owner_cell,
            ownership_generation: 0,
            schema_version: 0,
            state: "PROVISIONING".into(),
            timezone: input.timezone,
        };
        *self.tenant.lock().unwrap() = Some(tenant.clone());
        Ok(tenant)
    }
    async fn acquire_lease(&self, tenant: Uuid) -> Result<i64, AppError> {
        assert_eq!(self.tenant.lock().unwrap().as_ref().unwrap().id, tenant);
        Ok(1)
    }
    async fn finalize(
        &self,
        tenant: Uuid,
        generation: i64,
        schema: i64,
    ) -> Result<Tenant, AppError> {
        let mut current = self.tenant.lock().unwrap();
        let current = current.as_mut().unwrap();
        assert_eq!(current.id, tenant);
        current.ownership_generation = generation;
        current.schema_version = schema;
        current.state = "ACTIVE".into();
        Ok(current.clone())
    }
}
async fn start(
    api: axum::Router,
) -> (
    String,
    tokio::task::JoinHandle<()>,
    std::path::PathBuf,
    Uuid,
) {
    let root = std::env::temp_dir().join(format!("arena360-rpc-{}", Uuid::now_v7()));
    let cell = Uuid::now_v7();
    let provisioner = Arc::new(TenantProvisioner::new(
        root.clone(),
        Arc::new(Control {
            tenant: Mutex::new(None),
            cell,
        }),
    ));
    let app = router(StorageRpc::new(api, provisioner), SERVICE_TOKEN).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (address, server, root, cell)
}

#[tokio::test]
async fn grpc_rechecks_auth_writes_sqlite_and_rejects_foreign_tenants() {
    let fixture = support::SessionFixture::new().await;
    let state = fixture.app().await;
    let bearer = state
        .auth
        .generate_device_token_for(fixture.device, fixture.tenant.db.tenant_id(), fixture.venue)
        .unwrap();
    let player =
        gaming_cafe_api::repositories::TenantUserRepository::new(fixture.tenant.db.clone())
            .find_by_id(fixture.player)
            .await
            .unwrap()
            .unwrap();
    let player_token = state
        .auth
        .generate_player_token_for(
            &player,
            fixture.device,
            fixture.tenant.db.tenant_id(),
            fixture.venue,
        )
        .unwrap();
    let foreign = state
        .auth
        .generate_device_token_for(fixture.device, Uuid::now_v7(), fixture.venue)
        .unwrap();
    let (address, server, _, _) = start(build_business_router(state)).await;
    let client = StorageClient::new(SERVICE_TOKEN).unwrap();
    let request = |token: Option<&str>| {
        let mut request = Request::builder()
            .method("POST")
            .uri("/kiosk/sessions")
            .header("content-type", "application/json")
            .header("x-player-token", &player_token);
        if let Some(token) = token {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        request.body(Body::from("{}")).unwrap()
    };
    assert_eq!(
        client
            .invoke(&address, request(None))
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        client
            .invoke(&address, request(Some(&foreign)))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = client
        .invoke(&address, request(Some(&bearer)))
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["data"]["sessionId"].as_str().unwrap();
    let pool = fixture.tenant.db.read_pool().unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM usage_sessions WHERE id=?")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    server.abort();
    fixture.close().await;
}

#[tokio::test]
async fn private_rpc_requires_service_token_and_rejects_control_routes() {
    let (address, server, _, _) = start(axum::Router::new()).await;
    let mut raw = TenantStorageServiceClient::connect(address).await.unwrap();
    let input = ApiRequest {
        method: "GET".into(),
        path: "/platform/tenants".into(),
        ..Default::default()
    };
    assert_eq!(
        raw.invoke(input.clone()).await.unwrap_err().code(),
        tonic::Code::Unauthenticated
    );
    let mut request = tonic::Request::new(input);
    request
        .metadata_mut()
        .insert("x-arena-service-token", SERVICE_TOKEN.parse().unwrap());
    assert_eq!(
        raw.invoke(request).await.unwrap_err().code(),
        tonic::Code::InvalidArgument
    );
    server.abort();
}

#[tokio::test]
async fn remote_provisioning_migrates_and_seeds_only_on_storage() {
    let (address, server, root, cell) = start(axum::Router::new()).await;
    let client = StorageClient::new(SERVICE_TOKEN).unwrap();
    let input = ProvisionTenant {
        tenant: CreateTenant {
            slug: "grpc-tenant".into(),
            name: "gRPC tenant".into(),
            timezone: "UTC".into(),
            owner_cell: Some(cell),
            subscription_plan: "trial".into(),
            entitlements: json!({}),
            trial_ends_at: Utc::now() + Duration::days(7),
            entitlement_grace_until: Utc::now() + Duration::days(8),
        },
        settings: vec![],
    };
    let result = client.provision(&address, &input).await.unwrap();
    assert_eq!(result["state"], "ACTIVE");
    assert_eq!(
        result["schemaVersion"],
        gaming_cafe_api::tenancy::target_schema_version()
    );
    let tenant: Uuid = result["id"].as_str().unwrap().parse().unwrap();
    let path = gaming_cafe_api::tenancy::tenant_path(&root, tenant);
    assert!(path.exists());
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(path))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM units")
            .fetch_one(&pool)
            .await
            .unwrap(),
        11
    );
    pool.close().await;
    let mut wrong_owner = input;
    wrong_owner.tenant.owner_cell = Some(Uuid::now_v7());
    assert!(matches!(
        client.provision(&address, &wrong_owner).await,
        Err(AppError::Api {
            status: StatusCode::BAD_REQUEST,
            ..
        })
    ));
    server.abort();
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[tokio::test]
async fn unavailable_storage_returns_503() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let error = StorageClient::new(SERVICE_TOKEN)
        .unwrap()
        .invoke(
            &address,
            Request::builder()
                .uri("/users")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        AppError::Api {
            status: StatusCode::SERVICE_UNAVAILABLE,
            ..
        }
    ));
}

#[tokio::test]
#[ignore = "requires isolated CONTROL_TEST_DATABASE_URL"]
async fn stateless_gateway_routes_through_postgres_to_storage_grpc() {
    use gaming_cafe_api::{
        app::{build_router, build_state_with_settings},
        runtime::{configure_gateway, Service},
        storage_rpc::RemoteStorage,
    };
    let fixture = support::SessionFixture::new().await;
    let storage_state = fixture.app().await;
    let bearer = storage_state
        .auth
        .generate_device_token_for(fixture.device, fixture.tenant.db.tenant_id(), fixture.venue)
        .unwrap();
    let player =
        gaming_cafe_api::repositories::TenantUserRepository::new(fixture.tenant.db.clone())
            .find_by_id(fixture.player)
            .await
            .unwrap()
            .unwrap();
    let player_token = storage_state
        .auth
        .generate_player_token_for(
            &player,
            fixture.device,
            fixture.tenant.db.tenant_id(),
            fixture.venue,
        )
        .unwrap();
    let (address, storage, _, cell) = start(build_business_router(storage_state)).await;
    let mut settings = support::settings();
    settings.roles = Service::Gateway.roles();
    settings.control_database_url = Some(std::env::var("CONTROL_TEST_DATABASE_URL").unwrap());
    let gateway_path = settings.tenant_data_dir.clone();
    let mut gateway = build_state_with_settings(Arc::new(settings)).await;
    configure_gateway(
        Arc::get_mut(&mut gateway).unwrap(),
        RemoteStorage {
            client: StorageClient::new(SERVICE_TOKEN).unwrap(),
            address: address.clone(),
            cell_id: cell,
        },
    );
    assert!(gateway.tenant_dbs.is_none());
    assert!(gateway.leases.is_none());
    assert!(gateway.tenant_provisioner.is_none());
    let pool = gateway.control_db.as_ref().unwrap().clone();
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(cell)
        .bind(cell.to_string())
        .bind(address)
        .execute(&pool)
        .await
        .unwrap();
    let tenant = fixture.tenant.db.tenant_id();
    sqlx::query("INSERT INTO tenants(id,slug,name,timezone,state,owner_cell,ownership_generation) VALUES($1,$2,'gRPC routing','UTC','ACTIVE',$3,1)").bind(tenant).bind(tenant.to_string()).bind(cell).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO subscriptions(tenant_id,plan_code,status,starts_at,ends_at) VALUES($1,'trial','TRIAL',NOW(),NOW()+INTERVAL '7 days')").bind(tenant).execute(&pool).await.unwrap();
    let (status, body) = support::request(
        build_router(gateway),
        "POST",
        "/kiosk/sessions",
        Some(&bearer),
        Some(&player_token),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(
        !gateway_path.exists(),
        "Gateway must not create tenant files"
    );
    sqlx::query("DELETE FROM subscriptions WHERE tenant_id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM tenants WHERE id=$1")
        .bind(tenant)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM cells WHERE id=$1")
        .bind(cell)
        .execute(&pool)
        .await
        .unwrap();
    storage.abort();
    fixture.close().await;
}

#[tokio::test]
async fn grpc_preserves_binary_body_query_content_type_and_scoped_headers() {
    let api = axum::Router::new().route(
        "/echo",
        axum::routing::post(|request: Request<Body>| async move {
            assert_eq!(request.uri().query(), Some("page=2&search=a%20b"));
            assert_eq!(
                request.headers()["content-type"],
                "application/octet-stream"
            );
            assert_eq!(request.headers()["x-location-id"], "venue-42");
            assert_eq!(request.headers()["x-request-id"], "request-42");
            assert!(!request.headers().contains_key("x-arena-service-token"));
            let bytes = to_bytes(request.into_body(), 1024).await.unwrap();
            axum::http::Response::builder()
                .status(StatusCode::CONFLICT)
                .header("content-type", "application/octet-stream")
                .header("retry-after", "2")
                .body(Body::from(bytes))
                .unwrap()
        }),
    );
    let (address, server, _, _) = start(api).await;
    let response = StorageClient::new(SERVICE_TOKEN)
        .unwrap()
        .invoke(
            &address,
            Request::builder()
                .method("POST")
                .uri("/echo?page=2&search=a%20b")
                .header("content-type", "application/octet-stream")
                .header("x-location-id", "venue-42")
                .header("x-request-id", "request-42")
                .body(Body::from(vec![0, 255, 1, 128]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(response.headers()["retry-after"], "2");
    assert_eq!(
        response.headers()["content-type"],
        "application/octet-stream"
    );
    assert_eq!(
        &to_bytes(response.into_body(), 1024).await.unwrap()[..],
        &[0, 255, 1, 128]
    );
    server.abort();
}

#[tokio::test]
#[ignore = "requires isolated CONTROL_TEST_DATABASE_URL"]
async fn platform_api_provisions_via_grpc_with_postgres_leases_and_no_gateway_files() {
    use gaming_cafe_api::{
        app::{build_router, build_state_with_settings},
        control::{LeaseClient, LeaseConfig, Repository},
        runtime::{configure_gateway, Service},
        storage_rpc::RemoteStorage,
        tenancy::PostgresProvisioningControl,
    };
    use sha2::{Digest, Sha256};
    let mut settings = support::settings();
    settings.roles = Service::Gateway.roles();
    settings.control_database_url = Some(std::env::var("CONTROL_TEST_DATABASE_URL").unwrap());
    let gateway_path = settings.tenant_data_dir.clone();
    let mut gateway = build_state_with_settings(Arc::new(settings)).await;
    let pool = gateway.control_db.as_ref().unwrap().clone();
    let cell = Uuid::now_v7();
    let root = std::env::temp_dir().join(format!("arena360-platform-grpc-{cell}"));
    let leases = Arc::new(LeaseClient::new(pool.clone(), cell, LeaseConfig::default()).unwrap());
    let provisioner = Arc::new(TenantProvisioner::new(
        root.clone(),
        Arc::new(PostgresProvisioningControl::new(
            Repository::new(pool.clone()),
            leases,
        )),
    ));
    let storage_api = router(
        StorageRpc::new(axum::Router::new(), provisioner),
        SERVICE_TOKEN,
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let storage = tokio::spawn(async move {
        axum::serve(listener, storage_api).await.unwrap();
    });
    sqlx::query("INSERT INTO cells(id,name,address) VALUES($1,$2,$3)")
        .bind(cell)
        .bind(cell.to_string())
        .bind(&address)
        .execute(&pool)
        .await
        .unwrap();
    configure_gateway(
        Arc::get_mut(&mut gateway).unwrap(),
        RemoteStorage {
            client: StorageClient::new(SERVICE_TOKEN).unwrap(),
            address: address.clone(),
            cell_id: cell,
        },
    );
    let operator = Uuid::now_v7();
    sqlx::query("INSERT INTO platform_operators(id,username,password_hash) VALUES($1,$2,'login-disabled-fixture')").bind(operator).bind(operator.to_string()).execute(&pool).await.unwrap();
    let token = format!("p_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let hash = hex::encode(Sha256::digest(token.as_bytes()));
    sqlx::query("INSERT INTO platform_auth_tokens(token_hash,operator_id,purpose,expires_at) VALUES($1,$2,'SESSION',NOW()+INTERVAL '1 hour')").bind(hash).bind(operator).execute(&pool).await.unwrap();
    let (status, overview) = support::request(
        build_router(gateway.clone()),
        "GET",
        "/platform/overview",
        Some(&token),
        None,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["localCellId"], cell.to_string());
    assert_eq!(overview["canProvision"], true);
    assert_eq!(overview["provisioningMode"], "remote");
    assert_eq!(overview["provisioningAddress"], address);
    assert_eq!(
        overview["targetSchemaVersion"],
        gaming_cafe_api::tenancy::target_schema_version()
    );
    let slug = format!("grpc-{cell}");
    let (status, body) = support::request(
        build_router(gateway),
        "POST",
        "/platform/tenants",
        Some(&token),
        None,
        json!({"name":"Remote platform tenant","slug":slug,"timezone":"UTC","trialDays":7}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let tenant = Repository::new(pool.clone())
        .tenant_by_slug(&slug)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tenant.state, "ACTIVE");
    assert_eq!(tenant.owner_cell, Some(cell));
    assert_eq!(
        tenant.schema_version,
        gaming_cafe_api::tenancy::target_schema_version()
    );
    assert!(gaming_cafe_api::tenancy::tenant_path(&root, tenant.id).exists());
    assert!(!gateway_path.exists());
    storage.abort();
    tokio::fs::remove_dir_all(root).await.unwrap();
}

#[cfg(not(feature = "duckdb-analytics"))]
#[tokio::test]
async fn sqlite_reports_are_available_over_grpc_without_analytics_dependencies() {
    let fixture = support::SessionFixture::new().await;
    let permissions = vec![
        "stats:read".to_owned(),
        "finance:read".to_owned(),
        "inventory:read".to_owned(),
    ];
    let user = fixture
        .tenant
        .staff(Some(fixture.venue), permissions.clone())
        .await;
    let state = fixture.app().await;
    let tenant = fixture.tenant.db.tenant_id();
    let claims:gaming_cafe_api::dto::JwtUserClaims=serde_json::from_value(json!({"sub":user,"userId":user,"tenantId":tenant,"roles":["staff"],"permissions":permissions,"allowedTenants":[tenant],"iss":"gamezone","aud":"gamezone","appId":"admin","orgIds":[tenant],"iat":Utc::now().timestamp(),"exp":Utc::now().timestamp()+3600})).unwrap();
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.settings.jwt_secret.as_bytes()),
    )
    .unwrap();
    let (address, server, root, _) = start(build_business_router(state)).await;
    let client = StorageClient::new(SERVICE_TOKEN).unwrap();
    for path in [
        "/stats/dashboard",
        "/stats/business",
        "/stats/finance/report",
        "/inventory/overview",
    ] {
        let request = Request::builder()
            .uri(format!("{path}?startDate=2026-10-01&endDate=2026-10-02"))
            .header("authorization", format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap();
        let response = client.invoke(&address, request).await.unwrap();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(
            status,
            StatusCode::OK,
            "{path}: {}",
            String::from_utf8_lossy(&bytes)
        );
    }
    server.abort();
    let _ = tokio::fs::remove_dir_all(root).await;
    fixture.close().await;
}
