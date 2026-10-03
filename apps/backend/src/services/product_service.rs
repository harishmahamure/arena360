use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{self, get_or_set, keys, CacheService};
use crate::error::AppError;
use crate::models::{CreateProductDto, Product, ProductFilterDto, UpdateProductDto};
use crate::repositories::{ProductRecipeRepository, ProductRepository, UnitRepository};
use crate::validation::{optional_product_category, require_product_category};

pub struct ProductService {
    repo: ProductRepository,
    recipes: ProductRecipeRepository,
    units: UnitRepository,
    cache: Arc<dyn CacheService>,
}

impl ProductService {
    pub fn new(pool: PgPool, cache: Arc<dyn CacheService>) -> Self {
        Self {
            recipes: ProductRecipeRepository::new(pool.clone()),
            units: UnitRepository::new(pool.clone()),
            repo: ProductRepository::new(pool),
            cache,
        }
    }

    pub fn with_locations(mut self, ids: Vec<Uuid>) -> Self {
        self.repo = self.repo.with_locations(ids);
        self
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
        if let Some(conversion) = self
            .metric_conversion(dto.purchase_unit_id, dto.unit_id)
            .await?
        {
            dto.units_per_purchase_unit = Some(conversion);
        }
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
        if dto.is_raw_material == Some(true) && !self.recipes.get(id).await?.is_empty() {
            return Err(AppError::BadRequest(
                "Remove this product's recipe and options before marking it as a raw material"
                    .to_string(),
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
        let current = self
            .repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Product with ID {id} not found")))?;
        if let Some(conversion) = self
            .metric_conversion(
                dto.purchase_unit_id.or(current.purchase_unit_id),
                dto.unit_id.or(current.unit_id),
            )
            .await?
        {
            dto.units_per_purchase_unit = Some(conversion);
        }
        let product = self.repo.update(id, &dto, actor_id).await?;
        self.invalidate_products(Some(id)).await?;
        Ok(product)
    }

    async fn metric_conversion(
        &self,
        purchase_unit_id: Option<Uuid>,
        stock_unit_id: Option<Uuid>,
    ) -> Result<Option<i32>, AppError> {
        let (Some(purchase_id), Some(stock_id)) = (purchase_unit_id, stock_unit_id) else {
            return Ok(None);
        };
        let purchase = self
            .units
            .find_by_id(purchase_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Purchase unit does not exist".to_string()))?;
        let stock = self
            .units
            .find_by_id(stock_id)
            .await?
            .ok_or_else(|| AppError::BadRequest("Stock unit does not exist".to_string()))?;
        metric_unit_conversion(&purchase.r#type, &stock.r#type)
    }

    pub async fn delete(&self, id: Uuid) -> Result<Product, AppError> {
        let product = self.repo.deactivate(id).await?;
        self.invalidate_products(Some(id)).await?;
        Ok(product)
    }
}

/// Inventory and recipes count whole stock units. Known metric conversions
/// must not depend on a manually entered packaging multiplier.
fn metric_unit_conversion(purchase: &str, stock: &str) -> Result<Option<i32>, AppError> {
    match (purchase, stock) {
        ("kilogram", "gram") | ("liter", "milliliter") => Ok(Some(1000)),
        ("gram", "kilogram") | ("milliliter", "liter") => Err(AppError::BadRequest(
            "Use grams or milliliters as the stock unit so recipe portions can be tracked accurately".to_string(),
        )),
        ("kilogram", "kilogram") | ("gram", "gram") | ("liter", "liter") | ("milliliter", "milliliter") => Ok(Some(1)),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod unit_conversion_tests {
    use super::metric_unit_conversion;

    #[test]
    fn kilogram_purchases_supply_gram_portions() {
        let conversion = metric_unit_conversion("kilogram", "gram").unwrap().unwrap();
        let stock = 20 * conversion;
        assert_eq!(stock, 20_000);
        assert_eq!(stock / 20, 1000);
        assert_eq!(stock - 20, 19_980);
    }

    #[test]
    fn metric_pairs_are_fixed_and_packaging_remains_configurable() {
        assert_eq!(
            metric_unit_conversion("liter", "milliliter").unwrap(),
            Some(1000)
        );
        assert_eq!(metric_unit_conversion("gram", "gram").unwrap(), Some(1));
        assert_eq!(metric_unit_conversion("box", "piece").unwrap(), None);
        assert!(metric_unit_conversion("gram", "kilogram").is_err());
        assert!(metric_unit_conversion("milliliter", "liter").is_err());
    }
}
