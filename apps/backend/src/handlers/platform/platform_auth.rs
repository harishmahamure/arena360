use super::{pool, Context};
use crate::{error::AppError, services::totp_util::{generate_totp_setup, verify_totp_code}};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
    routing::post,
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(FromRow)]
struct Operator {
    id: Uuid,
    password_hash: String,
    totp_enabled: bool,
}

pub(super) fn router() -> Router<Context> {
    Router::new()
        .route("/platform/auth/login", post(login))
        .route("/platform/auth/totp/setup", post(setup))
        .route("/platform/auth/totp/verify", post(verify))
        .route("/platform/auth/logout", post(logout))
}

fn bearer(headers: &HeaderMap) -> Result<&str, AppError> {
    headers.get("authorization").and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|v| v.len() == 66 && v.starts_with("p_") && v[2..].bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| AppError::Unauthorized("Platform sign-in required".into()))
}

fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

fn new_token() -> String {
    // Two independent OS-generated UUIDv4 values give 244 random bits.
    format!("p_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

async fn issue(c: &Context, operator: Uuid, purpose: &str) -> Result<String, AppError> {
    let token = new_token();
    let expiry = if purpose == "SESSION" { Duration::hours(12) } else { Duration::minutes(10) };
    sqlx::query("INSERT INTO platform_auth_tokens(token_hash,operator_id,purpose,expires_at) VALUES($1,$2,$3,$4)")
        .bind(token_hash(&token)).bind(operator).bind(purpose).bind(Utc::now()+expiry)
        .execute(pool(c)?).await?;
    Ok(token)
}

async fn token_operator(c: &Context, token: &str, purpose: &str) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT a.operator_id FROM platform_auth_tokens a JOIN platform_operators o ON o.id=a.operator_id WHERE a.token_hash=$1 AND a.purpose=$2 AND a.expires_at>clock_timestamp() AND a.attempts<5 AND o.is_active AND (o.locked_until IS NULL OR o.locked_until<clock_timestamp())")
        .bind(token_hash(token)).bind(purpose).fetch_optional(pool(c)?).await?
        .ok_or_else(|| AppError::Unauthorized("Platform session expired or invalid".into()))
}

pub(super) async fn authenticate(
    State(c): State<Context>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, AppError> {
    token_operator(&c, bearer(request.headers())?, "SESSION").await?;
    Ok(next.run(request).await)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credentials { username: String, password: String }

async fn login(State(c): State<Context>, Json(input): Json<Credentials>) -> Result<Json<Value>, AppError> {
    if input.username.len()>100 || input.username.trim().is_empty() || input.password.len()>1024 {
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }
    let operator: Option<Operator> = sqlx::query_as("SELECT id,password_hash,totp_enabled FROM platform_operators WHERE lower(username)=lower($1) AND is_active AND (locked_until IS NULL OR locked_until<clock_timestamp())")
        .bind(&input.username).fetch_optional(pool(&c)?).await?;
    let password = input.password;
    let hash = operator.as_ref().map(|o| o.password_hash.clone())
        .unwrap_or_else(|| "$2b$12$C6UzMDM.H6dfI/f/IKcEeOejvMquCz0n.5P1OfRDw1TlXTjxSe8gK".into());
    let valid = tokio::task::spawn_blocking(move || bcrypt::verify(password, &hash))
        .await.map_err(|e| AppError::Internal(e.to_string()))?
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if !valid || operator.is_none() {
        if let Some(o) = operator {
            sqlx::query("UPDATE platform_operators SET failed_logins=failed_logins+1,locked_until=CASE WHEN failed_logins+1>=5 THEN clock_timestamp()+INTERVAL '15 minutes' ELSE NULL END,updated_at=clock_timestamp() WHERE id=$1")
                .bind(o.id).execute(pool(&c)?).await?;
        }
        return Err(AppError::Unauthorized("Invalid credentials".into()));
    }
    let operator = operator.unwrap();
    let challenge = issue(&c, operator.id, "CHALLENGE").await?;
    Ok(Json(json!({"challenge":challenge,"setupRequired":!operator.totp_enabled})))
}

async fn setup(State(c): State<Context>, headers: HeaderMap) -> Result<Json<Value>, AppError> {
    let id = token_operator(&c, bearer(&headers)?, "CHALLENGE").await?;
    let row: (String, bool) = sqlx::query_as("SELECT username,totp_enabled FROM platform_operators WHERE id=$1")
        .bind(id).fetch_one(pool(&c)?).await?;
    if row.1 { return Err(AppError::Forbidden("TOTP is already enabled".into())); }
    let (secret, uri) = generate_totp_setup(&format!("platform-{}", row.0))?;
    sqlx::query("UPDATE platform_operators SET totp_pending_secret=$2,updated_at=clock_timestamp() WHERE id=$1 AND NOT totp_enabled")
        .bind(id).bind(&secret).execute(pool(&c)?).await?;
    Ok(Json(json!({"secret":secret,"otpauthUri":uri})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Code { code: String }
async fn verify(State(c): State<Context>, headers: HeaderMap, Json(input): Json<Code>) -> Result<Json<Value>, AppError> {
    let token = bearer(&headers)?;
    if input.code.len()!=6 || !input.code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AppError::Unauthorized("Invalid authentication code".into()));
    }
    let mut tx = pool(&c)?.begin().await?;
    let row: Option<(Uuid,String,Option<String>,Option<String>,bool,i32)> = sqlx::query_as("SELECT o.id,o.username,o.totp_secret,o.totp_pending_secret,o.totp_enabled,a.attempts FROM platform_auth_tokens a JOIN platform_operators o ON o.id=a.operator_id WHERE a.token_hash=$1 AND a.purpose='CHALLENGE' AND a.expires_at>clock_timestamp() AND o.is_active AND (o.locked_until IS NULL OR o.locked_until<clock_timestamp()) FOR UPDATE OF a,o")
        .bind(token_hash(token)).fetch_optional(&mut *tx).await?;
    let (id, username, secret, pending, enabled, attempts) = row.ok_or_else(||AppError::Unauthorized("Platform challenge expired".into()))?;
    if attempts>=5 { return Err(AppError::Unauthorized("Too many authentication attempts".into())); }
    let secret = if enabled { secret } else { pending }.ok_or_else(||AppError::BadRequest("Start TOTP setup first".into()))?;
    let valid = verify_totp_code(&secret, &input.code, &format!("platform-{username}"))?;
    if !valid {
        sqlx::query("UPDATE platform_auth_tokens SET attempts=attempts+1 WHERE token_hash=$1")
            .bind(token_hash(token)).execute(&mut *tx).await?;
        sqlx::query("UPDATE platform_operators SET failed_logins=failed_logins+1,locked_until=CASE WHEN failed_logins+1>=5 THEN clock_timestamp()+INTERVAL '15 minutes' ELSE NULL END,updated_at=clock_timestamp() WHERE id=$1")
            .bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        return Err(AppError::Unauthorized("Invalid authentication code".into()));
    }
    if !enabled {
        sqlx::query("UPDATE platform_operators SET totp_secret=totp_pending_secret,totp_pending_secret=NULL,totp_enabled=true,updated_at=clock_timestamp() WHERE id=$1")
            .bind(id).execute(&mut *tx).await?;
    }
    sqlx::query("UPDATE platform_operators SET failed_logins=0,locked_until=NULL WHERE id=$1")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM platform_auth_tokens WHERE token_hash=$1").bind(token_hash(token)).execute(&mut *tx).await?;
    // Invalidate every older session after initial enrollment.
    if !enabled {
        sqlx::query("DELETE FROM platform_auth_tokens WHERE operator_id=$1 AND purpose='SESSION'").bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"token":issue(&c,id,"SESSION").await?,"username":username})))
}

async fn logout(State(c): State<Context>, headers: HeaderMap) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM platform_auth_tokens WHERE token_hash=$1 AND purpose='SESSION'")
        .bind(token_hash(bearer(&headers)?)).execute(pool(&c)?).await?;
    Ok(StatusCode::NO_CONTENT)
}
