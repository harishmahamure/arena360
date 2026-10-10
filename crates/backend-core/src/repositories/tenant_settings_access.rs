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
    pub async fn effective_permissions(&self, user: Uuid) -> Result<Vec<String>, AppError> {
        if self.membership_context(user).await?.is_none() {
            return Err(AppError::Unauthorized("Account or tenant membership revoked".into()));
        }
        let mut grants: Vec<String> = sqlx::query_scalar("SELECT DISTINCT p.value FROM access_assignments a JOIN access_roles r ON r.id=a.role_id JOIN json_each(r.permissions) p WHERE a.user_id=? AND r.is_template=0 AND COALESCE((SELECT enabled FROM access_modules WHERE module=substr(p.value,1,instr(p.value,':')-1)),1)=1 ORDER BY p.value")
            .bind(user.to_string()).fetch_all(&self.db.read_pool()?).await?;
        grants.retain(|p| crate::access::known(p));
        grants.push(crate::access::MANAGED.into());
        grants.sort();
        Ok(grants)
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

impl TenantSettingsRepository {
    pub fn validate_location_dto(
        dto: &crate::models::SaveVenueLocationDto,
    ) -> Result<(), AppError> {
        let slug = dto.slug.trim();
        if slug.is_empty()
            || slug.len() > 80
            || !slug
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            || dto.name.trim().is_empty()
            || dto.name.trim().len() > 160
            || dto.timezone.trim().is_empty()
            || dto.timezone.len() > 80
            || dto.currency.len() != 3
            || !dto.currency.chars().all(|c| c.is_ascii_uppercase())
        {
            return Err(AppError::BadRequest("Use a lowercase location slug, a name, an IANA timezone, and a three-letter currency code".into()));
        }
        crate::services::settings_catalog::validate(
            "Asia/Kolkata",
            "venue.timezone",
            &serde_json::json!(dto.timezone),
            true,
        )?;
        crate::services::settings_catalog::validate(
            "Asia/Kolkata",
            "pricing.currency",
            &serde_json::json!(dto.currency),
            true,
        )?;
        Ok(())
    }

    pub async fn save_location(
        &self,
        org: Uuid,
        id: Option<Uuid>,
        dto: crate::models::SaveVenueLocationDto,
    ) -> Result<VenueLocation, AppError> {
        self.require_tenant(org)?;
        Self::validate_location_dto(&dto)?;
        super::tenant_back_office::write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let at = super::tenant_back_office::now()?;
                    let (location, active) = if let Some(id) = id {
                        let old: Option<bool> = sqlx::query_scalar("SELECT is_active FROM venue_locations WHERE id=?").bind(id.to_string()).fetch_optional(&mut *c).await?;
                        (id, dto.is_active.unwrap_or(old.ok_or_else(|| AppError::NotFound("Location not found".into()))?))
                    } else {
                        (Uuid::now_v7(), dto.is_active.unwrap_or(true))
                    };
                    if !active {
                        let in_use: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM devices WHERE location_id=? AND deleted_at IS NULL UNION ALL SELECT 1 FROM inventory_locations WHERE venue_location_id=? AND is_active=1 AND deleted_at IS NULL UNION ALL SELECT 1 FROM shifts WHERE location_id=? AND status='active')").bind(location.to_string()).bind(location.to_string()).bind(location.to_string()).fetch_one(&mut *c).await?;
                        if in_use {
                            return Err(AppError::Conflict("Move or retire devices and inventory locations, and close active shifts before deactivating this venue".into()));
                        }
                    }
                    let duplicate: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM venue_locations WHERE slug=? AND id<>?)").bind(dto.slug.trim()).bind(location.to_string()).fetch_one(&mut *c).await?;
                    if duplicate {
                        return Err(AppError::Conflict("A location with this slug already exists".into()));
                    }
                    sqlx::query("INSERT INTO venue_locations(id,slug,name,timezone,currency,is_active,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET slug=excluded.slug,name=excluded.name,timezone=excluded.timezone,currency=excluded.currency,is_active=excluded.is_active,updated_at=excluded.updated_at").bind(location.to_string()).bind(dto.slug.trim()).bind(dto.name.trim()).bind(dto.timezone.trim()).bind(dto.currency).bind(active).bind(&at).bind(&at).execute(&mut *c).await?;
                    let row: VenueLocation = sqlx::query_as("SELECT unhex(replace(id,'-','')) id,? organization_id,slug,name,timezone,currency,is_active FROM venue_locations WHERE id=?").bind(org).bind(location.to_string()).fetch_one(&mut *c).await?;
                    super::tenant_back_office::event(c, "venue_location", location, if id.is_some() { "location.updated" } else { "location.created" }, Some(location), false, serde_json::json!(row)).await?;
                    Ok(row)
                })
            }),
        )
        .await
    }
}
