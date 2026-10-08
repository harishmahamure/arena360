use std::sync::Arc;

use async_trait::async_trait;
use futures::{stream, StreamExt};
use sqlx::{PgPool, SqliteConnection};
use uuid::Uuid;

use crate::error::AppError;

use super::{migrate_connection, target_schema_version, TenantDbManager};

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::FromRow)]
pub struct PendingTenantMigration {
    pub tenant_id: Uuid,
    pub ownership_generation: i64,
    pub schema_version: i64,
}

/// Holds process-independent admission (for example a PostgreSQL advisory lock).
pub struct MigrationAdmission {
    pub(crate) _guard: Option<Box<dyn Send>>,
}
#[async_trait]
pub trait MigrationState: Send + Sync {
    async fn admit(
        &self,
        _cell: Uuid,
        _migration: PendingTenantMigration,
        _target: i64,
    ) -> Result<MigrationAdmission, AppError> {
        Ok(MigrationAdmission { _guard: None })
    }
    async fn record_failure(
        &self,
        _cell: Uuid,
        _migration: PendingTenantMigration,
        _target: i64,
        _error: &str,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn pending(
        &self,
        cell_id: Uuid,
        target_version: i64,
    ) -> Result<Vec<PendingTenantMigration>, AppError>;

    async fn record_version(
        &self,
        cell_id: Uuid,
        migration: PendingTenantMigration,
        version: i64,
    ) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct PostgresMigrationState {
    pool: PgPool,
}

impl PostgresMigrationState {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl MigrationState for PostgresMigrationState {
    async fn pending(
        &self,
        cell_id: Uuid,
        target_version: i64,
    ) -> Result<Vec<PendingTenantMigration>, AppError> {
        Ok(sqlx::query_as::<_, PendingTenantMigration>(
            r#"SELECT id AS tenant_id, ownership_generation, schema_version
               FROM tenants
               WHERE owner_cell = $1
                 AND state NOT IN ('FAILED', 'DELETED', 'COLD')
                 AND schema_version < $2
               ORDER BY id"#,
        )
        .bind(cell_id)
        .bind(target_version)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn record_version(
        &self,
        cell_id: Uuid,
        migration: PendingTenantMigration,
        version: i64,
    ) -> Result<(), AppError> {
        let updated = sqlx::query(
            r#"UPDATE tenants
               SET schema_version = $1, updated_at = NOW()
               WHERE id = $2
                 AND owner_cell = $3
                 AND ownership_generation = $4
                 AND schema_version <= $1"#,
        )
        .bind(version)
        .bind(migration.tenant_id)
        .bind(cell_id)
        .bind(migration.ownership_generation)
        .execute(&self.pool)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(AppError::Conflict(
                "tenant ownership changed while recording migration".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MigrationContext {
    pub tenant_id: Uuid,
    pub ownership_generation: i64,
    pub from_version: i64,
    pub to_version: i64,
}

#[async_trait]
pub trait MigrationHook: Send + Sync {
    async fn before(
        &self,
        _context: MigrationContext,
        _connection: &mut SqliteConnection,
    ) -> Result<(), AppError> {
        Ok(())
    }

    async fn after(
        &self,
        _context: MigrationContext,
        _connection: &mut SqliteConnection,
    ) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MigrationOrchestratorConfig {
    pub max_concurrency: usize,
}

impl Default for MigrationOrchestratorConfig {
    fn default() -> Self {
        Self { max_concurrency: 1 }
    }
}

impl MigrationOrchestratorConfig {
    fn validate(self) -> Result<Self, AppError> {
        if self.max_concurrency == 0 {
            return Err(AppError::Internal(
                "migration concurrency must be greater than zero".into(),
            ));
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationOutcome {
    pub tenant_id: Uuid,
    pub from_version: i64,
    pub to_version: i64,
    pub error: Option<String>,
}

impl MigrationOutcome {
    pub fn succeeded(&self) -> bool {
        self.error.is_none()
    }
}

pub struct MigrationOrchestrator {
    cell_id: Uuid,
    databases: Arc<TenantDbManager>,
    state: Arc<dyn MigrationState>,
    hooks: Vec<Arc<dyn MigrationHook>>,
    config: MigrationOrchestratorConfig,
}

impl MigrationOrchestrator {
    pub fn new(
        cell_id: Uuid,
        databases: Arc<TenantDbManager>,
        state: Arc<dyn MigrationState>,
        hooks: Vec<Arc<dyn MigrationHook>>,
        config: MigrationOrchestratorConfig,
    ) -> Result<Self, AppError> {
        Ok(Self {
            cell_id,
            databases,
            state,
            hooks,
            config: config.validate()?,
        })
    }

    pub async fn run_pending(&self) -> Result<Vec<MigrationOutcome>, AppError> {
        let target_version = target_schema_version();
        let pending = self.state.pending(self.cell_id, target_version).await?;
        let cell_id = self.cell_id;
        let databases = self.databases.clone();
        let state = self.state.clone();
        let hooks = self.hooks.clone();

        Ok(stream::iter(pending)
            .map(move |migration| {
                let databases = databases.clone();
                let state = state.clone();
                let hooks = hooks.clone();
                async move {
                    match migrate_one(cell_id, target_version, migration, databases, state, hooks)
                        .await
                    {
                        Ok(outcome) => outcome,
                        Err(error) => MigrationOutcome {
                            tenant_id: migration.tenant_id,
                            from_version: migration.schema_version,
                            to_version: target_version,
                            error: Some(error.to_string()),
                        },
                    }
                }
            })
            .buffer_unordered(self.config.max_concurrency)
            .collect()
            .await)
    }
}

async fn migrate_one(
    cell_id: Uuid,
    target_version: i64,
    migration: PendingTenantMigration,
    databases: Arc<TenantDbManager>,
    state: Arc<dyn MigrationState>,
    hooks: Vec<Arc<dyn MigrationHook>>,
) -> Result<MigrationOutcome, AppError> {
    let _permit = match databases.background_jobs() {
        Some(j) => Some(
            j.acquire(crate::background::Priority::SchemaBackfill)
                .await?,
        ),
        None => None,
    };
    let _admission = state.admit(cell_id, migration, target_version).await?;
    let result = migrate_admitted(
        cell_id,
        target_version,
        migration,
        databases,
        state.clone(),
        hooks,
    )
    .await;
    if let Err(error) = &result {
        state
            .record_failure(cell_id, migration, target_version, &error.to_string())
            .await?;
    }
    result
}
async fn migrate_admitted(
    cell_id: Uuid,
    target_version: i64,
    migration: PendingTenantMigration,
    databases: Arc<TenantDbManager>,
    state: Arc<dyn MigrationState>,
    hooks: Vec<Arc<dyn MigrationHook>>,
) -> Result<MigrationOutcome, AppError> {
    let database = databases.open(migration.tenant_id).await?;
    if database.ownership_generation() != migration.ownership_generation {
        return Err(AppError::Conflict(
            "tenant ownership changed before migration".into(),
        ));
    }

    let context = database
        .with_writer(move |connection| {
            Box::pin(async move {
                let from_version = current_schema_version(connection).await?;
                let context = MigrationContext {
                    tenant_id: migration.tenant_id,
                    ownership_generation: migration.ownership_generation,
                    from_version,
                    to_version: target_version,
                };
                if from_version > target_version {
                    return Err(AppError::Conflict(format!(
                        "tenant schema version {from_version} is newer than supported version {target_version}"
                    )));
                }
                if from_version < target_version {
                    for hook in &hooks {
                        hook.before(context, connection).await?;
                    }
                    migrate_connection(connection).await.map_err(|error| {
                        AppError::Internal(format!("tenant migration failed: {error}"))
                    })?;
                }
                for hook in &hooks {
                    hook.after(context, connection).await?;
                }
                Ok(context)
            })
        })
        .await?;

    state
        .record_version(cell_id, migration, context.to_version)
        .await?;
    Ok(MigrationOutcome {
        tenant_id: migration.tenant_id,
        from_version: context.from_version,
        to_version: context.to_version,
        error: None,
    })
}

async fn current_schema_version(connection: &mut SqliteConnection) -> Result<i64, AppError> {
    let has_migrations: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(&mut *connection)
    .await?;
    if !has_migrations {
        return Ok(0);
    }
    Ok(
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations WHERE success")
            .fetch_one(connection)
            .await?,
    )
}
