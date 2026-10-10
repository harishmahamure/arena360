//! Resolve location grants from current memberships, never from a client supplied role.
use crate::{dto::JwtUserClaims, error::AppError};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct LocationScope {
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub organization_admin: bool,
    pub locations: Vec<Uuid>,
}
impl LocationScope {
    pub async fn resolve_tenant(
        db: std::sync::Arc<crate::tenancy::TenantDb>,
        claims: &JwtUserClaims,
        permission: &str,
        requested: Option<Uuid>,
    ) -> Result<Self, AppError> {
        let org = Uuid::parse_str(&claims.tenantId)
            .map_err(|_| AppError::Unauthorized("Invalid tenant identity".into()))?;
        let user = claims
            .user_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Invalid user identity".into()))?;
        let settings = crate::repositories::TenantSettingsRepository::new(db.clone());
        settings.ensure_access(org, user, permission).await?;
        let membership = settings
            .membership_context(user)
            .await?
            .ok_or_else(|| AppError::Forbidden("Active membership required".into()))?;
        let admin = membership.role == "admin";
        let mut locations = crate::repositories::TenantUserRepository::new(db)
            .location_ids_for_permission(user, permission)
            .await?;
        if let Some(id) = requested {
            if !locations.contains(&id) {
                return Err(AppError::Forbidden(format!(
                    "Permission required at this location: {permission}"
                )));
            }
            locations = vec![id];
        }
        if locations.is_empty() && !admin {
            return Err(AppError::Forbidden(
                "No locations assigned for this operation".into(),
            ));
        }
        Ok(Self {
            organization_id: org,
            user_id: user,
            organization_admin: admin,
            locations,
        })
    }

    pub fn first(&self) -> Result<Uuid, AppError> {
        self.locations
            .first()
            .copied()
            .ok_or_else(|| AppError::BadRequest("Create an active location first".into()))
    }
}

pub fn requested_location(headers: &axum::http::HeaderMap) -> Result<Option<Uuid>, AppError> {
    headers
        .get("x-location-id")
        .map(|v| {
            v.to_str()
                .ok()
                .and_then(|v| Uuid::parse_str(v).ok())
                .ok_or_else(|| AppError::BadRequest("Invalid location selection".into()))
        })
        .transpose()
}
pub async fn require_tenant_admin(
    db: std::sync::Arc<crate::tenancy::TenantDb>,
    org: Uuid,
    user: Uuid,
) -> Result<(), AppError> {
    if db.tenant_id() != org {
        return Err(AppError::Forbidden("Tenant identity mismatch".into()));
    }
    let membership = crate::repositories::TenantSettingsRepository::new(db)
        .membership_context(user)
        .await?;
    if membership.is_some_and(|m| m.role == "admin") {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Organization administrator access is required to change shared defaults".into(),
        ))
    }
}

pub async fn report_scope_tenant(
    db: std::sync::Arc<crate::tenancy::TenantDb>,
    claims: &JwtUserClaims,
    headers: &axum::http::HeaderMap,
    requested: Option<Uuid>,
    permission: &str,
) -> Result<Option<Vec<Uuid>>, AppError> {
    let selected = requested.or(requested_location(headers)?);
    let scope = LocationScope::resolve_tenant(db, claims, permission, selected).await?;
    Ok(if scope.organization_admin && selected.is_none() {
            None
        } else {
            Some(scope.locations)
        })
}

/// Staff read by venue grant; player proofs only read that player's history.
#[derive(Debug, Clone)]
pub struct TransactionReadScope {
    pub player_id: Option<Uuid>,
    pub locations: Option<Vec<Uuid>>,
}
impl TransactionReadScope {
    pub async fn resolve_tenant(db: std::sync::Arc<crate::tenancy::TenantDb>, claims: &JwtUserClaims, requested: Option<Uuid>) -> Result<Self, AppError> {
        if claims.is_admin_or_staff() {
            let scope = LocationScope::resolve_tenant(db, claims, "transactions:read", requested).await?;
            Ok(Self { player_id: None, locations: Some(scope.locations) })
        } else if claims.is_player() {
            let current = crate::realtime::tenant_transport::current_claims(db, claims).await?;
            let player = current.user_id_uuid().ok_or_else(|| AppError::Unauthorized("Invalid player identity".into()))?;
            Ok(Self { player_id: Some(player), locations: requested.map(|id| vec![id]) })
        } else {
            Err(AppError::Forbidden("Player or panel identity required".into()))
        }
    }
    pub fn apply_player_filter(&self, filters: &mut crate::models::TransactionFilterDto) -> Result<(), AppError> {
        if let Some(player) = self.player_id {
            if filters.player_id.is_some_and(|requested| requested != player) {
                return Err(AppError::Forbidden("Cannot read another player's transactions".into()));
            }
            filters.player_id = Some(player);
        }
        Ok(())
    }
    pub fn ensure_resource(&self, player: Uuid, venue: Uuid) -> Result<(), AppError> {
        if self.player_id.is_some_and(|allowed| allowed != player) || self.locations.as_ref().is_some_and(|allowed| !allowed.contains(&venue)) {
            Err(AppError::Forbidden("Transaction is outside the permitted scope".into()))
        } else { Ok(()) }
    }
}
