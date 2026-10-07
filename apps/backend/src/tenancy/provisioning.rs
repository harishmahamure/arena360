use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Acquire, Sqlite, Transaction};
use uuid::Uuid;

use crate::control::{CreateTenant, LeaseClient, Repository, Tenant};
use crate::error::AppError;

use super::{format_sqlite_timestamp, migrate, target_schema_version, tenant_path};

const ADMIN_PERMISSIONS: &str = r#"["access:read","access:manage","team:read","team:write","kitchen:read","kitchen:write","kitchen:manage","finance:read","activity:read","events:admin","events:staff","notifications:read","devices:read","devices:write","plans:read","plans:write","products:read","products:write","sessions:read","sessions:write","stats:read","transactions:read","transactions:write","player-plans:read","player-plans:write","players:read","players:write","units:read","units:write","shifts:read","shifts:write","shifts:force_close","cash-registers:read","cash-registers:write","cash-registers:reconcile","cash-registers:adjust_opening","cash-deposits:read","cash-deposits:write","cash-deposits:approve","credit:read","credit:write","credit-limit:write","staff-gaming-allowance:read","staff-gaming-allowance:write","expenses:read","expenses:write","expenses:approve","vendors:read","vendors:write","config:read","config:write","settings:read","settings:write","rules:read","rules:edit","rules:publish","games:read","games:write","inventory:read","inventory:manage","inventory:transfer_request","inventory:transfer_fulfill","inventory:waste_record","inventory:waste_approve","procurement:read","procurement:write","procurement:approve","procurement:receive","inventory:reorder_manage"]"#;
const STAFF_PERMISSIONS: &str = r#"["devices:read","plans:read","products:read","sessions:read","sessions:write","stats:read","transactions:read","transactions:write","player-plans:read","player-plans:write","players:read","players:write","units:read","shifts:read","shifts:write","cash-registers:read","cash-registers:write","cash-deposits:read","cash-deposits:write","credit:read","credit:write","expenses:read","settings:read","rules:read","games:read","inventory:read","inventory:transfer_request","inventory:waste_record","procurement:read","procurement:write","procurement:receive","kitchen:read","kitchen:write","events:staff","team:read","notifications:read"]"#;
const MANAGER_PERMISSIONS: &str = r#"["kitchen:read","kitchen:write","kitchen:manage","finance:read","activity:read","notifications:read","devices:read","devices:write","plans:read","plans:write","products:read","products:write","sessions:read","sessions:write","stats:read","transactions:read","transactions:write","player-plans:read","player-plans:write","players:read","players:write","units:read","units:write","shifts:read","shifts:force_close","cash-registers:read","cash-registers:reconcile","cash-registers:adjust_opening","cash-deposits:read","cash-deposits:write","cash-deposits:approve","credit:read","credit:write","credit-limit:write","staff-gaming-allowance:read","staff-gaming-allowance:write","expenses:read","expenses:write","expenses:approve","vendors:read","vendors:write","config:read","config:write","settings:read","settings:write","rules:read","rules:edit","rules:publish","games:read","games:write","inventory:read","inventory:manage","inventory:transfer_request","inventory:transfer_fulfill","inventory:waste_record","inventory:waste_approve","procurement:read","procurement:write","procurement:approve","procurement:receive","inventory:reorder_manage"]"#;
const FINANCE_PERMISSIONS: &str = r#"["finance:read","expenses:read","vendors:read","transactions:read","credit:read","cash-deposits:read","cash-registers:read","shifts:read"]"#;
const AUDITOR_PERMISSIONS: &str = r#"["finance:read","expenses:read","vendors:read","transactions:read","credit:read","cash-deposits:read","cash-registers:read","shifts:read","activity:read"]"#;

const UNIT_SEEDS: [(&str, &str, &str, &str); 11] = [
    (
        "018f0000-0000-7000-8000-000000000001",
        "Piece",
        "pc",
        "piece",
    ),
    ("018f0000-0000-7000-8000-000000000002", "Box", "box", "box"),
    (
        "018f0000-0000-7000-8000-000000000003",
        "Carton",
        "ctn",
        "carton",
    ),
    (
        "018f0000-0000-7000-8000-000000000004",
        "Pack",
        "pack",
        "pack",
    ),
    (
        "018f0000-0000-7000-8000-000000000005",
        "Bottle",
        "bt",
        "bottle",
    ),
    ("018f0000-0000-7000-8000-000000000006", "Can", "can", "can"),
    (
        "018f0000-0000-7000-8000-000000000007",
        "Kilogram",
        "kg",
        "kilogram",
    ),
    ("018f0000-0000-7000-8000-000000000008", "Gram", "g", "gram"),
    (
        "018f0000-0000-7000-8000-000000000009",
        "Liter",
        "L",
        "liter",
    ),
    (
        "018f0000-0000-7000-8000-00000000000a",
        "Milliliter",
        "ml",
        "milliliter",
    ),
    (
        "018f0000-0000-7000-8000-00000000000b",
        "Other",
        "other",
        "other",
    ),
];

const ROLE_SEEDS: [(&str, &str, &str, &str, bool); 7] = [
    (
        "018f0000-0000-7000-9000-000000000001",
        "admin",
        "Administrator",
        ADMIN_PERMISSIONS,
        false,
    ),
    (
        "018f0000-0000-7000-9000-000000000002",
        "staff",
        "Counter operator",
        STAFF_PERMISSIONS,
        false,
    ),
    (
        "018f0000-0000-7000-9000-000000000003",
        "template-manager",
        "Venue manager",
        MANAGER_PERMISSIONS,
        true,
    ),
    (
        "018f0000-0000-7000-9000-000000000004",
        "template-counter",
        "Counter operator",
        STAFF_PERMISSIONS,
        true,
    ),
    (
        "018f0000-0000-7000-9000-000000000005",
        "template-kitchen",
        "Kitchen operator",
        r#"["kitchen:read","kitchen:write"]"#,
        true,
    ),
    (
        "018f0000-0000-7000-9000-000000000006",
        "template-finance",
        "Finance reviewer",
        FINANCE_PERMISSIONS,
        true,
    ),
    (
        "018f0000-0000-7000-9000-000000000007",
        "template-auditor",
        "Auditor",
        AUDITOR_PERMISSIONS,
        true,
    ),
];

#[derive(Debug, Clone)]
pub struct InitialSettingOverride {
    pub location_id: Option<Uuid>,
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Clone)]
pub struct ProvisionTenant {
    pub tenant: CreateTenant,
    pub settings: Vec<InitialSettingOverride>,
}

#[derive(Debug, Clone)]
pub struct ProvisionedTenant {
    pub tenant: Tenant,
    pub path: PathBuf,
    pub ownership_generation: i64,
}

#[async_trait]
pub trait ProvisioningControl: Send + Sync {
    async fn register(&self, tenant: CreateTenant) -> Result<Tenant, AppError>;
    async fn acquire_lease(&self, tenant_id: Uuid) -> Result<i64, AppError>;
    async fn finalize(
        &self,
        tenant_id: Uuid,
        ownership_generation: i64,
        schema_version: i64,
    ) -> Result<Tenant, AppError>;
}

pub struct PostgresProvisioningControl {
    repository: Repository,
    leases: Arc<LeaseClient>,
}

impl PostgresProvisioningControl {
    pub fn new(repository: Repository, leases: Arc<LeaseClient>) -> Self {
        Self { repository, leases }
    }
}

#[async_trait]
impl ProvisioningControl for PostgresProvisioningControl {
    async fn register(&self, tenant: CreateTenant) -> Result<Tenant, AppError> {
        if tenant.owner_cell != Some(self.leases.cell_id()) {
            return Err(AppError::BadRequest(
                "Provisioning owner cell must match the local cell".into(),
            ));
        }
        self.repository.create_or_resume_tenant(tenant).await
    }

    async fn acquire_lease(&self, tenant_id: Uuid) -> Result<i64, AppError> {
        Ok(self
            .leases
            .acquire_for_provisioning(tenant_id)
            .await?
            .ownership_generation)
    }

    async fn finalize(
        &self,
        tenant_id: Uuid,
        ownership_generation: i64,
        schema_version: i64,
    ) -> Result<Tenant, AppError> {
        self.repository
            .finalize_provisioning(
                tenant_id,
                self.leases.cell_id(),
                ownership_generation,
                schema_version,
            )
            .await
    }
}

pub struct TenantProvisioner {
    root: PathBuf,
    control: Arc<dyn ProvisioningControl>,
}

impl TenantProvisioner {
    pub fn new(root: PathBuf, control: Arc<dyn ProvisioningControl>) -> Self {
        Self { root, control }
    }

    pub async fn provision(&self, request: ProvisionTenant) -> Result<ProvisionedTenant, AppError> {
        validate_settings(&request.tenant.timezone, &request.settings)?;
        let tenant = self.control.register(request.tenant).await?;
        let path = create_tenant_file(&self.root, tenant.id).await?;
        seed_defaults(&path, &request.settings, &tenant.timezone).await?;
        let ownership_generation = self.control.acquire_lease(tenant.id).await?;
        let tenant = self
            .control
            .finalize(tenant.id, ownership_generation, target_schema_version())
            .await?;
        Ok(ProvisionedTenant {
            tenant,
            path,
            ownership_generation,
        })
    }
}

fn validate_settings(
    default_timezone: &str,
    settings: &[InitialSettingOverride],
) -> Result<(), AppError> {
    for setting in settings {
        crate::services::settings_catalog::validate(
            default_timezone,
            &setting.key,
            &setting.value,
            setting.location_id.is_some(),
        )?;
    }
    Ok(())
}

async fn create_tenant_file(root: &Path, tenant_id: Uuid) -> Result<PathBuf, AppError> {
    let path = tenant_path(root, tenant_id);
    let parent = path
        .parent()
        .ok_or_else(|| AppError::Internal("tenant database path has no parent".into()))?;
    tokio::fs::create_dir_all(parent)
        .await
        .map_err(|error| AppError::Internal(format!("create tenant directory: {error}")))?;
    let existing_length = match tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
    {
        Ok(file) => {
            drop(file);
            None
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Some(
            tokio::fs::metadata(&path)
                .await
                .map_err(|error| {
                    AppError::Internal(format!("inspect tenant database file: {error}"))
                })?
                .len(),
        ),
        Err(error) => {
            return Err(AppError::Internal(format!(
                "create tenant database file: {error}"
            )));
        }
    };
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await?;
    if existing_length.is_some_and(|length| length > 0) {
        let has_migration_metadata: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema \
             WHERE type = 'table' AND name = '_sqlx_migrations')",
        )
        .fetch_one(&pool)
        .await?;
        if !has_migration_metadata {
            pool.close().await;
            return Err(AppError::Conflict(
                "Existing tenant database has no migration metadata".into(),
            ));
        }
    }
    migrate(&pool)
        .await
        .map_err(|error| AppError::Internal(format!("tenant migration failed: {error}")))?;
    pool.close().await;
    Ok(path)
}

async fn seed_defaults(
    path: &Path,
    settings: &[InitialSettingOverride],
    timezone: &str,
) -> Result<(), AppError> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(SqliteConnectOptions::new().filename(path))
        .await?;
    let mut connection = pool.acquire().await?;
    let mut transaction = connection.begin().await?;
    let timestamp = format_sqlite_timestamp(&Utc::now())
        .map_err(|error| AppError::Internal(format!("format tenant timestamp: {error}")))?;
    sqlx::query("INSERT INTO tenant_runtime(singleton,timezone) VALUES(1,?) ON CONFLICT(singleton) DO NOTHING")
        .bind(timezone).execute(&mut *transaction).await?;
    seed_units(&mut transaction, &timestamp).await?;
    seed_roles(&mut transaction, &timestamp).await?;
    seed_settings(&mut transaction, settings, &timestamp).await?;
    transaction.commit().await?;
    drop(connection);
    pool.close().await;
    Ok(())
}

async fn seed_units(
    transaction: &mut Transaction<'_, Sqlite>,
    timestamp: &str,
) -> Result<(), AppError> {
    for (id, name, abbreviation, unit_type) in UNIT_SEEDS {
        sqlx::query(
            r#"INSERT OR IGNORE INTO units
                 (id, name, abbreviation, unit_type, is_active, created_at, updated_at)
               VALUES ($1, $2, $3, $4, 1, $5, $5)"#,
        )
        .bind(id)
        .bind(name)
        .bind(abbreviation)
        .bind(unit_type)
        .bind(timestamp)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn seed_roles(
    transaction: &mut Transaction<'_, Sqlite>,
    timestamp: &str,
) -> Result<(), AppError> {
    for (id, system_key, name, permissions, is_template) in ROLE_SEEDS {
        sqlx::query(
            r#"INSERT OR IGNORE INTO access_roles
                 (id, system_key, name, permissions, is_template, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $6)"#,
        )
        .bind(id)
        .bind(system_key)
        .bind(name)
        .bind(permissions)
        .bind(is_template)
        .bind(timestamp)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn seed_settings(
    transaction: &mut Transaction<'_, Sqlite>,
    settings: &[InitialSettingOverride],
    timestamp: &str,
) -> Result<(), AppError> {
    for setting in settings {
        sqlx::query(
            r#"INSERT OR IGNORE INTO setting_overrides
                 (id, location_id, key, value, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $5)"#,
        )
        .bind(Uuid::now_v7().to_string())
        .bind(setting.location_id.map(|id| id.to_string()))
        .bind(&setting.key)
        .bind(serde_json::to_string(&setting.value).map_err(|error| {
            AppError::Internal(format!("serialize initial setting override: {error}"))
        })?)
        .bind(timestamp)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}
