use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateProductDto, Product, ProductFilterDto, UpdateProductDto};

pub struct ProductRepository {
    pool: PgPool,
}

impl ProductRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, name, description,
               price::float8 as price,
               "purchasePrice"::float8 as purchase_price,
               category::text as category,
               sku,
               "stockQuantity" as stock_quantity,
               "isActive" as is_active,
               "createdBy" as created_by,
               "updatedBy" as updated_by,
               "createdAt" as created_at,
               "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM products
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Product>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let product = sqlx::query_as::<_, Product>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(product)
    }

    pub async fn list(
        &self,
        filters: &ProductFilterDto,
    ) -> Result<PaginationResult<Product>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, name, description, price::float8 as price, \
             \"purchasePrice\"::float8 as purchase_price, category::text as category, sku, \
             \"stockQuantity\" as stock_quantity, \"isActive\" as is_active, \
             \"createdBy\" as created_by, \"updatedBy\" as updated_by, \
             \"createdAt\" as created_at, \"updatedAt\" as updated_at, \"deletedAt\" as deleted_at \
             FROM products WHERE \"deletedAt\" IS NULL",
        );

        Self::apply_filters(&mut builder, filters);

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "name" => "name",
            "price" => "price",
            "stockQuantity" => "\"stockQuantity\"",
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

        let products = builder
            .build_query_as::<Product>()
            .fetch_all(&self.pool)
            .await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM products WHERE \"deletedAt\" IS NULL");
        Self::apply_filters(&mut count_builder, filters);

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(products, total.0, page, limit))
    }

    fn apply_filters(builder: &mut QueryBuilder<Postgres>, filters: &ProductFilterDto) {
        if let Some(name) = &filters.name {
            builder.push(" AND name ILIKE ");
            builder.push_bind(format!("%{name}%"));
        }
        if let Some(category) = filters.category.clone() {
            builder.push(" AND category::text = ");
            builder.push_bind(category);
        }
        if let Some(disabled) = filters.disabled {
            builder.push(" AND \"isActive\" = ");
            builder.push_bind(disabled == 0);
        }
        if let Some(min_price) = filters.min_price {
            builder.push(" AND price >= ");
            builder.push_bind(min_price);
        }
        if let Some(max_price) = filters.max_price {
            builder.push(" AND price <= ");
            builder.push_bind(max_price);
        }
    }

    pub async fn create(
        &self,
        dto: &CreateProductDto,
        actor_id: Option<Uuid>,
    ) -> Result<Product, AppError> {
        let category = dto.category.as_deref().unwrap_or("other");
        let stock_quantity = dto.stock_quantity.unwrap_or(0);
        let is_active = dto.is_active.unwrap_or(true);

        let product = sqlx::query_as::<_, Product>(
            r#"
            INSERT INTO products (
                id, name, description, price, "purchasePrice", category, sku,
                "stockQuantity", "isActive", "createdBy", "updatedBy", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3, $4, $5::products_category_enum, $6,
                $7, $8, $9, $9, NOW(), NOW()
            )
            RETURNING id, name, description,
                      price::float8 as price,
                      "purchasePrice"::float8 as purchase_price,
                      category::text as category,
                      sku,
                      "stockQuantity" as stock_quantity,
                      "isActive" as is_active,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.description)
        .bind(dto.price)
        .bind(dto.purchase_price)
        .bind(category)
        .bind(&dto.sku)
        .bind(stock_quantity)
        .bind(is_active)
        .bind(actor_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(product)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateProductDto,
        actor_id: Option<Uuid>,
    ) -> Result<Product, AppError> {
        let product = sqlx::query_as::<_, Product>(
            r#"
            UPDATE products SET
                name = COALESCE($2, name),
                description = COALESCE($3, description),
                price = COALESCE($4, price),
                "purchasePrice" = COALESCE($5, "purchasePrice"),
                category = COALESCE($6::products_category_enum, category),
                sku = COALESCE($7, sku),
                "stockQuantity" = COALESCE($8, "stockQuantity"),
                "isActive" = COALESCE($9, "isActive"),
                "updatedBy" = COALESCE($10, "updatedBy"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, description,
                      price::float8 as price,
                      "purchasePrice"::float8 as purchase_price,
                      category::text as category,
                      sku,
                      "stockQuantity" as stock_quantity,
                      "isActive" as is_active,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.description)
        .bind(dto.price)
        .bind(dto.purchase_price)
        .bind(&dto.category)
        .bind(&dto.sku)
        .bind(dto.stock_quantity)
        .bind(dto.is_active)
        .bind(actor_id)
        .fetch_optional(&self.pool)
        .await?;

        product.ok_or_else(|| AppError::NotFound(format!("Product with ID {id} not found")))
    }

    pub async fn deactivate(&self, id: Uuid) -> Result<Product, AppError> {
        let product = sqlx::query_as::<_, Product>(
            r#"
            UPDATE products SET "isActive" = false, "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, description,
                      price::float8 as price,
                      "purchasePrice"::float8 as purchase_price,
                      category::text as category,
                      sku,
                      "stockQuantity" as stock_quantity,
                      "isActive" as is_active,
                      "createdBy" as created_by,
                      "updatedBy" as updated_by,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        product.ok_or_else(|| AppError::NotFound(format!("Product with ID {id} not found")))
    }

    pub async fn sku_exists(&self, sku: &str, exclude_id: Option<Uuid>) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM products WHERE LOWER(sku) = LOWER($1) AND id != $2 AND "deletedAt" IS NULL)"#,
                )
                .bind(sku)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM products WHERE LOWER(sku) = LOWER($1) AND "deletedAt" IS NULL)"#,
                )
                .bind(sku)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }

    pub async fn name_exists(
        &self,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM products WHERE LOWER(name) = LOWER($1) AND id != $2 AND "deletedAt" IS NULL)"#,
                )
                .bind(name)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM products WHERE LOWER(name) = LOWER($1) AND "deletedAt" IS NULL)"#,
                )
                .bind(name)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }
}
