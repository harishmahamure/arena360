use std::sync::Arc;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::error::AppError;

use super::{format_sqlite_timestamp, TenantDb};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedVenueLocation {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocationProjectionResult {
    pub inserted_or_updated: u64,
    pub deactivated: u64,
}

pub async fn sync_venue_locations(
    db: Arc<TenantDb>,
    locations: Vec<ProjectedVenueLocation>,
) -> Result<LocationProjectionResult, AppError> {
    let stale_at = format_sqlite_timestamp(&Utc::now())
        .map_err(|error| AppError::Internal(format!("format projection timestamp: {error}")))?;
    db.with_immediate_writer(move |connection| {
        Box::pin(async move {
            let mut changed = 0;
            for location in &locations {
                let created_at =
                    format_sqlite_timestamp(&location.created_at).map_err(|error| {
                        AppError::Internal(format!("format location created_at: {error}"))
                    })?;
                let updated_at =
                    format_sqlite_timestamp(&location.updated_at).map_err(|error| {
                        AppError::Internal(format!("format location updated_at: {error}"))
                    })?;
                changed += sqlx::query(
                    "INSERT INTO venue_locations(
                        id,slug,name,is_active,created_at,updated_at
                     ) VALUES (?,?,?,?,?,?)
                     ON CONFLICT(id) DO UPDATE SET
                        slug=excluded.slug,
                        name=excluded.name,
                        is_active=excluded.is_active,
                        updated_at=excluded.updated_at
                     WHERE venue_locations.slug IS NOT excluded.slug
                        OR venue_locations.name IS NOT excluded.name
                        OR venue_locations.is_active IS NOT excluded.is_active
                        OR venue_locations.updated_at IS NOT excluded.updated_at",
                )
                .bind(location.id.to_string())
                .bind(&location.slug)
                .bind(&location.name)
                .bind(location.is_active)
                .bind(created_at)
                .bind(updated_at)
                .execute(&mut *connection)
                .await?
                .rows_affected();
            }

            let deactivated = if locations.is_empty() {
                sqlx::query(
                    "UPDATE venue_locations
                     SET is_active=0,updated_at=?
                     WHERE is_active=1",
                )
                .bind(&stale_at)
                .execute(&mut *connection)
                .await?
                .rows_affected()
            } else {
                let mut query =
                    sqlx::QueryBuilder::new("UPDATE venue_locations SET is_active=0,updated_at=");
                query
                    .push_bind(&stale_at)
                    .push(" WHERE is_active=1 AND id NOT IN (");
                let mut ids = query.separated(",");
                for location in &locations {
                    ids.push_bind(location.id.to_string());
                }
                ids.push_unseparated(")");
                query
                    .build()
                    .execute(&mut *connection)
                    .await?
                    .rows_affected()
            };

            Ok(LocationProjectionResult {
                inserted_or_updated: changed,
                deactivated,
            })
        })
    })
    .await
}
