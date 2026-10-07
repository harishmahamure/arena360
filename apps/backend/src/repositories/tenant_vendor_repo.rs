//! Tenant-local supplier CRUD. Uniqueness checks run under the same writer as mutations.
use super::tenant_back_office::{event, now, write};
use crate::{dto::PaginationResult, error::AppError, models::*, tenancy::TenantDb};
use serde_json::json;
use sqlx::{QueryBuilder, Sqlite};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub struct TenantVendorRepository {
    db: Arc<TenantDb>,
}
impl TenantVendorRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    const SELECT: &'static str = r#"
                        SELECT unhex(replace(id, '-' , '' )) AS id, name, contact_person, phone, email, address, gst_number,
                        is_active, notes, unhex(replace(created_by, '-' , '' )) AS created_by, unhex(replace(updated_by, '-'
                        , '' )) AS updated_by, created_at, updated_at FROM vendors
                    "#;
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Vendor>, AppError> {
        Ok(sqlx::query_as(&format!("{} WHERE id=?", Self::SELECT))
            .bind(id.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?)
    }
    pub async fn list(&self, f: &VendorFilterDto) -> Result<PaginationResult<Vendor>, AppError> {
        let page = f.page.unwrap_or(1).max(1);
        let limit = f.limit.unwrap_or(10).clamp(1, 100);
        let mut q = QueryBuilder::<Sqlite>::new(format!("{} WHERE 1=1", Self::SELECT));
        Self::filters(&mut q, f);
        let col = if f.sort_by.as_deref() == Some("name") {
            "name"
        } else {
            "created_at"
        };
        let dir = if f.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        q.push(format!(" ORDER BY {col} {dir},id {dir} LIMIT "))
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1).saturating_mul(limit));
        let pool = self.db.read_pool()?;
        let items = q.build_query_as().fetch_all(&pool).await?;
        let mut count = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM vendors WHERE 1=1");
        Self::filters(&mut count, f);
        let total: i64 = count.build_query_scalar().fetch_one(&pool).await?;
        Ok(PaginationResult::new(items, total, page, limit))
    }
    fn filters(q: &mut QueryBuilder<Sqlite>, f: &VendorFilterDto) {
        if let Some(name) = &f.name {
            q.push(" AND name LIKE ").push_bind(format!("%{name}%"));
        }
        if let Some(active) = f.is_active {
            q.push(" AND is_active=").push_bind(active);
        }
    }
    pub async fn create(
        &self,
        dto: &CreateVendorDto,
        actor: Option<Uuid>,
    ) -> Result<Vendor, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    Self::ensure_name(c, &dto.name, None).await?;
                    let id = Uuid::now_v7();
                    let ts = now()?;
                    sqlx::query(r#"
                        INSERT INTO vendors(id, name, contact_person, phone, email, address, gst_number, is_active, notes,
                        created_by, updated_by, created_at, updated_at) VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#).bind(id.to_string()).bind(&dto.name).bind(dto.contact_person).bind(dto.phone).bind(dto.email).bind(dto.address).bind(dto.gst_number).bind(dto.is_active.unwrap_or(true)).bind(dto.notes).bind(actor.map(|x| x.to_string())).bind(actor.map(|x| x.to_string())).bind(&ts).bind(&ts).execute(&mut *c).await?;
                    let row: Vendor = sqlx::query_as(&format!("{} WHERE id=?", Self::SELECT)).bind(id.to_string()).fetch_one(&mut *c).await?;
                    event(c, "vendor", id, "vendor.created", None, false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateVendorDto,
        actor: Option<Uuid>,
    ) -> Result<Vendor, AppError> {
        let dto = dto.clone();
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    if let Some(name) = &dto.name {
                        Self::ensure_name(c, name, Some(id)).await?;
                    }
                    let n = sqlx::query(r#"
                        UPDATE vendors SET name=COALESCE(?, name), contact_person=COALESCE(?, contact_person),
                        phone=COALESCE(?, phone), email=COALESCE(?, email), address=COALESCE(?, address),
                        gst_number=COALESCE(?, gst_number), is_active=COALESCE(?, is_active), notes=COALESCE(?, notes),
                        updated_by=?, updated_at=? WHERE id=?
                    "#).bind(dto.name).bind(dto.contact_person).bind(dto.phone).bind(dto.email).bind(dto.address).bind(dto.gst_number).bind(dto.is_active).bind(dto.notes).bind(actor.map(|x| x.to_string())).bind(now()?).bind(id.to_string()).execute(&mut *c).await?.rows_affected();
                    if n == 0 {
                        return Err(AppError::NotFound(format!("Vendor with ID {id} not found")));
                    }
                    let row: Vendor = sqlx::query_as(&format!("{} WHERE id=?", Self::SELECT)).bind(id.to_string()).fetch_one(&mut *c).await?;
                    event(c, "vendor", id, "vendor.updated", None, false, json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    if sqlx::query("DELETE FROM vendors WHERE id=?")
                        .bind(id.to_string())
                        .execute(&mut *c)
                        .await?
                        .rows_affected()
                        == 0
                    {
                        return Err(AppError::NotFound(format!("Vendor with ID {id} not found")));
                    }
                    event(
                        c,
                        "vendor",
                        id,
                        "vendor.deleted",
                        None,
                        true,
                        json!({"id":id}),
                    )
                    .await
                })
            }),
        )
        .await
    }
    async fn ensure_name(
        c: &mut sqlx::SqliteConnection,
        name: &str,
        exclude: Option<Uuid>,
    ) -> Result<(), AppError> {
        if name.trim().is_empty() || name.len() > 200 {
            return Err(AppError::BadRequest("Invalid vendor name".into()));
        }
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM vendors WHERE lower(name)=lower(?) AND (? IS NULL OR id<>?))").bind(name).bind(exclude.map(|x| x.to_string())).bind(exclude.map(|x| x.to_string())).fetch_one(c).await?;
        if exists {
            return Err(AppError::Conflict(format!(
                "Vendor '{name}' already exists"
            )));
        }
        Ok(())
    }
}
