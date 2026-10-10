use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, NaiveTime, Utc};
use futures::future::BoxFuture;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use serde_json::{json, Value};
use sqlx::{FromRow, QueryBuilder, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    CreateGameDto, CreatePlanDto, CreateProductDto, CreateUnitDto, Game, GameFilterDto, Plan,
    PlanFilterDto, PricingRuleSet, PricingRuleVersion, Product, ProductFilterDto, ProductOption,
    ProductOptionGroup, ProductRecipe, RecipeIngredient, SettingHistoryQuery, SettingOverride,
    SettingRevision, Unit, UnitFilterDto, UpdateGameDto, UpdatePlanDto, UpdateProductDto,
    UpdateUnitDto, UpsertSettingOverrideDto,
};
use crate::tenancy::{
    decimal_to_scale4, format_sqlite_timestamp, scale4_to_decimal,
    write_outbox_event_on_connection, NewOutboxEvent, TenantDb,
};

/// An ingredient candidate as seen when validating a recipe.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct IngredientInfo {
    pub id: Uuid,
    pub name: String,
    pub is_raw_material: bool,
    pub has_recipe: bool,
}


type WriteOperation<T> = Box<
    dyn for<'connection> FnOnce(
            &'connection mut SqliteConnection,
        ) -> BoxFuture<'connection, Result<T, AppError>>
        + Send,
>;

async fn write<T: Send + 'static>(
    db: &TenantDb,
    operation: WriteOperation<T>,
) -> Result<T, AppError> {
    db.with_immediate_writer(operation).await
}

fn now() -> Result<String, AppError> {
    format_sqlite_timestamp(&Utc::now())
        .map_err(|error| AppError::Internal(format!("format tenant timestamp: {error}")))
}

fn money(value: f64) -> Result<i64, AppError> {
    if !value.is_finite() {
        return Err(AppError::BadRequest("Money must be finite".into()));
    }
    let decimal = Decimal::from_str(&value.to_string())
        .map_err(|_| AppError::BadRequest("Invalid money value".into()))?;
    decimal_to_scale4(decimal)
        .map_err(|error| AppError::BadRequest(format!("Invalid money value: {error}")))
}

fn money_f64(value: i64) -> Result<f64, AppError> {
    scale4_to_decimal(value)
        .to_f64()
        .ok_or_else(|| AppError::Internal("Stored money is outside f64 range".into()))
}

fn json_text(value: Option<&Value>) -> Result<Option<String>, AppError> {
    value
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| AppError::BadRequest(format!("Invalid JSON value: {error}")))
}

async fn event(
    connection: &mut SqliteConnection,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    location_id: Option<Uuid>,
    deleted: bool,
    payload: Value,
) -> Result<(), AppError> {
    write_outbox_event_on_connection(
        connection,
        NewOutboxEvent {
            location_id,
            aggregate_type: aggregate_type.into(),
            aggregate_id,
            event_type: event_type.into(),
            schema_version: 1,
            deleted,
            payload,
        },
    )
    .await?;
    Ok(())
}

async fn catalog_location_scope(
    db: &TenantDb,
    parent_table: &str,
    location_table: &str,
    parent_key: &str,
    id: Uuid,
) -> Result<(bool, Vec<(Uuid, Option<f64>)>), AppError> {
    let availability: Option<String> = sqlx::query_scalar(&format!(
        "SELECT availability_scope FROM {parent_table} WHERE id=? AND deleted_at IS NULL"
    ))
    .bind(id.to_string())
    .fetch_optional(&db.read_pool()?)
    .await?;
    let availability =
        availability.ok_or_else(|| AppError::NotFound("Catalog item not found".to_string()))?;
    let rows: Vec<(Uuid, Option<i64>)> = sqlx::query_as(&format!(
        "SELECT unhex(replace(location_id,'-','')),price
         FROM {location_table} WHERE {parent_key}=? ORDER BY location_id"
    ))
    .bind(id.to_string())
    .fetch_all(&db.read_pool()?)
    .await?;
    let rows = rows
        .into_iter()
        .map(|(location_id, price)| Ok((location_id, price.map(money_f64).transpose()?)))
        .collect::<Result<_, AppError>>()?;
    Ok((availability == "ALL", rows))
}

async fn replace_catalog_location_scope(
    db: Arc<TenantDb>,
    parent_table: &'static str,
    location_table: &'static str,
    parent_key: &'static str,
    aggregate_type: &'static str,
    id: Uuid,
    location_ids: Vec<Uuid>,
    prices: Vec<(Uuid, f64)>,
    replace_only_at: Option<Vec<Uuid>>,
) -> Result<(), AppError> {
    let timestamp = now()?;
    let replace_only_at_set = replace_only_at.as_ref().map(|ids| {
        ids.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
    });
    let mut rows = location_ids
        .iter()
        .copied()
        .map(|location_id| (location_id, None))
        .collect::<BTreeMap<_, _>>();
    for (location_id, price) in prices {
        rows.insert(location_id, Some(money(price)?));
    }
    if let Some(allowed) = &replace_only_at_set {
        rows.retain(|location_id, _| allowed.contains(location_id));
    }
    write(
        &db,
        Box::new(move |connection| {
            Box::pin(async move {
                let result = sqlx::query(&format!(
                    "UPDATE {parent_table} SET availability_scope=?,updated_at=?
                     WHERE id=? AND deleted_at IS NULL"
                ))
                .bind(if location_ids.is_empty() {
                    "ALL"
                } else {
                    "SELECTED"
                })
                .bind(&timestamp)
                .bind(id.to_string())
                .execute(&mut *connection)
                .await?;
                if result.rows_affected() == 0 {
                    return Err(AppError::NotFound("Catalog item not found".to_string()));
                }
                if let Some(location_ids) = replace_only_at {
                    if !location_ids.is_empty() {
                        let mut delete = QueryBuilder::<Sqlite>::new(format!(
                            "DELETE FROM {location_table} WHERE {parent_key}="
                        ));
                        delete.push_bind(id.to_string());
                        delete.push(" AND location_id IN (");
                        let mut separated = delete.separated(",");
                        for location_id in location_ids {
                            separated.push_bind(location_id.to_string());
                        }
                        separated.push_unseparated(")");
                        delete.build().execute(&mut *connection).await?;
                    }
                } else {
                    sqlx::query(&format!(
                        "DELETE FROM {location_table} WHERE {parent_key}=?"
                    ))
                    .bind(id.to_string())
                    .execute(&mut *connection)
                    .await?;
                }
                for (location_id, price) in rows {
                    sqlx::query(&format!(
                        "INSERT INTO {location_table}({parent_key},location_id,price)
                         VALUES (?,?,?)"
                    ))
                    .bind(id.to_string())
                    .bind(location_id.to_string())
                    .bind(price)
                    .execute(&mut *connection)
                    .await?;
                }
                event(
                    connection,
                    aggregate_type,
                    id,
                    &format!("{aggregate_type}.updated"),
                    None,
                    false,
                    json!({"id":id,"locationIds":location_ids,"updatedAt":timestamp}),
                )
                .await
            })
        }),
    )
    .await
}

const PRODUCT_SELECT: &str = r#"
    SELECT unhex(replace(id,'-','')) AS id, name, description,
           day_price / 10000.0 AS price,
           purchase_price_per_box / 10000.0 AS purchase_price,
           unhex(replace(unit_id,'-','')) AS unit_id,
           unhex(replace(purchase_unit_id,'-','')) AS purchase_unit_id,
           units_per_purchase_unit,
           day_price / 10000.0 AS day_price,
           night_price / 10000.0 AS night_price,
           purchase_price_per_box / 10000.0 AS purchase_price_per_box,
           category, sku, 0 AS stock_quantity, is_active, is_raw_material,
           unhex(replace(created_by,'-','')) AS created_by,
           unhex(replace(updated_by,'-','')) AS updated_by,
           created_at, updated_at, deleted_at
      FROM products
"#;

#[derive(Clone)]
pub struct TenantProductRepository {
    db: Arc<TenantDb>,
    location_ids: Vec<Uuid>,
}

impl TenantProductRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self {
            db,
            location_ids: Vec::new(),
        }
    }

    pub fn with_locations(mut self, location_ids: Vec<Uuid>) -> Self {
        self.location_ids = location_ids;
        self
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Product>, AppError> {
        Ok(sqlx::query_as::<_, Product>(&format!(
            "{PRODUCT_SELECT} WHERE id = ? AND deleted_at IS NULL"
        ))
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }

    pub async fn sale_price_rows(
        &self,
        location_id: Uuid,
    ) -> Result<Vec<(Uuid, f64, f64, String)>, AppError> {
        Ok(sqlx::query_as(
            "SELECT unhex(replace(p.id,'-','')),
                    COALESCE(pl.price,p.day_price)/10000.0,
                    COALESCE(pl.price,p.night_price)/10000.0,
                    p.category
             FROM products p
             LEFT JOIN product_locations pl
               ON pl.product_id=p.id AND pl.location_id=?
             WHERE p.deleted_at IS NULL AND p.is_raw_material=0
               AND (p.availability_scope='ALL' OR pl.product_id IS NOT NULL)
             ORDER BY p.id",
        )
        .bind(location_id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }

    pub async fn location_scope(
        &self,
        id: Uuid,
    ) -> Result<(bool, Vec<(Uuid, Option<f64>)>), AppError> {
        catalog_location_scope(&self.db, "products", "product_locations", "product_id", id).await
    }

    pub async fn replace_location_scope(
        &self,
        id: Uuid,
        location_ids: Vec<Uuid>,
        prices: Vec<(Uuid, f64)>,
    ) -> Result<(), AppError> {
        replace_catalog_location_scope(
            self.db.clone(),
            "products",
            "product_locations",
            "product_id",
            "product",
            id,
            location_ids,
            prices,
            None,
        )
        .await
    }

    pub async fn replace_authorized_location_prices(
        &self,
        id: Uuid,
        location_ids: Vec<Uuid>,
        prices: Vec<(Uuid, f64)>,
        authorized_location_ids: Vec<Uuid>,
    ) -> Result<(), AppError> {
        replace_catalog_location_scope(
            self.db.clone(),
            "products",
            "product_locations",
            "product_id",
            "product",
            id,
            location_ids,
            prices,
            Some(authorized_location_ids),
        )
        .await
    }

    pub async fn list(
        &self,
        filters: &ProductFilterDto,
    ) -> Result<PaginationResult<Product>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let mut query =
            QueryBuilder::<Sqlite>::new(format!("{PRODUCT_SELECT} WHERE deleted_at IS NULL"));
        self.product_filters(&mut query, filters)?;
        let sort = match filters.sort_by.as_deref() {
            Some("name") => "name",
            Some("price") => "day_price",
            Some("stockQuantity") => "stock_quantity",
            _ => "created_at",
        };
        let direction = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        query.push(format!(
            " ORDER BY {sort} {direction}, id {direction} LIMIT "
        ));
        query.push_bind(limit);
        query.push(" OFFSET ");
        query.push_bind((page - 1) * limit);
        let rows = query
            .build_query_as::<Product>()
            .fetch_all(&self.db.read_pool()?)
            .await?;

        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM products WHERE deleted_at IS NULL");
        self.product_filters(&mut count, filters)?;
        let total = count
            .build_query_scalar::<i64>()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total, page, limit))
    }

    fn product_filters<'a>(
        &'a self,
        query: &mut QueryBuilder<'a, Sqlite>,
        filters: &'a ProductFilterDto,
    ) -> Result<(), AppError> {
        // Handler-supplied authorization always wins. Never widen that scope by
        // unioning a raw requested location from the query string.
        let requested_locations = if let Some(allowed) = &filters.allowed_location_ids {
            allowed.clone()
        } else if !self.location_ids.is_empty() {
            self.location_ids.clone()
        } else {
            filters.location_id.iter().copied().collect()
        };
        if !requested_locations.is_empty() {
            query.push(
                " AND (availability_scope = 'ALL' OR EXISTS (
                    SELECT 1 FROM product_locations pl
                    WHERE pl.product_id = products.id AND pl.location_id IN (",
            );
            let mut separated = query.separated(", ");
            for id in requested_locations {
                separated.push_bind(id.to_string());
            }
            separated.push_unseparated(")))");
        }
        if let Some(name) = &filters.name {
            query.push(" AND instr(lower(name), lower(");
            query.push_bind(name);
            query.push(")) > 0");
        }
        if let Some(category) = &filters.category {
            query.push(" AND category = ");
            query.push_bind(category);
        }
        if let Some(disabled) = filters.disabled {
            query.push(" AND is_active = ");
            query.push_bind(disabled == 0);
        }
        if let Some(value) = filters.min_price {
            query.push(" AND day_price >= ");
            query.push_bind(money(value)?);
        }
        if let Some(value) = filters.max_price {
            query.push(" AND day_price <= ");
            query.push_bind(money(value)?);
        }
        if let Some(for_sale) = filters.for_sale {
            query.push(" AND is_raw_material = ");
            query.push_bind(!for_sale);
        }
        Ok(())
    }

    pub async fn create(
        &self,
        dto: &CreateProductDto,
        actor_id: Option<Uuid>,
    ) -> Result<Product, AppError> {
        let id = Uuid::now_v7();
        let timestamp = now()?;
        let locations = self.location_ids.clone();
        let day_price = money(dto.day_price.unwrap_or(dto.price))?;
        let night_price = money(dto.night_price.unwrap_or(dto.price))?;
        let purchase_price = dto
            .purchase_price_per_box
            .or(dto.purchase_price)
            .map(money)
            .transpose()?;
        let name = dto.name.clone();
        let description = dto.description.clone();
        let category = dto.category.clone().unwrap_or_else(|| "other".into());
        let sku = dto.sku.clone();
        let unit_id = dto.unit_id;
        let purchase_unit_id = dto.purchase_unit_id;
        let units_per = dto.units_per_purchase_unit.unwrap_or(1);
        let active = dto.is_active.unwrap_or(true);
        let raw = dto.is_raw_material.unwrap_or(false);
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    sqlx::query(
                        r#"INSERT INTO products
                           (id,name,description,day_price,night_price,purchase_price_per_box,
                            unit_id,purchase_unit_id,units_per_purchase_unit,category,sku,is_active,
                            is_raw_material,availability_scope,created_by,updated_by,created_at,updated_at)
                           VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#,
                    )
                    .bind(id.to_string())
                    .bind(&name)
                    .bind(description)
                    .bind(day_price)
                    .bind(night_price)
                    .bind(purchase_price)
                    .bind(unit_id.map(|value| value.to_string()))
                    .bind(purchase_unit_id.map(|value| value.to_string()))
                    .bind(units_per)
                    .bind(category)
                    .bind(sku)
                    .bind(active)
                    .bind(raw)
                    .bind(if locations.is_empty() { "ALL" } else { "SELECTED" })
                    .bind(actor_id.map(|value| value.to_string()))
                    .bind(actor_id.map(|value| value.to_string()))
                    .bind(&timestamp)
                    .bind(&timestamp)
                    .execute(&mut *connection)
                    .await?;
                    for location_id in locations {
                        sqlx::query(
                            "INSERT INTO product_locations(product_id, location_id) VALUES (?, ?)",
                        )
                        .bind(id.to_string())
                        .bind(location_id.to_string())
                        .execute(&mut *connection)
                        .await?;
                    }
                    event(
                        connection,
                        "product",
                        id,
                        "product.created",
                        None,
                        false,
                        json!({"id": id, "name": name, "updatedAt": timestamp}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Created product disappeared".into()))
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateProductDto,
        actor_id: Option<Uuid>,
    ) -> Result<Product, AppError> {
        let timestamp = now()?;
        let day_price = dto.day_price.or(dto.price).map(money).transpose()?;
        let night_price = dto.night_price.map(money).transpose()?;
        let purchase_price = dto
            .purchase_price_per_box
            .or(dto.purchase_price)
            .map(money)
            .transpose()?;
        let name = dto.name.clone();
        let description = dto.description.clone();
        let category = dto.category.clone();
        let sku = dto.sku.clone();
        let unit_id = dto.unit_id;
        let purchase_unit_id = dto.purchase_unit_id;
        let units_per = dto.units_per_purchase_unit;
        let active = dto.is_active;
        let raw = dto.is_raw_material;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let result = sqlx::query(
                        r#"UPDATE products SET name=COALESCE(?,name),
                           description=COALESCE(?,description),day_price=COALESCE(?,day_price),
                           night_price=COALESCE(?,night_price),
                           purchase_price_per_box=COALESCE(?,purchase_price_per_box),
                           unit_id=COALESCE(?,unit_id),purchase_unit_id=COALESCE(?,purchase_unit_id),
                           units_per_purchase_unit=COALESCE(?,units_per_purchase_unit),
                           category=COALESCE(?,category),sku=COALESCE(?,sku),
                           is_active=COALESCE(?,is_active),is_raw_material=COALESCE(?,is_raw_material),
                           updated_by=COALESCE(?,updated_by),updated_at=?
                           WHERE id=? AND deleted_at IS NULL"#,
                    )
                    .bind(name)
                    .bind(description)
                    .bind(day_price)
                    .bind(night_price)
                    .bind(purchase_price)
                    .bind(unit_id.map(|value| value.to_string()))
                    .bind(purchase_unit_id.map(|value| value.to_string()))
                    .bind(units_per)
                    .bind(category)
                    .bind(sku)
                    .bind(active)
                    .bind(raw)
                    .bind(actor_id.map(|value| value.to_string()))
                    .bind(&timestamp)
                    .bind(id.to_string())
                    .execute(&mut *connection)
                    .await?;
                    if result.rows_affected() == 0 {
                        return Err(AppError::NotFound(format!("Product with ID {id} not found")));
                    }
                    event(
                        connection,
                        "product",
                        id,
                        "product.updated",
                        None,
                        false,
                        json!({"id": id, "updatedAt": timestamp}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Product with ID {id} not found")))
    }

    pub async fn deactivate(&self, id: Uuid) -> Result<Product, AppError> {
        let dto = UpdateProductDto {
            name: None,
            description: None,
            price: None,
            purchase_price: None,
            unit_id: None,
            purchase_unit_id: None,
            units_per_purchase_unit: None,
            day_price: None,
            night_price: None,
            purchase_price_per_box: None,
            category: None,
            sku: None,
            stock_quantity: None,
            is_active: Some(false),
            is_raw_material: None,
        };
        self.update(id, &dto, None).await
    }

    pub async fn sku_exists(&self, sku: &str, exclude: Option<Uuid>) -> Result<bool, AppError> {
        self.text_exists("sku", sku, exclude).await
    }

    pub async fn name_exists(&self, name: &str, exclude: Option<Uuid>) -> Result<bool, AppError> {
        self.text_exists("name", name, exclude).await
    }

    async fn text_exists(
        &self,
        column: &str,
        value: &str,
        exclude: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM products WHERE lower({column})=lower(?) \
             AND (? IS NULL OR id<>?) AND deleted_at IS NULL)"
        );
        Ok(sqlx::query_scalar::<_, bool>(&sql)
            .bind(value)
            .bind(exclude.map(|id| id.to_string()))
            .bind(exclude.map(|id| id.to_string()))
            .fetch_one(&self.db.read_pool()?)
            .await?)
    }
}

const PLAN_SELECT: &str = r#"
    SELECT unhex(replace(id,'-','')) AS id,name,description,
           price / 10000.0 AS price,plan_type,validity_days,
           time_window_start,time_window_end,time_credits,is_active,device_type,device_sub_type,
           allowed_days,allowed_months,dynamic_deduction_enabled,deduction_profile,
           unhex(replace(created_by,'-','')) AS created_by,
           unhex(replace(updated_by,'-','')) AS updated_by,
           created_at,updated_at,deleted_at
      FROM plans
"#;

pub struct TenantPlanCreateValues<'a> {
    pub dto: &'a CreatePlanDto,
    pub validity_days: i32,
    pub time_credits: i32,
    pub time_window_start: Option<NaiveTime>,
    pub time_window_end: Option<NaiveTime>,
    pub dynamic_deduction_enabled: bool,
    pub deduction_profile: Option<Value>,
}

#[derive(Clone)]
pub struct TenantPlanRepository {
    db: Arc<TenantDb>,
    location_ids: Vec<Uuid>,
}

impl TenantPlanRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self {
            db,
            location_ids: Vec::new(),
        }
    }

    pub fn with_locations(mut self, ids: Vec<Uuid>) -> Self {
        self.location_ids = ids;
        self
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Plan>, AppError> {
        Ok(
            sqlx::query_as::<_, Plan>(&format!("{PLAN_SELECT} WHERE id=? AND deleted_at IS NULL"))
                .bind(id.to_string())
                .fetch_optional(&self.db.read_pool()?)
                .await?,
        )
    }

    pub async fn find_active(&self) -> Result<Vec<Plan>, AppError> {
        Ok(sqlx::query_as::<_, Plan>(&format!(
            "{PLAN_SELECT} WHERE is_active=1 AND deleted_at IS NULL ORDER BY name,id"
        ))
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }

    pub async fn location_price(
        &self,
        plan_id: Uuid,
        location_id: Uuid,
    ) -> Result<Option<f64>, AppError> {
        let value: Option<i64> = sqlx::query_scalar(
            "SELECT price FROM plan_locations WHERE plan_id=? AND location_id=?",
        )
        .bind(plan_id.to_string())
        .bind(location_id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?
        .flatten();
        value.map(money_f64).transpose()
    }

    pub async fn location_scope(
        &self,
        id: Uuid,
    ) -> Result<(bool, Vec<(Uuid, Option<f64>)>), AppError> {
        catalog_location_scope(&self.db, "plans", "plan_locations", "plan_id", id).await
    }

    pub async fn replace_location_scope(
        &self,
        id: Uuid,
        location_ids: Vec<Uuid>,
        prices: Vec<(Uuid, f64)>,
    ) -> Result<(), AppError> {
        replace_catalog_location_scope(
            self.db.clone(),
            "plans",
            "plan_locations",
            "plan_id",
            "plan",
            id,
            location_ids,
            prices,
            None,
        )
        .await
    }

    pub async fn replace_authorized_location_prices(
        &self,
        id: Uuid,
        location_ids: Vec<Uuid>,
        prices: Vec<(Uuid, f64)>,
        authorized_location_ids: Vec<Uuid>,
    ) -> Result<(), AppError> {
        replace_catalog_location_scope(
            self.db.clone(),
            "plans",
            "plan_locations",
            "plan_id",
            "plan",
            id,
            location_ids,
            prices,
            Some(authorized_location_ids),
        )
        .await
    }

    pub async fn list(&self, filters: &PlanFilterDto) -> Result<PaginationResult<Plan>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let mut query =
            QueryBuilder::<Sqlite>::new(format!("{PLAN_SELECT} WHERE deleted_at IS NULL"));
        self.plan_filters(&mut query, filters)?;
        let sort = match filters.sort_by.as_deref() {
            Some("name") => "name",
            Some("price") => "price",
            Some("planType") => "plan_type",
            _ => "created_at",
        };
        let direction = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        query.push(format!(
            " ORDER BY {sort} {direction}, id {direction} LIMIT "
        ));
        query.push_bind(limit);
        query.push(" OFFSET ");
        query.push_bind((page - 1) * limit);
        let rows = query
            .build_query_as::<Plan>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM plans WHERE deleted_at IS NULL");
        self.plan_filters(&mut count, filters)?;
        let total = count
            .build_query_scalar::<i64>()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total, page, limit))
    }

    fn plan_filters<'a>(
        &'a self,
        query: &mut QueryBuilder<'a, Sqlite>,
        filters: &'a PlanFilterDto,
    ) -> Result<(), AppError> {
        let requested_locations = if let Some(allowed) = &filters.allowed_location_ids {
            allowed.clone()
        } else if !self.location_ids.is_empty() {
            self.location_ids.clone()
        } else {
            filters.location_id.iter().copied().collect()
        };
        if !requested_locations.is_empty() {
            query.push(
                " AND (availability_scope='ALL' OR EXISTS (
                    SELECT 1 FROM plan_locations pl
                    WHERE pl.plan_id=plans.id AND pl.location_id IN (",
            );
            let mut separated = query.separated(", ");
            for id in requested_locations {
                separated.push_bind(id.to_string());
            }
            separated.push_unseparated(")))");
        }
        if let Some(search) = &filters.search {
            query.push(" AND (instr(lower(name),lower(");
            query.push_bind(search);
            query.push("))>0 OR instr(lower(COALESCE(description,'')),lower(");
            query.push_bind(search);
            query.push("))>0)");
        }
        for (column, value) in [
            ("plan_type", filters.plan_type.as_ref()),
            ("device_type", filters.device_type.as_ref()),
            ("device_sub_type", filters.device_sub_type.as_ref()),
        ] {
            if let Some(value) = value {
                query.push(format!(" AND {column}="));
                query.push_bind(value);
            }
        }
        if let Some(active) = filters.is_active_bool() {
            query.push(" AND is_active=");
            query.push_bind(active);
        }
        if let Some(value) = filters.min_price {
            query.push(" AND price>=");
            query.push_bind(money(value)?);
        }
        if let Some(value) = filters.max_price {
            query.push(" AND price<=");
            query.push_bind(money(value)?);
        }
        Ok(())
    }

    pub async fn create(
        &self,
        values: TenantPlanCreateValues<'_>,
        actor_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        let id = Uuid::now_v7();
        let timestamp = now()?;
        let locations = self.location_ids.clone();
        let dto = values.dto;
        let name = dto.name.clone();
        let description = dto.description.clone();
        let price = money(dto.price)?;
        let plan_type = dto.plan_type.clone();
        let start = values
            .time_window_start
            .map(|value| value.format("%H:%M:%S").to_string());
        let end = values
            .time_window_end
            .map(|value| value.format("%H:%M:%S").to_string());
        let allowed_days = json_text(dto.allowed_days.as_ref())?;
        let allowed_months = json_text(dto.allowed_months.as_ref())?;
        let deduction = json_text(values.deduction_profile.as_ref())?;
        let active = dto.is_active.unwrap_or(true);
        let device_type = dto.device_type.clone();
        let device_sub_type = dto.device_sub_type.clone();
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    sqlx::query(
                        r#"INSERT INTO plans
                           (id,name,description,price,plan_type,validity_days,time_window_start,
                            time_window_end,time_credits,is_active,device_type,device_sub_type,
                            allowed_days,allowed_months,dynamic_deduction_enabled,deduction_profile,
                            availability_scope,created_by,updated_by,created_at,updated_at)
                           VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"#,
                    )
                    .bind(id.to_string())
                    .bind(&name)
                    .bind(description)
                    .bind(price)
                    .bind(plan_type)
                    .bind(values.validity_days)
                    .bind(start)
                    .bind(end)
                    .bind(values.time_credits)
                    .bind(active)
                    .bind(device_type)
                    .bind(device_sub_type)
                    .bind(allowed_days)
                    .bind(allowed_months)
                    .bind(values.dynamic_deduction_enabled)
                    .bind(deduction)
                    .bind(if locations.is_empty() {
                        "ALL"
                    } else {
                        "SELECTED"
                    })
                    .bind(actor_id.map(|value| value.to_string()))
                    .bind(actor_id.map(|value| value.to_string()))
                    .bind(&timestamp)
                    .bind(&timestamp)
                    .execute(&mut *connection)
                    .await?;
                    for location in locations {
                        sqlx::query("INSERT INTO plan_locations(plan_id,location_id) VALUES (?,?)")
                            .bind(id.to_string())
                            .bind(location.to_string())
                            .execute(&mut *connection)
                            .await?;
                    }
                    event(
                        connection,
                        "plan",
                        id,
                        "plan.created",
                        None,
                        false,
                        json!({"id": id, "name": name, "updatedAt": timestamp}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Created plan disappeared".into()))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdatePlanDto,
        start: Option<NaiveTime>,
        end: Option<NaiveTime>,
        allowed_days: Option<&Value>,
        allowed_months: Option<&Value>,
        dynamic: Option<bool>,
        deduction: Option<&Value>,
        actor_id: Option<Uuid>,
    ) -> Result<Plan, AppError> {
        let timestamp = now()?;
        let name = dto.name.clone();
        let description = dto.description.clone();
        let price = dto.price.map(money).transpose()?;
        let plan_type = dto.plan_type.clone();
        let start = start.map(|value| value.format("%H:%M:%S").to_string());
        let end = end.map(|value| value.format("%H:%M:%S").to_string());
        let allowed_days = json_text(allowed_days)?;
        let allowed_months = json_text(allowed_months)?;
        let deduction = json_text(deduction)?;
        let validity_days = dto.validity_days;
        let time_credits = dto.time_credits;
        let is_active = dto.is_active;
        let device_type = dto.device_type.clone();
        let device_sub_type = dto.device_sub_type.clone();
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let result = sqlx::query(
                        r#"UPDATE plans SET name=COALESCE(?,name),description=COALESCE(?,description),
                           price=COALESCE(?,price),plan_type=COALESCE(?,plan_type),
                           validity_days=COALESCE(?,validity_days),
                           time_window_start=COALESCE(?,time_window_start),
                           time_window_end=COALESCE(?,time_window_end),
                           time_credits=COALESCE(?,time_credits),is_active=COALESCE(?,is_active),
                           device_type=COALESCE(?,device_type),
                           device_sub_type=COALESCE(?,device_sub_type),
                           allowed_days=COALESCE(?,allowed_days),
                           allowed_months=COALESCE(?,allowed_months),
                           dynamic_deduction_enabled=COALESCE(?,dynamic_deduction_enabled),
                           deduction_profile=?,updated_by=COALESCE(?,updated_by),updated_at=?
                           WHERE id=? AND deleted_at IS NULL"#,
                    )
                    .bind(name)
                    .bind(description)
                    .bind(price)
                    .bind(plan_type)
                    .bind(validity_days)
                    .bind(start)
                    .bind(end)
                    .bind(time_credits)
                    .bind(is_active)
                    .bind(device_type)
                    .bind(device_sub_type)
                    .bind(allowed_days)
                    .bind(allowed_months)
                    .bind(dynamic)
                    .bind(deduction)
                    .bind(actor_id.map(|value| value.to_string()))
                    .bind(&timestamp)
                    .bind(id.to_string())
                    .execute(&mut *connection)
                    .await?;
                    if result.rows_affected() == 0 {
                        return Err(AppError::NotFound(format!("Plan with ID {id} not found")));
                    }
                    event(
                        connection,
                        "plan",
                        id,
                        "plan.updated",
                        None,
                        false,
                        json!({"id": id, "updatedAt": timestamp}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Plan with ID {id} not found")))
    }

    pub async fn deactivate(&self, id: Uuid) -> Result<(), AppError> {
        let timestamp = now()?;
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    let result =
                        sqlx::query("UPDATE plans SET is_active=0,updated_at=? WHERE id=? AND deleted_at IS NULL")
                            .bind(&timestamp)
                            .bind(id.to_string())
                            .execute(&mut *connection)
                            .await?;
                    if result.rows_affected() == 0 {
                        return Err(AppError::NotFound(format!("Plan with ID {id} not found")));
                    }
                    event(connection, "plan", id, "plan.updated", None, false, json!({"id":id,"isActive":false,"updatedAt":timestamp})).await
                })
            }),
        )
        .await
    }
}

const UNIT_SELECT: &str = r#"
    SELECT unhex(replace(id,'-','')) AS id,name,abbreviation,unit_type AS type,description,is_active,
           NULL AS created_by,NULL AS updated_by,created_at,updated_at,deleted_at FROM units
"#;

#[derive(Clone)]
pub struct TenantUnitRepository {
    db: Arc<TenantDb>,
}

impl TenantUnitRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Unit>, AppError> {
        Ok(
            sqlx::query_as::<_, Unit>(&format!("{UNIT_SELECT} WHERE id=? AND deleted_at IS NULL"))
                .bind(id.to_string())
                .fetch_optional(&self.db.read_pool()?)
                .await?,
        )
    }

    pub async fn list(&self, filters: &UnitFilterDto) -> Result<PaginationResult<Unit>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let mut query =
            QueryBuilder::<Sqlite>::new(format!("{UNIT_SELECT} WHERE deleted_at IS NULL"));
        unit_filters(&mut query, filters);
        let direction = if filters.sort.as_deref() == Some("desc") {
            "DESC"
        } else {
            "ASC"
        };
        query.push(format!(" ORDER BY name {direction},id {direction} LIMIT "));
        query.push_bind(limit);
        query.push(" OFFSET ");
        query.push_bind((page - 1) * limit);
        let rows = query
            .build_query_as::<Unit>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM units WHERE deleted_at IS NULL");
        unit_filters(&mut count, filters);
        let total = count
            .build_query_scalar::<i64>()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total, page, limit))
    }

    pub async fn create(&self, dto: &CreateUnitDto, _: Option<Uuid>) -> Result<Unit, AppError> {
        let id = Uuid::now_v7();
        let timestamp = now()?;
        let name = dto.name.clone();
        let abbreviation = dto.abbreviation.clone();
        let unit_type = dto.r#type.clone().unwrap_or_else(|| "other".into());
        let description = dto.description.clone();
        let active = dto.is_active.unwrap_or(true);
        let db = self.db.clone();
        write(&db, Box::new(move |connection| Box::pin(async move {
            sqlx::query("INSERT INTO units(id,name,abbreviation,unit_type,description,is_active,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?)")
                .bind(id.to_string()).bind(&name).bind(abbreviation).bind(unit_type).bind(description)
                .bind(active).bind(&timestamp).bind(&timestamp).execute(&mut *connection).await?;
            event(connection,"unit",id,"unit.created",None,false,json!({"id":id,"name":name,"updatedAt":timestamp})).await
        }))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Created unit disappeared".into()))
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateUnitDto,
        _: Option<Uuid>,
    ) -> Result<Unit, AppError> {
        let timestamp = now()?;
        let name = dto.name.clone();
        let abbreviation = dto.abbreviation.clone();
        let unit_type = dto.r#type.clone();
        let description = dto.description.clone();
        let active = dto.is_active;
        let db = self.db.clone();
        write(&db, Box::new(move |connection| Box::pin(async move {
            let result=sqlx::query("UPDATE units SET name=COALESCE(?,name),abbreviation=COALESCE(?,abbreviation),unit_type=COALESCE(?,unit_type),description=COALESCE(?,description),is_active=COALESCE(?,is_active),updated_at=? WHERE id=? AND deleted_at IS NULL")
                .bind(name).bind(abbreviation).bind(unit_type).bind(description).bind(active).bind(&timestamp).bind(id.to_string()).execute(&mut *connection).await?;
            if result.rows_affected()==0{return Err(AppError::NotFound(format!("Unit with ID {id} not found")));}
            event(connection,"unit",id,"unit.updated",None,false,json!({"id":id,"updatedAt":timestamp})).await
        }))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Unit with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let timestamp = now()?;
        let db = self.db.clone();
        write(&db, Box::new(move |connection| Box::pin(async move {
            let result=sqlx::query("UPDATE units SET is_active=0,updated_at=? WHERE id=? AND deleted_at IS NULL")
                .bind(&timestamp).bind(id.to_string()).execute(&mut *connection).await?;
            if result.rows_affected()==0{return Err(AppError::NotFound(format!("Unit with ID {id} not found")));}
            event(connection,"unit",id,"unit.updated",None,false,json!({"id":id,"isActive":false,"updatedAt":timestamp})).await
        }))).await
    }

    pub async fn name_exists(
        &self,
        name: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        self.text_exists("name", name, exclude_id).await
    }

    pub async fn abbreviation_exists(
        &self,
        abbreviation: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        self.text_exists("abbreviation", abbreviation, exclude_id)
            .await
    }

    async fn text_exists(
        &self,
        column: &str,
        value: &str,
        exclude_id: Option<Uuid>,
    ) -> Result<bool, AppError> {
        let sql = format!(
            "SELECT EXISTS(SELECT 1 FROM units WHERE lower({column})=lower(?) \
             AND (? IS NULL OR id<>?) AND deleted_at IS NULL)"
        );
        Ok(sqlx::query_scalar(&sql)
            .bind(value)
            .bind(exclude_id.map(|id| id.to_string()))
            .bind(exclude_id.map(|id| id.to_string()))
            .fetch_one(&self.db.read_pool()?)
            .await?)
    }
}

fn unit_filters<'a>(query: &mut QueryBuilder<'a, Sqlite>, filters: &'a UnitFilterDto) {
    if let Some(name) = &filters.name {
        query.push(" AND instr(lower(name),lower(");
        query.push_bind(name);
        query.push("))>0");
    }
    if let Some(unit_type) = &filters.r#type {
        query.push(" AND unit_type=");
        query.push_bind(unit_type);
    }
    if let Some(active) = filters.is_active {
        query.push(" AND is_active=");
        query.push_bind(active);
    }
}

const GAME_SELECT: &str = r#"
    SELECT unhex(replace(id,'-','')) AS id,name,thumbnail_url,logo_url,video_url,launch_ref,
           is_active,sort_order,unhex(replace(created_by,'-','')) AS created_by,
           unhex(replace(updated_by,'-','')) AS updated_by,created_at,updated_at,deleted_at FROM games
"#;

#[derive(Clone)]
pub struct TenantGameRepository {
    db: Arc<TenantDb>,
}

impl TenantGameRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Game>, AppError> {
        Ok(
            sqlx::query_as::<_, Game>(&format!("{GAME_SELECT} WHERE id=? AND deleted_at IS NULL"))
                .bind(id.to_string())
                .fetch_optional(&self.db.read_pool()?)
                .await?,
        )
    }

    pub async fn list(&self, filters: &GameFilterDto) -> Result<PaginationResult<Game>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(50).clamp(1, 200);
        let mut query =
            QueryBuilder::<Sqlite>::new(format!("{GAME_SELECT} WHERE deleted_at IS NULL"));
        game_filters(&mut query, filters);
        let sort = match filters.sort_by.as_deref() {
            Some("name") => "name",
            Some("createdAt") => "created_at",
            _ => "sort_order",
        };
        let direction = if filters.sort_order.as_deref() == Some("DESC") {
            "DESC"
        } else {
            "ASC"
        };
        query
            .push(format!(
                " ORDER BY {sort} {direction},name ASC,id ASC LIMIT "
            ))
            .push_bind(limit);
        query.push(" OFFSET ").push_bind((page - 1) * limit);
        let rows = query
            .build_query_as::<Game>()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM games WHERE deleted_at IS NULL");
        game_filters(&mut count, filters);
        let total = count
            .build_query_scalar::<i64>()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total, page, limit))
    }

    pub async fn create(
        &self,
        dto: &CreateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        let id = Uuid::now_v7();
        let timestamp = now()?;
        let name = dto.name.clone();
        let thumbnail = dto.thumbnail_url.clone();
        let logo = dto.logo_url.clone();
        let video = dto.video_url.clone();
        let launch = dto.launch_ref.clone();
        let active = dto.is_active.unwrap_or(true);
        let order = dto.sort_order.unwrap_or(0);
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            sqlx::query("INSERT INTO games(id,name,thumbnail_url,logo_url,video_url,launch_ref,is_active,sort_order,created_by,updated_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)")
                .bind(id.to_string()).bind(&name).bind(thumbnail).bind(logo).bind(video).bind(launch).bind(active).bind(order)
                .bind(actor_id.map(|v|v.to_string())).bind(actor_id.map(|v|v.to_string())).bind(&timestamp).bind(&timestamp)
                .execute(&mut *connection).await?;
            event(connection,"game",id,"game.created",None,false,json!({"id":id,"name":name,"updatedAt":timestamp})).await
        }))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Created game disappeared".into()))
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateGameDto,
        actor_id: Option<Uuid>,
    ) -> Result<Game, AppError> {
        let timestamp = now()?;
        let name = dto.name.clone();
        let thumbnail = dto.thumbnail_url.clone();
        let logo = dto.logo_url.clone();
        let video = dto.video_url.clone();
        let launch = dto.launch_ref.clone();
        let active = dto.is_active;
        let order = dto.sort_order;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            let result=sqlx::query("UPDATE games SET name=COALESCE(?,name),thumbnail_url=COALESCE(?,thumbnail_url),logo_url=COALESCE(?,logo_url),video_url=COALESCE(?,video_url),launch_ref=COALESCE(?,launch_ref),is_active=COALESCE(?,is_active),sort_order=COALESCE(?,sort_order),updated_by=COALESCE(?,updated_by),updated_at=? WHERE id=? AND deleted_at IS NULL")
                .bind(name).bind(thumbnail).bind(logo).bind(video).bind(launch).bind(active).bind(order)
                .bind(actor_id.map(|v|v.to_string())).bind(&timestamp).bind(id.to_string()).execute(&mut *connection).await?;
            if result.rows_affected()==0{return Err(AppError::NotFound(format!("Game with ID {id} not found")));}
            event(connection,"game",id,"game.updated",None,false,json!({"id":id,"updatedAt":timestamp})).await
        }))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Game with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let timestamp = now()?;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            let result=sqlx::query("UPDATE games SET deleted_at=?,updated_at=? WHERE id=? AND deleted_at IS NULL")
                .bind(&timestamp).bind(&timestamp).bind(id.to_string()).execute(&mut *connection).await?;
            if result.rows_affected()==0{return Err(AppError::NotFound(format!("Game with ID {id} not found")));}
            event(connection,"game",id,"game.deleted",None,true,json!({"id":id,"deletedAt":timestamp})).await
        }))).await
    }
}

fn game_filters<'a>(query: &mut QueryBuilder<'a, Sqlite>, filters: &'a GameFilterDto) {
    if let Some(active) = filters.is_active {
        query.push(" AND is_active=").push_bind(active);
    }
    if let Some(name) = &filters.name {
        query
            .push(" AND instr(lower(name),lower(")
            .push_bind(name)
            .push("))>0");
    }
}

#[derive(Clone)]
pub struct TenantProductRecipeRepository {
    db: Arc<TenantDb>,
}

impl TenantProductRecipeRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }

    pub async fn get(&self, product_id: Uuid) -> Result<ProductRecipe, AppError> {
        let pool = self.db.read_pool()?;
        let items:Vec<(String,i32)>=sqlx::query_as("SELECT ingredient_id,quantity FROM product_recipe_items WHERE product_id=? ORDER BY ingredient_id").bind(product_id.to_string()).fetch_all(&pool).await?;
        let groups:Vec<(String,String,bool,bool)>=sqlx::query_as("SELECT id,name,required,multiple FROM product_option_groups WHERE product_id=? ORDER BY sort_order,id").bind(product_id.to_string()).fetch_all(&pool).await?;
        let options:Vec<(String,String,String,i64)>=sqlx::query_as("SELECT o.id,o.group_id,o.name,o.price_delta FROM product_options o JOIN product_option_groups g ON g.id=o.group_id WHERE g.product_id=? ORDER BY o.sort_order,o.id").bind(product_id.to_string()).fetch_all(&pool).await?;
        let ingredients:Vec<(String,String,i32)>=sqlx::query_as("SELECT oi.option_id,oi.ingredient_id,oi.quantity FROM product_option_ingredients oi JOIN product_options o ON o.id=oi.option_id JOIN product_option_groups g ON g.id=o.group_id WHERE g.product_id=? ORDER BY oi.ingredient_id").bind(product_id.to_string()).fetch_all(&pool).await?;
        let mut by_option: HashMap<Uuid, Vec<RecipeIngredient>> = HashMap::new();
        for (option, ingredient, quantity) in ingredients {
            by_option
                .entry(Uuid::parse_str(&option).map_err(|e| AppError::Internal(e.to_string()))?)
                .or_default()
                .push(RecipeIngredient {
                    ingredient_id: Uuid::parse_str(&ingredient)
                        .map_err(|e| AppError::Internal(e.to_string()))?,
                    quantity,
                });
        }
        let mut by_group: HashMap<Uuid, Vec<ProductOption>> = HashMap::new();
        for (id, group, name, delta) in options {
            let id = Uuid::parse_str(&id).map_err(|e| AppError::Internal(e.to_string()))?;
            by_group
                .entry(Uuid::parse_str(&group).map_err(|e| AppError::Internal(e.to_string()))?)
                .or_default()
                .push(ProductOption {
                    id: Some(id),
                    name,
                    price_delta: money_f64(delta)?,
                    ingredients: by_option.remove(&id).unwrap_or_default(),
                });
        }
        Ok(ProductRecipe {
            items: items
                .into_iter()
                .map(|(id, quantity)| {
                    Ok(RecipeIngredient {
                        ingredient_id: Uuid::parse_str(&id)
                            .map_err(|e| AppError::Internal(e.to_string()))?,
                        quantity,
                    })
                })
                .collect::<Result<_, AppError>>()?,
            option_groups: groups
                .into_iter()
                .map(|(id, name, required, multiple)| {
                    let id = Uuid::parse_str(&id).map_err(|e| AppError::Internal(e.to_string()))?;
                    Ok(ProductOptionGroup {
                        id: Some(id),
                        name,
                        required,
                        multiple,
                        options: by_group.remove(&id).unwrap_or_default(),
                    })
                })
                .collect::<Result<_, AppError>>()?,
        })
    }

    pub async fn replace(&self, product_id: Uuid, recipe: &ProductRecipe) -> Result<(), AppError> {
        let recipe = recipe.clone();
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            sqlx::query("DELETE FROM product_recipe_items WHERE product_id=?").bind(product_id.to_string()).execute(&mut *connection).await?;
            sqlx::query("DELETE FROM product_option_groups WHERE product_id=?").bind(product_id.to_string()).execute(&mut *connection).await?;
            for item in &recipe.items{sqlx::query("INSERT INTO product_recipe_items(product_id,ingredient_id,quantity) VALUES (?,?,?)").bind(product_id.to_string()).bind(item.ingredient_id.to_string()).bind(item.quantity).execute(&mut *connection).await?;}
            for(group_order,group)in recipe.option_groups.iter().enumerate(){
                let group_id=group.id.ok_or_else(||AppError::Internal("Option group ID was not assigned".into()))?;
                sqlx::query("INSERT INTO product_option_groups(id,product_id,name,required,multiple,sort_order) VALUES (?,?,?,?,?,?)").bind(group_id.to_string()).bind(product_id.to_string()).bind(group.name.trim()).bind(group.required).bind(group.multiple).bind(group_order as i32).execute(&mut *connection).await?;
                for(option_order,option)in group.options.iter().enumerate(){let option_id=option.id.ok_or_else(||AppError::Internal("Option ID was not assigned".into()))?;
                    sqlx::query("INSERT INTO product_options(id,group_id,name,price_delta,sort_order) VALUES (?,?,?,?,?)").bind(option_id.to_string()).bind(group_id.to_string()).bind(option.name.trim()).bind(money(option.price_delta)?).bind(option_order as i32).execute(&mut *connection).await?;
                    for ingredient in &option.ingredients{sqlx::query("INSERT INTO product_option_ingredients(option_id,ingredient_id,quantity) VALUES (?,?,?)").bind(option_id.to_string()).bind(ingredient.ingredient_id.to_string()).bind(ingredient.quantity).execute(&mut *connection).await?;}
                }
            }
            event(connection,"product",product_id,"product.recipe.replaced",None,false,json!({"id":product_id,"recipe":recipe})).await
        }))).await
    }

    pub async fn ingredient_info(
        &self,
        ids: &[Uuid],
    ) -> Result<Vec<IngredientInfo>, AppError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut query=QueryBuilder::<Sqlite>::new("SELECT unhex(replace(p.id,'-','')) AS id,p.name,p.is_raw_material,EXISTS(SELECT 1 FROM product_recipe_items r WHERE r.product_id=p.id) AS has_recipe FROM products p WHERE p.id IN (");
        let mut separated = query.separated(",");
        for id in ids {
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(") AND p.deleted_at IS NULL ORDER BY p.id");
        Ok(query
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?)
    }

    pub async fn is_used_as_ingredient(&self, id: Uuid) -> Result<bool, AppError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM product_recipe_items WHERE ingredient_id=?) OR EXISTS(SELECT 1 FROM product_option_ingredients WHERE ingredient_id=?)").bind(id.to_string()).bind(id.to_string()).fetch_one(&self.db.read_pool()?).await?)
    }
    pub async fn owned_ids(&self, id: Uuid) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar("SELECT unhex(replace(id,'-','')) FROM product_option_groups WHERE product_id=? UNION ALL SELECT unhex(replace(o.id,'-','')) FROM product_options o JOIN product_option_groups g ON g.id=o.group_id WHERE g.product_id=?").bind(id.to_string()).bind(id.to_string()).fetch_all(&self.db.read_pool()?).await?)
    }
    pub async fn with_option_groups(&self) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar(
            "SELECT DISTINCT unhex(replace(product_id,'-','')) FROM product_option_groups ORDER BY product_id",
        )
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
    pub async fn with_required_options(&self) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar("SELECT DISTINCT unhex(replace(product_id,'-','')) FROM product_option_groups WHERE required=1 ORDER BY product_id").fetch_all(&self.db.read_pool()?).await?)
    }

    /// Reads ingredient stock while the caller holds the tenant's immediate writer transaction.
    pub async fn stock_for_update(
        connection: &mut SqliteConnection,
        venue_location_id: Uuid,
        ingredient_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, (String, i32)>, AppError> {
        if ingredient_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT unhex(replace(p.id,'-','')) AS id,p.name,
                    COALESCE(ls.quantity_pieces,0)
             FROM products p
             LEFT JOIN location_stock ls
               ON ls.product_id=p.id
              AND ls.inventory_location_id=(
                    SELECT id FROM inventory_locations
                    WHERE venue_location_id=",
        );
        query.push_bind(venue_location_id.to_string());
        query.push(
            " AND kind='store' AND is_active=1 AND deleted_at IS NULL
                    ORDER BY id LIMIT 1)
             WHERE p.id IN (",
        );
        let mut separated = query.separated(",");
        for id in ingredient_ids {
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(") ORDER BY p.id");
        let rows: Vec<(Uuid, String, i32)> = query.build_query_as().fetch_all(connection).await?;
        Ok(rows
            .into_iter()
            .map(|(id, name, quantity)| (id, (name, quantity)))
            .collect())
    }

    pub async fn made_to_order_stock(
        &self,
        inventory_location_id: Option<Uuid>,
        venue_location_id: Uuid,
    ) -> Result<Vec<(Uuid, i32, i32)>, AppError> {
        Ok(sqlx::query_as(
            "SELECT unhex(replace(r.product_id,'-','')),r.quantity,
                    COALESCE(ls.quantity_pieces,0)
             FROM product_recipe_items r
             LEFT JOIN location_stock ls
               ON ls.product_id=r.ingredient_id
              AND ls.inventory_location_id=COALESCE(?,(
                    SELECT id FROM inventory_locations
                    WHERE venue_location_id=?
                      AND kind='store' AND is_active=1 AND deleted_at IS NULL
                    ORDER BY id LIMIT 1))
             ORDER BY r.product_id,r.ingredient_id",
        )
        .bind(inventory_location_id.map(|id| id.to_string()))
        .bind(venue_location_id.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }

    pub async fn recipe_requirements(&self) -> Result<Vec<(Uuid, Uuid, i32)>, AppError> {
        Ok(sqlx::query_as(
            "SELECT unhex(replace(product_id,'-','')),
                    unhex(replace(ingredient_id,'-','')),
                    quantity
             FROM product_recipe_items
             ORDER BY product_id,ingredient_id",
        )
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
}

#[derive(Debug, FromRow)]
struct PricingSetRow {
    id: Uuid,
    name: String,
    description: Option<String>,
    active_version_id: Option<Uuid>,
    created_by: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct TenantPricingPolicyRepository {
    db: Arc<TenantDb>,
}

impl TenantPricingPolicyRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    async fn map_set(&self, row: PricingSetRow) -> Result<PricingRuleSet, AppError> {
        let locations:Vec<Uuid>=sqlx::query_scalar("SELECT unhex(replace(location_id,'-','')) FROM pricing_rule_set_locations WHERE rule_set_id=? ORDER BY location_id").bind(row.id.to_string()).fetch_all(&self.db.read_pool()?).await?;
        Ok(PricingRuleSet {
            location_id: locations.first().copied(),
            location_ids: locations,
            id: row.id,
            organization_id: self.db.tenant_id(),
            name: row.name,
            description: row.description,
            active_version_id: row.active_version_id,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
    pub async fn list_sets(
        &self,
        _: Uuid,
        location: Option<Uuid>,
    ) -> Result<Vec<PricingRuleSet>, AppError> {
        let rows=sqlx::query_as::<_,PricingSetRow>("SELECT unhex(replace(id,'-','')) AS id,name,description,unhex(replace(active_version_id,'-','')) AS active_version_id,unhex(replace(created_by,'-','')) AS created_by,created_at,updated_at FROM pricing_rule_sets s WHERE ? IS NULL OR availability_scope='ALL' OR EXISTS(SELECT 1 FROM pricing_rule_set_locations l WHERE l.rule_set_id=s.id AND l.location_id=?) ORDER BY name,id").bind(location.map(|v|v.to_string())).bind(location.map(|v|v.to_string())).fetch_all(&self.db.read_pool()?).await?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            result.push(self.map_set(row).await?);
        }
        Ok(result)
    }
    pub async fn active_policies(
        &self,
        _: Uuid,
        location: Option<Uuid>,
    ) -> Result<Vec<Value>, AppError> {
        Ok(sqlx::query_scalar("SELECT v.policy FROM pricing_rule_sets s JOIN pricing_rule_versions v ON v.id=s.active_version_id WHERE v.status='published' AND (s.availability_scope='ALL' OR EXISTS(SELECT 1 FROM pricing_rule_set_locations l WHERE l.rule_set_id=s.id AND l.location_id=?)) ORDER BY s.id").bind(location.map(|v|v.to_string())).fetch_all(&self.db.read_pool()?).await?)
    }
    pub async fn get_set(&self, _: Uuid, id: Uuid) -> Result<PricingRuleSet, AppError> {
        let row=sqlx::query_as::<_,PricingSetRow>("SELECT unhex(replace(id,'-','')) AS id,name,description,unhex(replace(active_version_id,'-','')) AS active_version_id,unhex(replace(created_by,'-','')) AS created_by,created_at,updated_at FROM pricing_rule_sets WHERE id=?").bind(id.to_string()).fetch_optional(&self.db.read_pool()?).await?.ok_or_else(||AppError::NotFound(format!("Pricing rule set {id} not found")))?;
        self.map_set(row).await
    }
    pub async fn create_set(
        &self,
        _: Uuid,
        location: Option<Uuid>,
        locations: &[Uuid],
        name: &str,
        description: Option<&str>,
        policy: &Value,
        actor: Uuid,
    ) -> Result<(PricingRuleSet, PricingRuleVersion), AppError> {
        let id = Uuid::now_v7();
        let version_id = Uuid::now_v7();
        let timestamp = now()?;
        let name = name.to_owned();
        let description = description.map(str::to_owned);
        let policy =
            serde_json::to_string(policy).map_err(|e| AppError::BadRequest(e.to_string()))?;
        let mut locations = locations.to_vec();
        if locations.is_empty() {
            locations.extend(location);
        }
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            sqlx::query("INSERT INTO pricing_rule_sets(id,name,description,availability_scope,created_by,created_at,updated_at) VALUES (?,?,?,?,?,?,?)").bind(id.to_string()).bind(&name).bind(description).bind(if locations.is_empty(){"ALL"}else{"SELECTED"}).bind(actor.to_string()).bind(&timestamp).bind(&timestamp).execute(&mut *connection).await?;
            for location in locations{sqlx::query("INSERT INTO pricing_rule_set_locations(rule_set_id,location_id) VALUES (?,?)").bind(id.to_string()).bind(location.to_string()).execute(&mut *connection).await?;}
            sqlx::query("INSERT INTO pricing_rule_versions(id,rule_set_id,version,status,policy,created_by,created_at) VALUES (?,?,1,'draft',?,?,?)").bind(version_id.to_string()).bind(id.to_string()).bind(policy).bind(actor.to_string()).bind(&timestamp).execute(&mut *connection).await?;
            event(connection,"pricing_rule_set",id,"pricing.rule_set.created",None,false,json!({"id":id,"versionId":version_id,"updatedAt":timestamp})).await
        }))).await?;
        Ok((
            self.get_set(self.db.tenant_id(), id).await?,
            self.get_version(self.db.tenant_id(), id, version_id)
                .await?,
        ))
    }
    pub async fn create_version(
        &self,
        _: Uuid,
        set: Uuid,
        policy: &Value,
        actor: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let id = Uuid::now_v7();
        let timestamp = now()?;
        let policy =
            serde_json::to_string(policy).map_err(|e| AppError::BadRequest(e.to_string()))?;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pricing_rule_sets WHERE id=?)").bind(set.to_string()).fetch_one(&mut *connection).await?;if !exists{return Err(AppError::NotFound(format!("Pricing rule set {set} not found")));}sqlx::query("INSERT INTO pricing_rule_versions(id,rule_set_id,version,status,policy,created_by,created_at) SELECT ?,?,COALESCE(MAX(version),0)+1,'draft',?,?,? FROM pricing_rule_versions WHERE rule_set_id=?").bind(id.to_string()).bind(set.to_string()).bind(policy).bind(actor.to_string()).bind(timestamp).bind(set.to_string()).execute(&mut *connection).await?;Ok(())}))).await?;
        self.get_version(self.db.tenant_id(), set, id).await
    }
    pub async fn versions(
        &self,
        org: Uuid,
        set: Uuid,
    ) -> Result<Vec<PricingRuleVersion>, AppError> {
        self.get_set(org, set).await?;
        Ok(sqlx::query_as(&format!(
            "{VERSION_SELECT} WHERE rule_set_id=? ORDER BY version DESC"
        ))
        .bind(set.to_string())
        .fetch_all(&self.db.read_pool()?)
        .await?)
    }
    pub async fn get_version(
        &self,
        _: Uuid,
        set: Uuid,
        id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        sqlx::query_as(&format!("{VERSION_SELECT} WHERE id=? AND rule_set_id=?"))
            .bind(id.to_string())
            .bind(set.to_string())
            .fetch_optional(&self.db.read_pool()?)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Pricing rule version {id} not found")))
    }
    pub async fn mark_validated(
        &self,
        _: Uuid,
        set: Uuid,
        id: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let timestamp = now()?;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{let result=sqlx::query("UPDATE pricing_rule_versions SET status='validated',validated_at=? WHERE id=? AND rule_set_id=? AND status IN ('draft','validated')").bind(timestamp).bind(id.to_string()).bind(set.to_string()).execute(&mut *connection).await?;if result.rows_affected()==0{return Err(AppError::Conflict("Only draft or validated versions can be validated".into()));}Ok(())}))).await?;
        self.get_version(self.db.tenant_id(), set, id).await
    }
    pub async fn record_simulation(
        &self,
        _: Uuid,
        set: Uuid,
        id: Uuid,
        hash: &str,
    ) -> Result<(), AppError> {
        let hash = hash.to_owned();
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{sqlx::query("UPDATE pricing_rule_versions SET simulation_hash=? WHERE id=? AND rule_set_id=? AND status IN ('draft','validated')").bind(hash).bind(id.to_string()).bind(set.to_string()).execute(&mut *connection).await?;Ok(())}))).await
    }
    pub async fn publish(
        &self,
        _: Uuid,
        set: Uuid,
        id: Uuid,
        effective: DateTime<Utc>,
        actor: Uuid,
    ) -> Result<PricingRuleVersion, AppError> {
        let effective_text =
            format_sqlite_timestamp(&effective).map_err(|e| AppError::BadRequest(e.to_string()))?;
        let timestamp = now()?;
        let scheduled = effective > Utc::now();
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{let state:Option<(String,Option<String>)>=sqlx::query_as("SELECT status,simulation_hash FROM pricing_rule_versions WHERE id=? AND rule_set_id=?").bind(id.to_string()).bind(set.to_string()).fetch_optional(&mut *connection).await?;if !state.is_some_and(|(status,hash)|status=="validated"&&hash.is_some()){return Err(AppError::Conflict("A pricing version must be validated and simulated before publication".into()));}if !scheduled{sqlx::query("UPDATE pricing_rule_versions SET status='superseded' WHERE rule_set_id=? AND status='published'").bind(set.to_string()).execute(&mut *connection).await?;}sqlx::query("UPDATE pricing_rule_versions SET status=?,effective_at=?,published_at=?,published_by=? WHERE id=?").bind(if scheduled{"scheduled"}else{"published"}).bind(effective_text).bind((!scheduled).then_some(timestamp.clone())).bind(actor.to_string()).bind(id.to_string()).execute(&mut *connection).await?;if !scheduled{sqlx::query("UPDATE pricing_rule_sets SET active_version_id=?,updated_at=? WHERE id=?").bind(id.to_string()).bind(&timestamp).bind(set.to_string()).execute(&mut *connection).await?;}event(connection,"pricing_rule_set",set,"pricing.rules.changed",None,false,json!({"ruleSetId":set,"versionId":id,"status":if scheduled{"scheduled"}else{"published"},"effectiveAt":effective,"updatedAt":timestamp})).await}))).await?;
        self.get_version(self.db.tenant_id(), set, id).await
    }
    pub async fn activate_due(&self) -> Result<Vec<(Uuid, Uuid, Uuid, i32)>, AppError> {
        let timestamp = now()?;
        let due:Vec<(Uuid,Uuid,i32)>=sqlx::query_as("SELECT unhex(replace(id,'-','')),unhex(replace(rule_set_id,'-','')),version FROM pricing_rule_versions WHERE status='scheduled' AND effective_at<=? ORDER BY effective_at,version,id LIMIT 100").bind(&timestamp).fetch_all(&self.db.background_read_pool()?).await?;
        let mut activated = Vec::new();
        for (version, set, number) in due {
            let event_time = timestamp.clone();
            let db = self.db.clone();
            let changed=write(&db,Box::new(move|connection|Box::pin(async move{let scheduled:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pricing_rule_versions WHERE id=? AND status='scheduled')").bind(version.to_string()).fetch_one(&mut *connection).await?;if !scheduled{return Ok(false);}sqlx::query("UPDATE pricing_rule_versions SET status='superseded' WHERE rule_set_id=? AND status='published'").bind(set.to_string()).execute(&mut *connection).await?;sqlx::query("UPDATE pricing_rule_versions SET status='published',published_at=? WHERE id=?").bind(&event_time).bind(version.to_string()).execute(&mut *connection).await?;sqlx::query("UPDATE pricing_rule_sets SET active_version_id=?,updated_at=? WHERE id=?").bind(version.to_string()).bind(&event_time).bind(set.to_string()).execute(&mut *connection).await?;event(connection,"pricing_rule_set",set,"pricing.rules.changed",None,false,json!({"ruleSetId":set,"versionId":version,"version":number,"status":"published","activatedAt":event_time})).await?;Ok(true)}))).await?;
            if changed {
                activated.push((self.db.tenant_id(), set, version, number));
            }
        }
        Ok(activated)
    }
}

const VERSION_SELECT:&str="SELECT unhex(replace(id,'-','')) AS id,unhex(replace(rule_set_id,'-','')) AS rule_set_id,version,status,policy,simulation_hash,validated_at,effective_at,published_at,unhex(replace(created_by,'-','')) AS created_by,unhex(replace(published_by,'-','')) AS published_by,created_at FROM pricing_rule_versions";

#[derive(Clone)]
pub struct TenantSettingsRepository {
    pub(super) db: Arc<TenantDb>,
}
impl TenantSettingsRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    pub async fn list_overrides(
        &self,
        organization: Uuid,
        location: Option<Uuid>,
    ) -> Result<Vec<SettingOverride>, AppError> {
        self.require_tenant(organization)?;
        Ok(sqlx::query_as(&format!("{SETTING_SELECT} WHERE location_id IS NULL OR location_id=? ORDER BY key,location_id IS NOT NULL,location_id")).bind(self.db.tenant_id()).bind(location.map(|v|v.to_string())).fetch_all(&self.db.read_pool()?).await?)
    }
    pub async fn find_override(
        &self,
        organization: Uuid,
        location: Option<Uuid>,
        key: &str,
    ) -> Result<Option<SettingOverride>, AppError> {
        self.require_tenant(organization)?;
        Ok(sqlx::query_as(&format!(
            "{SETTING_SELECT} WHERE location_id IS ? AND key=?"
        ))
        .bind(self.db.tenant_id())
        .bind(location.map(|v| v.to_string()))
        .bind(key)
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }
    pub async fn upsert_override(
        &self, organization: Uuid, key: &str, dto: &UpsertSettingOverrideDto,
        actor: Uuid, request: Option<&str>, _: bool,
    ) -> Result<SettingOverride, AppError> {
        self.require_tenant(organization)?;
        validate_setting_write(key, &dto.reason, request)?;
        let tenant=self.db.tenant_id(); let key=key.to_owned(); let location=dto.location_id;
        let value=dto.value.to_string(); let reason=dto.reason.trim().to_owned();
        let expected=dto.expected_revision; let request=request.map(str::to_owned);
        write(&self.db,Box::new(move|c|Box::pin(async move{
            validate_setting_location(c,location).await?;
            let existing:Option<(String,String,i64)>=sqlx::query_as("SELECT id,value,revision FROM setting_overrides WHERE location_id IS ? AND key=?")
                .bind(location.map(|v|v.to_string())).bind(&key).fetch_optional(&mut *c).await?;
            let actual=existing.as_ref().map_or(0,|r|r.2);
            if expected.is_some_and(|v|v!=actual){return Err(AppError::conflict_code("SETTING_REVISION_CONFLICT",Some(json!({"expectedRevision":expected,"actualRevision":actual}))));}
            let history:i64=sqlx::query_scalar("SELECT COALESCE(MAX(revision),0) FROM setting_revisions WHERE location_id IS ? AND key=?")
                .bind(location.map(|v|v.to_string())).bind(&key).fetch_one(&mut *c).await?;
            let revision=actual.max(history).checked_add(1).ok_or_else(||AppError::Conflict("Setting revision is exhausted".into()))?;
            let id=existing.as_ref().map(|r|r.0.clone()).unwrap_or_else(||Uuid::now_v7().to_string());let ts=now()?;
            sqlx::query("INSERT INTO setting_overrides(id,location_id,key,value,revision,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET value=excluded.value,revision=excluded.revision,updated_by=excluded.updated_by,updated_at=excluded.updated_at")
                .bind(&id).bind(location.map(|v|v.to_string())).bind(&key).bind(&value).bind(revision).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await?;
            sqlx::query("INSERT INTO setting_revisions(location_id,key,revision,operation,old_value,new_value,reason,actor_user_id,request_id,created_at) VALUES(?,?,?,?,?,?,?,?,?,?)")
                .bind(location.map(|v|v.to_string())).bind(&key).bind(revision).bind(if existing.is_some(){"update"}else{"create"}).bind(existing.as_ref().map(|r|r.1.as_str())).bind(&value).bind(reason).bind(actor.to_string()).bind(request).bind(&ts).execute(&mut *c).await?;
            if location.is_none(){
                sqlx::query("INSERT INTO configurations(id,key,value,category,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_by=excluded.updated_by,updated_at=excluded.updated_at")
                    .bind(Uuid::now_v7().to_string()).bind(&key).bind(&value).bind(key.split('.').next().unwrap_or("general")).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await?;
            }
            event(c,"setting_override",Uuid::parse_str(&id).map_err(|e|AppError::Internal(e.to_string()))?,"configuration.changed",location,false,json!({"organizationId":tenant,"locationId":location,"key":key,"revision":revision,"updatedAt":ts})).await?;
            Ok(sqlx::query_as(&format!("{SETTING_SELECT} WHERE id=?")).bind(tenant).bind(id).fetch_one(c).await?)
        }))).await
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn delete_override(
        &self, organization:Uuid, location:Option<Uuid>, key:&str, expected:Option<i64>,
        reason:&str, actor:Uuid, request:Option<&str>, mirror:Option<&Value>,
    )->Result<bool,AppError>{
        self.require_tenant(organization)?;validate_setting_write(key,reason,request)?;
        let tenant=self.db.tenant_id();let key=key.to_owned();let reason=reason.trim().to_owned();let request=request.map(str::to_owned);let mirror=mirror.cloned();
        write(&self.db,Box::new(move|c|Box::pin(async move{
            let existing:Option<(String,String,i64)>=sqlx::query_as("SELECT id,value,revision FROM setting_overrides WHERE location_id IS ? AND key=?")
                .bind(location.map(|v|v.to_string())).bind(&key).fetch_optional(&mut *c).await?;
            let Some((id,value,actual))=existing else{return Ok(false)};
            if expected.is_some_and(|v|v!=actual){return Err(AppError::conflict_code("SETTING_REVISION_CONFLICT",Some(json!({"expectedRevision":expected,"actualRevision":actual}))));}
            let revision=actual.checked_add(1).ok_or_else(||AppError::Conflict("Setting revision is exhausted".into()))?;let ts=now()?;
            sqlx::query("DELETE FROM setting_overrides WHERE id=?").bind(&id).execute(&mut *c).await?;
            sqlx::query("INSERT INTO setting_revisions(location_id,key,revision,operation,old_value,reason,actor_user_id,request_id,created_at) VALUES(?,?,?,'delete',?,?,?,?,?)")
                .bind(location.map(|v|v.to_string())).bind(&key).bind(revision).bind(&value).bind(reason).bind(actor.to_string()).bind(request).bind(&ts).execute(&mut *c).await?;
            if location.is_none(){
                if let Some(value)=mirror{
                    sqlx::query("INSERT INTO configurations(id,key,value,category,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_by=excluded.updated_by,updated_at=excluded.updated_at")
                        .bind(Uuid::now_v7().to_string()).bind(&key).bind(value.to_string()).bind(key.split('.').next().unwrap_or("general")).bind(actor.to_string()).bind(actor.to_string()).bind(&ts).bind(&ts).execute(&mut *c).await?;
                }else{sqlx::query("DELETE FROM configurations WHERE key=?").bind(&key).execute(&mut *c).await?;}
            }
            event(c,"setting_override",Uuid::parse_str(&id).map_err(|e|AppError::Internal(e.to_string()))?,"configuration.changed",location,true,json!({"organizationId":tenant,"locationId":location,"key":key,"revision":revision,"deleted":true,"updatedAt":ts})).await?;Ok(true)
        }))).await
    }
    pub(super) fn require_tenant(&self,id:Uuid)->Result<(),AppError>{if id!=self.db.tenant_id(){Err(AppError::Forbidden("Tenant does not match the database".into()))}else{Ok(())}}
    pub async fn history(
        &self,
        organization: Uuid,
        query: &SettingHistoryQuery,
    ) -> Result<Vec<SettingRevision>, AppError> {
        self.require_tenant(organization)?;
        let mut builder = QueryBuilder::<Sqlite>::new("SELECT id,");
        builder.push_bind(self.db.tenant_id()).push(
            " AS organization_id,
             unhex(replace(location_id,'-','')) AS location_id,
             key,revision,operation,old_value,new_value,reason,
             unhex(replace(actor_user_id,'-','')) AS actor_user_id,
             request_id,created_at
             FROM setting_revisions WHERE location_id IS ",
        );
        builder.push_bind(query.location_id.map(|v| v.to_string()));
        if let Some(key) = &query.key {
            builder.push(" AND key=").push_bind(key);
        }
        builder
            .push(" ORDER BY created_at DESC,id DESC LIMIT ")
            .push_bind(query.limit.unwrap_or(100).clamp(1, 500));
        Ok(builder
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?)
    }
    pub async fn snapshot_values(&self,organization:Uuid,location:Option<Uuid>)->Result<(Vec<SettingOverride>,i64),AppError>{
        self.require_tenant(organization)?;
        let pool=self.db.read_pool()?;let mut tx=pool.begin().await?;
        if let Some(id)=location{let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM venue_locations WHERE id=? AND is_active=1)").bind(id.to_string()).fetch_one(&mut *tx).await?;if !valid{return Err(AppError::NotFound("Active location not found".into()));}}
        let overrides=sqlx::query_as(&format!("{SETTING_SELECT} WHERE location_id IS NULL OR location_id=? ORDER BY key,location_id IS NOT NULL,location_id")).bind(organization).bind(location.map(|v|v.to_string())).fetch_all(&mut *tx).await?;
        let revision=sqlx::query_scalar("SELECT COALESCE(MAX(id),0) FROM setting_revisions").fetch_one(&mut *tx).await?;
        Ok((overrides,revision))
    }
    pub async fn latest_revision(&self, organization: Uuid) -> Result<i64, AppError> {
        self.require_tenant(organization)?;
        Ok(
            sqlx::query_scalar("SELECT COALESCE(MAX(id),0) FROM setting_revisions")
                .fetch_one(&self.db.read_pool()?)
                .await?,
        )
    }
}
const SETTING_SELECT:&str="SELECT unhex(replace(id,'-','')) AS id,? AS organization_id,unhex(replace(location_id,'-','')) AS location_id,key,value,revision,unhex(replace(created_by,'-','')) AS created_by,unhex(replace(updated_by,'-','')) AS updated_by,created_at,updated_at FROM setting_overrides";

fn validate_setting_write(key:&str,reason:&str,request:Option<&str>)->Result<(),AppError>{
    if key.trim().is_empty() || key.chars().count()>120 || reason.trim().is_empty() || request.is_some_and(|v|v.chars().count()>128){return Err(AppError::BadRequest("Provide a setting key, reason, and valid request ID".into()));}Ok(())
}
async fn validate_setting_location(c:&mut SqliteConnection,location:Option<Uuid>)->Result<(),AppError>{
    if let Some(id)=location{let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM venue_locations WHERE id=? AND is_active=1)").bind(id.to_string()).fetch_one(c).await?;if !exists{return Err(AppError::NotFound("Active location not found".into()));}}Ok(())
}
