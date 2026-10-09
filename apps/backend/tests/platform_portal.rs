//! Operator API smoke gate using real control PostgreSQL and provisioned SQLite.
use gaming_cafe_api::{
    control::{LeaseClient, LeaseConfig, Repository},
    handlers::platform::{router_with_context, Context},
    tenancy::{PostgresProvisioningControl, TenantProvisioner},
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires an isolated control-plane database"]
async fn operator_api_provisions_and_manages_tenants_without_tenant_auth_or_routing() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&std::env::var("CONTROL_TEST_DATABASE_URL").unwrap())
        .await
        .unwrap();
    gaming_cafe_api::control::migrate(&pool).await.unwrap();
    let cell = Uuid::new_v4();
    let root = std::env::temp_dir().join(format!("arena-portal-{cell}"));
    let leases = Arc::new(LeaseClient::new(pool.clone(), cell, LeaseConfig::default()).unwrap());
    let provisioner = Arc::new(TenantProvisioner::new(
        root.clone(),
        Arc::new(PostgresProvisioningControl::new(
            Repository::new(pool.clone()),
            leases,
        )),
    ));
    let app = router_with_context(Context {
        pool: Some(pool.clone()),
        provisioner: Some(provisioner),
        cell: Some(cell),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/platform", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let client = reqwest::Client::new();
    // Every route, including writes, is protected before dispatch or database work.
    for path in [
        "/overview",
        "/cells",
        "/tenants",
        "/tenants/00000000-0000-0000-0000-000000000000",
    ] {
        assert_eq!(
            client
                .get(format!("{base}{path}"))
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
    }
    assert_eq!(
        client
            .post(format!("{base}/cells"))
            .bearer_auth("tenant-jwt")
            .json(&json!({"name":"x","address":"http://localhost:1"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let operator = format!("operator-{cell}");
    let password = "portal-test-password-123";
    let hash = bcrypt::hash(password,bcrypt::DEFAULT_COST).unwrap();
    sqlx::query("INSERT INTO platform_operators(username,password_hash) VALUES($1,$2)")
        .bind(&operator).bind(hash).execute(&pool).await.unwrap();
    let login:Value=client.post(format!("{base}/auth/login")).json(&json!({"username":operator,"password":password}))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(login["setupRequired"],true);
    let challenge=login["challenge"].as_str().unwrap();
    assert_eq!(client.get(format!("{base}/overview")).bearer_auth(challenge).send().await.unwrap().status(),401);
    let setup_response=client.post(format!("{base}/auth/totp/setup")).bearer_auth(challenge)
        .send().await.unwrap();
    let setup_status=setup_response.status();
    let setup_body=setup_response.text().await.unwrap();
    assert_eq!(setup_status,200,"{setup_body}");
    let setup:Value=serde_json::from_str(&setup_body).unwrap();
    let secret=setup["secret"].as_str().unwrap();
    let totp=totp_rs::TOTP::new(totp_rs::Algorithm::SHA1,6,1,30,
        totp_rs::Secret::Encoded(secret.to_owned()).to_bytes().unwrap(),
        Some("GameZone".into()),format!("platform-{operator}")).unwrap();
    let code=totp.generate_current().unwrap();
    let session:Value=client.post(format!("{base}/auth/totp/verify")).bearer_auth(challenge)
        .json(&json!({"code":code})).send().await.unwrap().json().await.unwrap();
    let token=session["token"].as_str().unwrap();
    let response=client.post(format!("{base}/cells")).bearer_auth(token).json(&json!({"id":cell,"name":format!("portal-{cell}"),"address":format!("http://{cell}.invalid")})).send().await.unwrap();
    assert_eq!(response.status(), 201);
    let slug = format!("portal-{cell}");
    let input = json!({"name":"Portal test","slug":slug,"timezone":"Asia/Kolkata","trialDays":30});
    let response = client
        .post(format!("{base}/tenants"))
        .bearer_auth(token)
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201, "{}", response.text().await.unwrap());
    let tenant = Repository::new(pool.clone())
        .tenant_by_slug(&slug)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tenant.state, "ACTIVE");
    assert!(gaming_cafe_api::tenancy::tenant_path(&root, tenant.id).exists());
    // Same slug is resumable; different inputs cannot silently rename it.
    assert_eq!(
        client
            .post(format!("{base}/tenants"))
            .bearer_auth(token)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        201
    );
    let id = tenant.id;
    let expiry = chrono::Utc::now()+chrono::Duration::days(45);
    assert_eq!(client.post(format!("{base}/plans")).bearer_auth(token)
        .json(&json!({"code":"pro","name":"Pro","entitlements":{"maxDevices":24},"graceDays":7,"isActive":true}))
        .send().await.unwrap().status(),201);
    assert_eq!(client.put(format!("{base}/tenants/{id}/subscription")).bearer_auth(token)
        .json(&json!({"planCode":"pro","endsAt":expiry}))
        .send().await.unwrap().status(),200);
    assert_eq!(client.put(format!("{base}/tenants/{id}/enabled")).bearer_auth(token)
        .json(&json!({"enabled":false})).send().await.unwrap().status(),200);
    let enabled:bool=sqlx::query_scalar("SELECT is_enabled FROM tenants WHERE id=$1").bind(id).fetch_one(&pool).await.unwrap();
    assert!(!enabled);
    assert_eq!(client.put(format!("{base}/tenants/{id}/enabled")).bearer_auth(token)
        .json(&json!({"enabled":true})).send().await.unwrap().status(),200);
    let response = client
        .get(format!("{base}/tenants?search={id}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    let list: Value = response.json().await.unwrap();
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert_eq!(list["items"][0]["lease_fresh"], true);
    assert_eq!(
        client
            .get(format!("{base}/tenants?limit=999"))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let username = format!("portal-admin-{cell}");
    let password = "portal-test-password-123";
    let response = client
        .post(format!("{base}/tenants/{id}/admins"))
        .bearer_auth(token)
        .json(&json!({"username":username,"password":password}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let account: Value = response.json().await.unwrap();
    assert!(account.get("password").is_none());
    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE username=$1")
        .bind(&username)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(bcrypt::verify(password, &hash).unwrap());
    assert_eq!(
        client
            .post(format!("{base}/tenants/{id}/admins"))
            .bearer_auth(token)
            .json(&json!({"username":username,"password":password}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let response = client
        .put(format!("{base}/tenants/{id}/timezone"))
        .bearer_auth(token)
        .json(&json!({"timezone":"UTC"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        client
            .put(format!("{base}/tenants/{id}/timezone"))
            .bearer_auth(token)
            .json(&json!({"timezone":"invalid"}))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let response = client
        .get(format!("{base}/tenants/{id}"))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let detail: Value = response.json().await.unwrap();
    assert_eq!(detail["admins"][0]["username"], username);
    assert_eq!(detail["tenant"]["timezone"], "UTC");
    assert!(!detail.to_string().contains("password_hash"));
    let target = Uuid::new_v4();
    client.post(format!("{base}/cells")).bearer_auth(token).json(&json!({"id":target,"name":format!("target-{target}"),"address":format!("http://{target}.invalid")})).send().await.unwrap().error_for_status().unwrap();
    let response = client
        .post(format!("{base}/tenants/{id}/move"))
        .bearer_auth(token)
        .json(&json!({"targetCell":target}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 202);
    let job: Value = response.json().await.unwrap();
    let job_id: Uuid = job["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        client
            .post(format!("{base}/tenants/{id}/cold"))
            .bearer_auth(token)
            .json(&json!({"minimumIdleSeconds":0}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let wrong = Uuid::new_v4();
    assert_eq!(
        client
            .post(format!("{base}/tenants/{wrong}/jobs/{job_id}/cancel"))
            .bearer_auth(token)
            .json(&json!({"kind":"Move"}))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        client
            .post(format!("{base}/tenants/{id}/jobs/{job_id}/cancel"))
            .bearer_auth(token)
            .json(&json!({"kind":"Move"}))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    let response = client
        .post(format!("{base}/tenants/{id}/cold"))
        .bearer_auth(token)
        .json(&json!({"minimumIdleSeconds":0}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 202);
    let job: Value = response.json().await.unwrap();
    gaming_cafe_api::cold::cancel(&pool, job["id"].as_str().unwrap().parse().unwrap())
        .await
        .unwrap();
    assert_eq!(
        client
            .post(format!("{base}/tenants/{id}/wake"))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(client.post(format!("{base}/auth/logout")).bearer_auth(token).send().await.unwrap().status(),204);
    assert_eq!(client.get(format!("{base}/overview")).bearer_auth(token).send().await.unwrap().status(),401);
    server.abort();
    let _ = server.await;
    pool.close().await;
    std::fs::remove_dir_all(root).unwrap();
}
