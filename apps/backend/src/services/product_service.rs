use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::error::AppError;
use crate::models::{CreateProductDto, Product, ProductFilterDto, UpdateProductDto};
use crate::repositories::ProductRepository;
use crate::validation::{optional_product_category, require_product_category};

pub struct ProductService {
    repo: ProductRepository,
    cache: Arc<dyn CacheService>,
}

impl ProductService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: ProductRepository::new(pool),
            cache,
        }
    }

    async fn invalidate_products(&self, id: Option<Uuid>) -> Result<(), AppError> {
        let mut cache_keys = Vec::new();
        if let Some(id) = id {
            cache_keys.push(keys::product(&id));
        }
        if !cache_keys.is_empty() {
            cache::invalidate(&*self.cache, &cache_keys).await?;
        }
        self.cache.invalidate_prefix("products:list:").await
    }

    pub async fn list(
        &self,
        filters: ProductFilterDto,
    ) -> Result<crate::dto::PaginationResult<Product>, AppError> {
        let cache_key = keys::products_list(&keys::filter_hash(&filters));
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo.list(&filters).await
        })
        .await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Product, AppError> {
        let cache_key = keys::product(&id);
        get_or_set(&*self.cache, &cache_key, keys::ttl::LOOKUP, || async {
            self.repo
                .find_by_id(id)
                .await?
                .ok_or_else(|| AppError::NotFound(format!("Product with ID {id} not found")))
        })
        .await
    }

    pub async fn create(
        &self,
        dto: CreateProductDto,
        actor_id: Option<Uuid>,
    ) -> Result<Product, AppError> {
        if dto.price < 0.0 {
            return Err(AppError::BadRequest(
                "Price must be greater than or equal to 0".to_string(),
            ));
        }

        if let Some(day_price) = dto.day_price {
            if day_price < 0.0 {
                return Err(AppError::BadRequest(
                    "dayPrice must be greater than or equal to 0".to_string(),
                ));
            }
        }
        if let Some(night_price) = dto.night_price {
            if night_price < 0.0 {
                return Err(AppError::BadRequest(
                    "nightPrice must be greater than or equal to 0".to_string(),
                ));
            }
        }
        if let Some(units) = dto.units_per_purchase_unit {
            if units <= 0 {
                return Err(AppError::BadRequest(
                    "unitsPerPurchaseUnit must be positive".to_string(),
                ));
            }
        }

        if let Some(sku) = &dto.sku {
            if self.repo.sku_exists(sku, None).await? {
                return Err(AppError::Conflict(format!(
                    "Product with SKU '{sku}' already exists"
                )));
            }
        }

        if self.repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Product with name '{}' already exists",
                dto.name
            )));
        }

        if let Some(stock) = dto.stock_quantity {
            if stock < 0 {
                return Err(AppError::BadRequest(
                    "Stock quantity must be greater than or equal to 0".to_string(),
                ));
            }
        }

        let mut dto = dto;
        dto.category = Some(require_product_category(dto.category)?);
        let product = self.repo.create(&dto, actor_id).await?;
        self.invalidate_products(Some(product.id)).await?;
        Ok(product)
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateProductDto,
        actor_id: Option<Uuid>,
    ) -> Result<Product, AppError> {
        if let Some(price) = dto.price {
            if price < 0.0 {
                return Err(AppError::BadRequest(
                    "Price must be greater than or equal to 0".to_string(),
                ));
            }
        }
        if let Some(day_price) = dto.day_price {
            if day_price < 0.0 {
                return Err(AppError::BadRequest(
                    "dayPrice must be greater than or equal to 0".to_string(),
                ));
            }
        }
        if let Some(night_price) = dto.night_price {
            if night_price < 0.0 {
                return Err(AppError::BadRequest(
                    "nightPrice must be greater than or equal to 0".to_string(),
                ));
            }
        }
        if let Some(units) = dto.units_per_purchase_unit {
            if units <= 0 {
                return Err(AppError::BadRequest(
                    "unitsPerPurchaseUnit must be positive".to_string(),
                ));
            }
        }

        if let Some(sku) = &dto.sku {
            if self.repo.sku_exists(sku, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Product with SKU '{sku}' already exists"
                )));
            }
        }

        if let Some(name) = &dto.name {
            if self.repo.name_exists(name, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Product with name '{name}' already exists"
                )));
            }
        }

        if let Some(stock) = dto.stock_quantity {
            if stock < 0 {
                return Err(AppError::BadRequest(
                    "Stock quantity must be greater than or equal to 0".to_string(),
                ));
            }
        }

        let mut dto = dto;
        dto.category = optional_product_category(dto.category)?;
        let product = self.repo.update(id, &dto, actor_id).await?;
        self.invalidate_products(Some(id)).await?;
        Ok(product)
    }

    pub async fn delete(&self, id: Uuid) -> Result<Product, AppError> {
        let product = self.repo.deactivate(id).await?;
        self.invalidate_products(Some(id)).await?;
        Ok(product)
    }
}
