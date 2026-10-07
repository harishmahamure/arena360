use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateVendorDto, UpdateVendorDto, Vendor, VendorFilterDto};
use crate::repositories::VendorRepository;

pub struct VendorService {
    repo: VendorRepository,
}

impl VendorService {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: VendorRepository::new(pool),
        }
    }

    pub async fn list(
        &self,
        filters: VendorFilterDto,
    ) -> Result<PaginationResult<Vendor>, AppError> {
        self.repo.list(&filters).await
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Vendor, AppError> {
        self.repo
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Vendor with ID {id} not found")))
    }

    pub async fn create(
        &self,
        dto: CreateVendorDto,
        created_by: Option<Uuid>,
    ) -> Result<Vendor, AppError> {
        if self.repo.name_exists(&dto.name, None).await? {
            return Err(AppError::Conflict(format!(
                "Vendor '{}' already exists",
                dto.name
            )));
        }

        self.repo.create(&dto, created_by).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: UpdateVendorDto,
        updated_by: Option<Uuid>,
    ) -> Result<Vendor, AppError> {
        if let Some(name) = &dto.name {
            if self.repo.name_exists(name, Some(id)).await? {
                return Err(AppError::Conflict(format!(
                    "Vendor '{name}' already exists"
                )));
            }
        }

        self.repo.update(id, &dto, updated_by).await
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        self.repo.delete(id).await
    }
}

impl VendorService {
    pub async fn list_tenant(
        &self,
        db: std::sync::Arc<crate::tenancy::TenantDb>,
        f: VendorFilterDto,
    ) -> Result<PaginationResult<Vendor>, AppError> {
        crate::repositories::TenantVendorRepository::new(db)
            .list(&f)
            .await
    }
    pub async fn get_tenant(
        &self,
        db: std::sync::Arc<crate::tenancy::TenantDb>,
        id: Uuid,
    ) -> Result<Vendor, AppError> {
        crate::repositories::TenantVendorRepository::new(db)
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Vendor with ID {id} not found")))
    }
    pub async fn create_tenant(
        &self,
        db: std::sync::Arc<crate::tenancy::TenantDb>,
        dto: CreateVendorDto,
        actor: Option<Uuid>,
    ) -> Result<Vendor, AppError> {
        crate::repositories::TenantVendorRepository::new(db)
            .create(&dto, actor)
            .await
    }
    pub async fn update_tenant(
        &self,
        db: std::sync::Arc<crate::tenancy::TenantDb>,
        id: Uuid,
        dto: UpdateVendorDto,
        actor: Option<Uuid>,
    ) -> Result<Vendor, AppError> {
        crate::repositories::TenantVendorRepository::new(db)
            .update(id, &dto, actor)
            .await
    }
    pub async fn delete_tenant(
        &self,
        db: std::sync::Arc<crate::tenancy::TenantDb>,
        id: Uuid,
    ) -> Result<(), AppError> {
        crate::repositories::TenantVendorRepository::new(db)
            .delete(id)
            .await
    }
}
