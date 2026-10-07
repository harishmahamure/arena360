//! Authorization for settings uses the tenant's secret-free membership projection.
use super::{TenantSettingsRepository, TenantUserRepository};
use crate::{
    error::AppError,
    models::{OrganizationMembershipContext, VenueLocation},
};
use uuid::Uuid;
impl TenantSettingsRepository {
    pub async fn membership_context(
        &self,
        user: Uuid,
    ) -> Result<Option<OrganizationMembershipContext>, AppError> {
        Ok(sqlx::query_as("SELECT ? AS organization_id,role,permissions FROM users WHERE id=? AND role IN('admin','staff') AND is_active=1 AND deleted_at IS NULL").bind(self.db.tenant_id()).bind(user.to_string()).fetch_optional(&self.db.read_pool()?).await?)
    }
    pub async fn ensure_access(
        &self,
        org: Uuid,
        user: Uuid,
        permission: &str,
    ) -> Result<(), AppError> {
        self.require_tenant(org)?;
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u WHERE u.id=? AND u.is_active=1 AND u.deleted_at IS NULL AND u.role IN('admin','staff') AND (EXISTS(SELECT 1 FROM access_assignments a JOIN access_roles r ON r.id=a.role_id JOIN json_each(r.permissions) p ON p.value=? WHERE a.user_id=u.id AND r.is_template=0)) AND COALESCE((SELECT enabled FROM access_modules WHERE module=?),1)=1)").bind(user.to_string()).bind(permission).bind(permission.split(':').next().unwrap_or(permission)).fetch_one(&self.db.read_pool()?).await?;
        if allowed {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "Permission required: {permission}"
            )))
        }
    }
    pub async fn validate_location(&self, org: Uuid, id: Uuid) -> Result<(), AppError> {
        self.require_tenant(org)?;
        let active: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM venue_locations WHERE id=? AND is_active=1)",
        )
        .bind(id.to_string())
        .fetch_one(&self.db.read_pool()?)
        .await?;
        if active {
            Ok(())
        } else {
            Err(AppError::NotFound("Active location not found".into()))
        }
    }
    pub async fn ensure_location_access(
        &self,
        org: Uuid,
        location: Uuid,
        user: Uuid,
    ) -> Result<(), AppError> {
        self.validate_location(org, location).await?;
        let allowed: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users u WHERE id=? AND is_active=1 AND deleted_at IS NULL AND (role='admin' OR (role='staff' AND EXISTS(SELECT 1 FROM location_role_assignments WHERE user_id=u.id AND location_id=?))))").bind(user.to_string()).bind(location.to_string()).fetch_one(&self.db.read_pool()?).await?;
        if allowed {
            Ok(())
        } else {
            Err(AppError::Forbidden("Location access required".into()))
        }
    }
    pub async fn ensure_location_permission(
        &self,
        org: Uuid,
        location: Uuid,
        user: Uuid,
        permission: &str,
    ) -> Result<(), AppError> {
        self.require_tenant(org)?;
        let allowed = TenantUserRepository::new(self.db.clone())
            .location_ids_for_permission(user, permission)
            .await?;
        if allowed.contains(&location) {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "Permission required at this location: {permission}"
            )))
        }
    }
    pub async fn list_locations(
        &self,
        org: Uuid,
        user: Uuid,
    ) -> Result<Vec<VenueLocation>, AppError> {
        self.require_tenant(org)?;
        Ok(sqlx::query_as("SELECT unhex(replace(l.id,'-','')) AS id,? AS organization_id,l.slug,l.name,l.timezone,l.currency,l.is_active FROM venue_locations l JOIN users u ON u.id=? WHERE l.is_active=1 AND u.is_active=1 AND u.deleted_at IS NULL AND (u.role='admin' OR (u.role='staff' AND EXISTS(SELECT 1 FROM location_role_assignments a WHERE a.user_id=u.id AND a.location_id=l.id))) ORDER BY l.name,l.id").bind(org).bind(user.to_string()).fetch_all(&self.db.read_pool()?).await?)
    }
    pub async fn list_managed_locations(&self, org: Uuid) -> Result<Vec<VenueLocation>, AppError> {
        self.require_tenant(org)?;
        Ok(sqlx::query_as("SELECT unhex(replace(id,'-','')) AS id,? AS organization_id,slug,name,timezone,currency,is_active FROM venue_locations ORDER BY name,id").bind(org).fetch_all(&self.db.read_pool()?).await?)
    }
}
