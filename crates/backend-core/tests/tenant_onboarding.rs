//! Durable setup, permission boundaries, atomic saves, and the first operational session.
mod support;

use axum::{http::StatusCode, Router};
use gaming_cafe_api::{
    app::{build_business_router, build_state_with_settings},
    dto::JwtUserClaims,
    error::AppError,
    models::onboarding::{CompleteOnboardingDto, OnboardingStep, SaveOnboardingDto},
    repositories::TenantOnboardingRepository,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use std::sync::Arc;
use support::{request, TenantFixture};
use uuid::Uuid;

async fn admin(f: &TenantFixture) -> Uuid {
    let grants = gaming_cafe_api::access::catalog()
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|module| {
            module["permissions"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| p["key"].as_str().unwrap().to_string())
        })
        .collect();
    let actor = f.staff(None, grants).await;
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET role='admin' WHERE id=?")
                .bind(actor.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    actor
}

fn draft(revision: i64, current_step: OnboardingStep) -> SaveOnboardingDto {
    SaveOnboardingDto {
        expected_revision: revision,
        current_step,
        review_settings: false,
    }
}

#[tokio::test]
async fn setup_is_available_over_public_configuration_grpc_with_rest_disabled() {
    use prost::Message;
    use tenant_protocol::arena360::v1::{ProxyRequest, ProxyResponse};
    let f = TenantFixture::new().await;
    let actor = admin(&f).await;
    let mut settings = support::settings();
    settings.legacy_rest_enabled = false;
    let mut state = build_state_with_settings(Arc::new(settings)).await;
    Arc::get_mut(&mut state).unwrap().tenant_dbs = Some(f.manager.clone());
    let router = gaming_cafe_api::app::build_router(state);
    assert_eq!(
        request(
            router.clone(),
            "GET",
            "/onboarding",
            Some(&token(&f, actor, "admin")),
            None,
            json!({})
        )
        .await
        .0,
        // The tonic-web fallback rejects non-gRPC requests with 400 when REST is disabled.
        StatusCode::BAD_REQUEST
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let client = reqwest::Client::new();
    for (method, body, expected_revision) in [
        ("GET", json!({}), 0),
        (
            "PUT",
            json!({"currentStep":"settings","expectedRevision":0}),
            1,
        ),
    ] {
        let message = ProxyRequest {
            method: method.into(),
            path: "/onboarding".into(),
            body_json: serde_json::to_vec(&body).unwrap(),
            headers: std::collections::HashMap::from([(
                "authorization".into(),
                format!("Bearer {}", token(&f, actor, "admin")),
            )]),
            ..Default::default()
        }
        .encode_to_vec();
        let mut frame = vec![0];
        frame.extend_from_slice(&(message.len() as u32).to_be_bytes());
        frame.extend_from_slice(&message);
        let wire = client
            .post(format!("{address}/arena360.v1.ConfigurationService/Invoke"))
            .header("content-type", "application/grpc-web+proto")
            .header("x-grpc-web", "1")
            .body(frame)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert!(wire.len() >= 5, "{wire:?}");
        assert_eq!(wire[0], 0, "{wire:?}");
        let length = u32::from_be_bytes(wire[1..5].try_into().unwrap()) as usize;
        let response = ProxyResponse::decode(&wire[5..5 + length]).unwrap();
        assert_eq!(
            response.status_code,
            200,
            "{}",
            String::from_utf8_lossy(&response.body_json)
        );
        let data: Value = serde_json::from_slice(&response.body_json).unwrap();
        assert_eq!(data["data"]["progress"]["revision"], expected_revision);
    }
    server.abort();
    f.close().await;
}

async fn app(f: &TenantFixture) -> Router {
    let mut state = build_state_with_settings(Arc::new(support::settings())).await;
    Arc::get_mut(&mut state).unwrap().tenant_dbs = Some(f.manager.clone());
    build_business_router(state)
}

fn token(f: &TenantFixture, actor: Uuid, role: &str) -> String {
    let claims: JwtUserClaims = serde_json::from_value(json!({
        "sub":actor,"userId":actor,"tenantId":f.db.tenant_id(),"roles":[role],
        "permissions":[],"allowedTenants":[f.db.tenant_id()],"orgIds":[f.db.tenant_id()],
        "appId":"admin","iss":"gamezone","aud":"gamezone","exp":chrono::Utc::now().timestamp()+300
    }))
    .unwrap();
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(support::settings().jwt_secret.as_bytes()),
    )
    .unwrap()
}

async fn success(app: &Router, token: &str, method: &str, path: &str, input: Value) -> Value {
    let (status, body) = request(app.clone(), method, path, Some(token), None, input).await;
    assert!(status.is_success(), "{method} {path}: {status} {body}");
    body["data"].clone()
}

#[tokio::test]
async fn fresh_tenant_http_path_finishes_setup_opens_shift_sells_plan_and_starts_session() {
    let f = TenantFixture::new().await;
    let actor = admin(&f).await;
    let api = app(&f).await;
    let auth = token(&f, actor, "admin");
    let fresh = success(&api, &auth, "GET", "/onboarding", json!({})).await;
    assert!(fresh["progress"]["startedAt"].is_null());
    assert_eq!(fresh["progress"]["revision"], 0);
    assert_eq!(fresh["readiness"]["ready"], false);
    assert_eq!(
        request(
            api.clone(),
            "POST",
            "/onboarding/complete",
            Some(&auth),
            None,
            json!({"expectedRevision":0})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );

    let venue = success(
        &api,
        &auth,
        "POST",
        &format!("/organizations/{}/locations", f.db.tenant_id()),
        json!({"slug":"main","name":"Main venue","timezone":"UTC","currency":"INR"}),
    )
    .await;
    success(
        &api,
        &auth,
        "PUT",
        "/onboarding",
        json!({"expectedRevision":0,"currentStep":"settings"}),
    )
    .await;
    success(
        &api,
        &auth,
        "PUT",
        "/onboarding",
        json!({"expectedRevision":1,"currentStep":"stations","reviewSettings":true}),
    )
    .await;
    let station = success(&api, &auth, "POST", "/devices", json!({"name":"PC-01","locationId":venue["id"],"deviceType":"PC","deviceSubType":"HIGH_END_PCS"})).await;
    let plan = success(&api, &auth, "POST", "/plans", json!({"name":"One hour","price":100,"planType":"time_based","timeCredits":60,"deviceType":"PC","deviceSubType":"HIGH_END_PCS"})).await;
    let ready = success(&api, &auth, "GET", "/onboarding", json!({})).await;
    assert_eq!(ready["readiness"]["ready"], true, "{ready}");
    success(
        &api,
        &auth,
        "PUT",
        "/onboarding",
        json!({"expectedRevision":2,"currentStep":"review"}),
    )
    .await;
    let complete = success(
        &api,
        &auth,
        "POST",
        "/onboarding/complete",
        json!({"expectedRevision":3}),
    )
    .await;
    assert!(complete["progress"]["completedAt"].is_string());
    assert_eq!(complete["progress"]["revision"], 4);
    let retried = success(
        &api,
        &auth,
        "POST",
        "/onboarding/complete",
        json!({"expectedRevision":4}),
    )
    .await;
    assert_eq!(complete, retried);

    let shift = success(
        &api,
        &auth,
        "POST",
        "/shifts/start",
        json!({"openingBalance":0,"venueLocationId":venue["id"]}),
    )
    .await;
    assert!(shift["shift"]["id"].is_string(), "{shift}");
    success(
        &api,
        &auth,
        "POST",
        "/auth/register",
        json!({"username":"first-player","password":"initial-password","phoneNumber":"9999999999"}),
    )
    .await;
    let player: String = sqlx::query_scalar("SELECT id FROM users WHERE username='first-player'")
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap();
    let sale = success(&api, &auth, "POST", "/transactions", json!({"playerId":player,"planId":plan["id"],"venueLocationId":venue["id"],"transactionType":"plan_purchase","paymentMethod":"cash","paymentStatus":"completed"})).await;
    let balance = success(
        &api,
        &auth,
        "POST",
        "/player-plans",
        json!({"playerId":player,"planId":plan["id"],"transactionId":sale["id"]}),
    )
    .await;
    let session = success(
        &api,
        &auth,
        "POST",
        "/sessions",
        json!({"balanceId":balance["id"],"deviceId":station["id"]}),
    )
    .await;
    assert_eq!(session["shiftId"], shift["shift"]["id"]);
    assert!(session["endTime"].is_null());
    let refresh = success(&api, &auth, "GET", "/onboarding", json!({})).await;
    assert_eq!(refresh["progress"], complete["progress"]);
    f.close().await;
}

#[tokio::test]
async fn progress_survives_handle_close_and_concurrent_saves_do_not_lose_updates() {
    let f = TenantFixture::new().await;
    let actor = admin(&f).await;
    let repo = TenantOnboardingRepository::new(f.db.clone());
    let (a, b) = tokio::join!(
        repo.save(actor, draft(0, OnboardingStep::Settings)),
        repo.save(actor, draft(0, OnboardingStep::Stations))
    );
    assert_eq!(a.is_ok() as u8 + b.is_ok() as u8, 1);
    assert!(
        matches!(a.err().or_else(||b.err()).unwrap(), AppError::Api { code, .. } if code=="ONBOARDING_REVISION_CONFLICT")
    );
    let saved = repo.snapshot(actor).await.unwrap();
    assert_eq!(saved.progress.revision, 1);
    assert!(saved.progress.started_at.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM outbox_events WHERE aggregate_type='onboarding'"
        )
        .fetch_one(&f.db.read_pool().unwrap())
        .await
        .unwrap(),
        1
    );
    f.db.close().await.unwrap();
    let reopened = f.manager.open(f.db.tenant_id()).await.unwrap();
    let resumed = TenantOnboardingRepository::new(reopened.clone())
        .snapshot(actor)
        .await
        .unwrap();
    assert_eq!(resumed.progress.current_step, saved.progress.current_step);
    assert_eq!(resumed.progress.revision, 1);
    assert_eq!(resumed.progress.started_at, saved.progress.started_at);
    reopened.close().await.unwrap();
    f.close().await;
}

#[tokio::test]
async fn failed_event_write_rolls_back_progress_and_can_be_retried() {
    let f = TenantFixture::new().await;
    let actor = admin(&f).await;
    let repo = TenantOnboardingRepository::new(f.db.clone());
    f.db.with_immediate_writer(|c|Box::pin(async move {
        sqlx::query("CREATE TRIGGER reject_setup_event BEFORE INSERT ON outbox_events WHEN NEW.aggregate_type='onboarding' BEGIN SELECT RAISE(ABORT,'injected save failure'); END").execute(c).await?;
        Ok(())
    })).await.unwrap();
    assert!(repo
        .save(actor, draft(0, OnboardingStep::Settings))
        .await
        .is_err());
    let after = repo.snapshot(actor).await.unwrap();
    assert_eq!(after.progress.revision, 0);
    assert!(after.progress.started_at.is_none());
    f.db.with_immediate_writer(|c| {
        Box::pin(async move {
            sqlx::query("DROP TRIGGER reject_setup_event")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert_eq!(
        repo.save(actor, draft(0, OnboardingStep::Settings))
            .await
            .unwrap()
            .progress
            .revision,
        1
    );
    f.close().await;
}

#[tokio::test]
async fn setup_rejects_staff_missing_grants_revoked_members_and_foreign_tenants() {
    let f = TenantFixture::new().await;
    let other = TenantFixture::new().await;
    let actor = admin(&f).await;
    let repo = TenantOnboardingRepository::new(f.db.clone());
    let staff = f
        .staff(None, vec!["settings:read".into(), "settings:write".into()])
        .await;
    assert!(matches!(
        repo.snapshot(staff).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(matches!(
        repo.save(staff, draft(0, OnboardingStep::Review)).await,
        Err(AppError::Forbidden(_))
    ));
    assert!(TenantOnboardingRepository::new(other.db.clone())
        .snapshot(actor)
        .await
        .is_err());
    assert!(TenantOnboardingRepository::new(other.db.clone())
        .save(actor, draft(0, OnboardingStep::Review))
        .await
        .is_err());
    let api = app(&f).await;
    assert_eq!(
        request(
            api.clone(),
            "GET",
            "/onboarding",
            Some(&token(&f, staff, "staff")),
            None,
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let foreign_auth = token(&other, actor, "admin");
    assert_eq!(
        request(
            api.clone(),
            "GET",
            "/onboarding",
            Some(&foreign_auth),
            None,
            json!({})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("DELETE FROM access_assignments WHERE user_id=?")
                .bind(actor.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(repo.snapshot(actor).await.is_err());
    assert!(repo
        .save(actor, draft(0, OnboardingStep::Review))
        .await
        .is_err());
    let actor2 = admin(&f).await;
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE users SET is_active=0 WHERE id=?")
                .bind(actor2.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(repo.snapshot(actor2).await.is_err());
    assert_eq!(
        TenantOnboardingRepository::new(other.db.clone())
            .snapshot(admin(&other).await)
            .await
            .unwrap()
            .progress
            .revision,
        0
    );
    f.close().await;
    other.close().await;
}

#[tokio::test]
async fn completion_rechecks_saved_data_and_requires_matching_scopes_and_operator_permissions() {
    let f = TenantFixture::new().await;
    let actor = admin(&f).await;
    let venue = f.venue("main").await;
    let other = f.venue("secondary").await;
    let repo = TenantOnboardingRepository::new(f.db.clone());
    repo.save(
        actor,
        SaveOnboardingDto {
            review_settings: true,
            ..draft(0, OnboardingStep::Review)
        },
    )
    .await
    .unwrap();
    let at = gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();
    let plan = Uuid::now_v7();
    f.db.with_immediate_writer(move|c|Box::pin(async move {
        sqlx::query("INSERT INTO devices(id,location_id,name,device_type,device_sub_type,created_at,updated_at) VALUES(?,?,'PC','PC','HIGH_END_PCS',?,?)").bind(Uuid::now_v7().to_string()).bind(venue.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO plans(id,name,price,plan_type,time_credits,device_type,device_sub_type,availability_scope,created_at,updated_at) VALUES(?,'Hour',10000,'time_based',60,'PC','MID_RANGE_PCS','SELECTED',?,?)").bind(plan.to_string()).bind(&at).bind(&at).execute(&mut *c).await?;
        sqlx::query("INSERT INTO plan_locations(plan_id,location_id) VALUES(?,?)").bind(plan.to_string()).bind(other.to_string()).execute(c).await?;
        Ok(())
    })).await.unwrap();
    assert!(!repo.snapshot(actor).await.unwrap().readiness.ready);
    f.db.with_immediate_writer(move|c|Box::pin(async move {
        sqlx::query("UPDATE plans SET device_sub_type='HIGH_END_PCS',availability_scope='ALL' WHERE id=?").bind(plan.to_string()).execute(c).await?;Ok(())
    })).await.unwrap();
    assert!(repo.snapshot(actor).await.unwrap().readiness.ready);
    // A plan was retired after the browser's readiness check, without touching progress revision.
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE plans SET is_active=0 WHERE id=?")
                .bind(plan.to_string())
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(
        matches!(repo.complete(actor,CompleteOnboardingDto {expected_revision:1}).await,Err(AppError::Api {code,..}) if code=="ONBOARDING_NOT_READY")
    );
    assert!(repo
        .snapshot(actor)
        .await
        .unwrap()
        .progress
        .completed_at
        .is_none());
    f.db.with_immediate_writer(move |c| {
        Box::pin(async move {
            sqlx::query("UPDATE plans SET is_active=1 WHERE id=?")
                .bind(plan.to_string())
                .execute(&mut *c)
                .await?;
            sqlx::query("INSERT INTO access_modules(module,enabled) VALUES('transactions',0)")
                .execute(c)
                .await?;
            Ok(())
        })
    })
    .await
    .unwrap();
    assert!(!repo.snapshot(actor).await.unwrap().readiness.ready);
    f.close().await;
}

#[tokio::test]
async fn migration_preserves_operating_tenants_and_leaves_fresh_tenants_unstarted() {
    let f = TenantFixture::new().await;
    let actor = admin(&f).await;
    let venue = f.venue("main").await;
    let repo = TenantOnboardingRepository::new(f.db.clone());
    assert!(repo
        .snapshot(actor)
        .await
        .unwrap()
        .progress
        .completed_at
        .is_none());
    f.db.close().await.unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(f.db.path()))
        .await
        .unwrap();
    let at = gaming_cafe_api::time::format_sqlite_timestamp(&chrono::Utc::now()).unwrap();
    sqlx::query("INSERT INTO shifts(id,user_id,location_id,clock_in,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(Uuid::now_v7().to_string()).bind(actor.to_string()).bind(venue.to_string()).bind(&at).bind(&at).bind(&at).execute(&pool).await.unwrap();
    sqlx::query("DROP TABLE tenant_onboarding")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/tenant/0019_onboarding.sql"))
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let reopened = f.manager.open(f.db.tenant_id()).await.unwrap();
    let progress = TenantOnboardingRepository::new(reopened.clone())
        .snapshot(actor)
        .await
        .unwrap()
        .progress;
    assert!(progress.completed_at.is_some());
    assert_eq!(progress.completed_at.as_ref().unwrap().len(), 27);
    assert_eq!(progress.current_step, OnboardingStep::Review);
    reopened.close().await.unwrap();
    f.close().await;
}
