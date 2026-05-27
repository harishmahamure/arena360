use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateVendorDto, UpdateVendorDto, Vendor, VendorFilterDto};

pub struct VendorRepository {
    pool: PgPool,
}

impl VendorRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, name,
               "contactPerson" as contact_person,
               phone, email, address,
               "gstNumber" as gst_number,
               "isActive" as is_active,
               notes,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at
        FROM vendors
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Vendor>, AppError> {
        let query = format!("{} WHERE id = $1", Self::SELECT);
        let vendor = sqlx::query_as::<_, Vendor>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(vendor)
    }

    pub async fn list(
        &self,
        filters: &VendorFilterDto,
    ) -> Result<PaginationResult<Vendor>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, name, \
             \"contactPerson\" as contact_person, \
             phone, email, address, \
             \"gstNumber\" as gst_number, \
             \"isActive\" as is_active, \
             notes, \
             \"createdBy\" as created_by, \
             \"updatedBy\" as updated_by, \
             \"createdAt\" as created_at, \
             \"updatedAt\" as updated_at \
             FROM vendors WHERE 1=1",
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "name" => "name",
            _ => "\"createdAt\"",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(" ORDER BY {sort_col} {sort_order} LIMIT "));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let items = builder
            .build_query_as::<Vendor>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM vendors WHERE 1=1");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(items, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &VendorFilterDto) {
        if let Some(name) = &filters.name {
            builder.push(" AND name ILIKE ");
            builder.push_bind(format!("%{name}%"));
        }
        if let Some(is_active) = filters.is_active {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(is_active);
        }
    }

    pub async fn create(
        &self,
        dto: &CreateVendorDto,
        created_by: Option<Uuid>,
    ) -> Result<Vendor, AppError> {
        let is_active = dto.is_active.unwrap_or(true);

        let vendor = sqlx::query_as::<_, Vendor>(
            r#"
            INSERT INTO vendors (
                id, name, "contactPerson", phone, email, address,
                "gstNumber", "isActive", notes,
                "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, $9, NOW(), NOW()
            )
            RETURNING id, name,
                      "contactPerson" as contact_person,
                      phone, email, address,
                      "gstNumber" as gst_number,
                      "isActive" as is_active,
                      notes,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.contact_person)
        .bind(&dto.phone)
        .bind(&dto.email)
        .bind(&dto.address)
        .bind(&dto.gst_number)
        .bind(is_active)
        .bind(&dto.notes)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(vendor)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateVendorDto,
        updated_by: Option<Uuid>,
    ) -> Result<Vendor, AppError> {
        let vendor = sqlx::query_as::<_, Vendor>(
            r#"
            UPDATE vendors SET
                name = COALESCE($2, name),
                "contactPerson" = COALESCE($3, "contactPerson"),
                phone = COALESCE($4, phone),
                email = COALESCE($5, email),
                address = COALESCE($6, address),
                "gstNumber" = COALESCE($7, "gstNumber"),
                "isActive" = COALESCE($8, "isActive"),
                notes = COALESCE($9, notes),
                "updatedBy" = $10,
                "updatedAt" = NOW()
            WHERE id = $1
            RETURNING id, name,
                      "contactPerson" as contact_person,
                      phone, email, address,
                      "gstNumber" as gst_number,
                      "isActive" as is_active,
                      notes,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at
            "#,
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.contact_person)
        .bind(&dto.phone)
        .bind(&dto.email)
        .bind(&dto.address)
        .bind(&dto.gst_number)
        .bind(dto.is_active)
        .bind(&dto.notes)
        .bind(updated_by)
        .fetch_optional(&self.pool)
        .await?;

        vendor.ok_or_else(|| AppError::NotFound(format!("Vendor with ID {id} not found")))
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM vendors WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Vendor with ID {id} not found")));
        }
        Ok(())
    }

    pub async fn name_exists(
        &self,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM vendors WHERE LOWER(name) = LOWER($1) AND id != $2)"#,
                )
                .bind(name)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM vendors WHERE LOWER(name) = LOWER($1))"#,
                )
                .bind(name)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }
}
