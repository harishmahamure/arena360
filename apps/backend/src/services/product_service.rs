use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateProductDto, Product, ProductFilterDto, UpdateProductDto};
use crate::repositories::ProductRepository;

pub struct ProductService {
    repo: ProductRepository,
}

impl ProductService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: ProductRepository::new(pool),
        }
    }

    pub async fn list(
        &self,
        filters: ProductFilterDto,
    ) -> Result<crate::dto::PaginationResult<Product>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Product, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Product with ID {id} not found")))
    }

    pub async fn create(&self, dto: CreateProductDto) -> Result<Product, AppError> {
        if dto.price < 0.0 {
            return Err(AppError::BadRequest(
                "Price must be greater than or equal to 0".to_string(),
            ));
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

        self.repo.create(&dto).await
    }

    pub async fn update(&self, id: Uuid, dto: UpdateProductDto) -> Result<Product, AppError> {
        if let Some(price) = dto.price {
            if price < 0.0 {
                return Err(AppError::BadRequest(
                    "Price must be greater than or equal to 0".to_string(),
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

        self.repo.update(id, &dto).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<Product, AppError> {
        self.repo.deactivate(id).await
    }
}
