use super::tenant_back_office::{event, now, write};
use crate::{
    error::AppError,
    models::{ConfigFilterDto, Configuration, UpsertConfigDto},
    tenancy::TenantDb,
};
use sqlx::{QueryBuilder, Sqlite};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub struct TenantConfigRepository {
    db: Arc<TenantDb>,
}
const SELECT: &str = "SELECT unhex(replace(id,'-','')) AS id,key,value,category,description,unhex(replace(created_by,'-','')) AS created_by,unhex(replace(updated_by,'-','')) AS updated_by,created_at,updated_at FROM configurations";
impl TenantConfigRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    pub async fn find_all(&self, f: &ConfigFilterDto) -> Result<Vec<Configuration>, AppError> {
        let mut b = QueryBuilder::<Sqlite>::new(format!("{SELECT} WHERE 1=1"));
        if let Some(category) = &f.category {
            b.push(" AND category=").push_bind(category);
        }
        if let Some(key) = &f.key {
            b.push(" AND key LIKE ").push_bind(format!("%{key}%"));
        }
        b.push(" ORDER BY category,key,id");
        Ok(b.build_query_as().fetch_all(&self.db.read_pool()?).await?)
    }
    pub async fn find_by_key(&self, key: &str) -> Result<Option<Configuration>, AppError> {
        Ok(sqlx::query_as(&format!("{SELECT} WHERE key=?"))
            .bind(key)
            .fetch_optional(&self.db.read_pool()?)
            .await?)
    }
    pub async fn upsert(
        &self,
        key: &str,
        category: &str,
        dto: &UpsertConfigDto,
        actor: Uuid,
    ) -> Result<Configuration, AppError> {
        let key = key.to_owned();
        let category = category.to_owned();
        let value = dto.value.to_string();
        let description = dto.description.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let ts = now()?;
                    sqlx::query("INSERT INTO configurations(id,key,value,category,description,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value,description=COALESCE(excluded.description,configurations.description),updated_by=excluded.updated_by,updated_at=excluded.updated_at").bind(Uuid::now_v7().to_string()).bind(&key).bind(&value).bind(category).bind(description).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let existing: Option<(String, String, i64)> = sqlx::query_as("SELECT id,value,revision FROM setting_overrides WHERE location_id IS NULL AND key=?").bind(&key).fetch_optional(&mut *c).await?;
                    let historical: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(revision),0) FROM setting_revisions WHERE location_id IS NULL AND key=?").bind(&key).fetch_one(&mut *c).await?;
                    let revision = existing.as_ref().map_or(0, |r| r.2).max(historical).checked_add(1).ok_or_else(|| AppError::Conflict("Setting revision exhausted".into()))?;
                    let setting = existing.as_ref().map(|r| r.0.clone()).unwrap_or_else(|| Uuid::now_v7().to_string());
                    sqlx::query("INSERT INTO setting_overrides(id,key,value,revision,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET value=excluded.value,revision=excluded.revision,updated_by=excluded.updated_by,updated_at=excluded.updated_at").bind(setting).bind(&key).bind(&value).bind(revision).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    sqlx::query("INSERT INTO setting_revisions(key,revision,operation,old_value,new_value,reason,actor_user_id,created_at) VALUES(?,?,?,?,?,'Updated through /config compatibility endpoint',?,?)").bind(&key).bind(revision).bind(if existing.is_some() { "update" } else { "create" }).bind(existing.as_ref().map(|r| r.1.as_str())).bind(value).bind(actor.to_string()).bind(&ts).execute(&mut *c).await?;
                    let row: Configuration = sqlx::query_as(&format!("{SELECT} WHERE key=?")).bind(&key).fetch_one(&mut *c).await?;
                    event(c, "configuration", row.id, "configuration.changed", None, false, serde_json::json!({"key":row.key,"revision":revision,"updatedAt":ts})).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn delete_by_key(&self, key: &str) -> Result<bool, AppError> {
        let key = key.to_owned();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let id: Option<String> =
                        sqlx::query_scalar("SELECT id FROM configurations WHERE key=?")
                            .bind(&key)
                            .fetch_optional(&mut *c)
                            .await?;
                    let Some(id) = id else { return Ok(false) };
                    sqlx::query("DELETE FROM configurations WHERE id=?")
                        .bind(&id)
                        .execute(&mut *c)
                        .await?;
                    event(
                        c,
                        "configuration",
                        Uuid::parse_str(&id).map_err(|e| AppError::Internal(e.to_string()))?,
                        "configuration.changed",
                        None,
                        true,
                        serde_json::json!({"key":key,"deleted":true}),
                    )
                    .await?;
                    Ok(true)
                })
            }),
        )
        .await
    }
}
