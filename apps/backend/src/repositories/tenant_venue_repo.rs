use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use futures::future::BoxFuture;
use serde_json::{json, Value};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    balance_status, ledger_reason, plan_kind, BalanceFilterDto, BalanceRow, CreateDeviceDto,
    Device, DeviceFilterDto, PlayerPlan, PlayerPlanBalance, PlayerPlanBalanceResponse,
    PlayerPlanCreateValues, PlayerPlanFilterDto, PlayerPlanResponse, PlayerPlanRow,
    PlayerPlanUpdateValues, PurchaseBalanceDto, SessionFilterDto, UpdateDeviceDto, UsageSession,
    UsageSessionResponse, UsageSessionRow,
};
use crate::tenancy::{
    format_sqlite_timestamp, write_outbox_event_on_connection, NewOutboxEvent, TenantDb,
};

type WriteOperation<T> = Box<
    dyn for<'connection> FnOnce(
            &'connection mut SqliteConnection,
        ) -> BoxFuture<'connection, Result<T, AppError>>
        + Send,
>;

async fn write<T: Send + 'static>(
    db: &TenantDb,
    operation: WriteOperation<T>,
) -> Result<T, AppError> {
    db.with_immediate_writer(operation).await
}

fn timestamp(value: &DateTime<Utc>) -> Result<String, AppError> {
    format_sqlite_timestamp(value)
        .map_err(|error| AppError::Internal(format!("format tenant timestamp: {error}")))
}

fn now() -> Result<String, AppError> {
    timestamp(&Utc::now())
}

async fn event(
    connection: &mut SqliteConnection,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    location_id: Option<Uuid>,
    payload: Value,
) -> Result<(), AppError> {
    write_outbox_event_on_connection(
        connection,
        NewOutboxEvent {
            location_id,
            aggregate_type: aggregate_type.into(),
            aggregate_id,
            event_type: event_type.into(),
            schema_version: 1,
            deleted: false,
            payload,
        },
    )
    .await?;
    Ok(())
}

async fn deleted_event(
    connection: &mut SqliteConnection,
    aggregate_type: &str,
    aggregate_id: Uuid,
    event_type: &str,
    location_id: Option<Uuid>,
    payload: Value,
) -> Result<(), AppError> {
    write_outbox_event_on_connection(
        connection,
        NewOutboxEvent {
            location_id,
            aggregate_type: aggregate_type.into(),
            aggregate_id,
            event_type: event_type.into(),
            schema_version: 1,
            deleted: true,
            payload,
        },
    )
    .await?;
    Ok(())
}

const DEVICE_SELECT: &str = r#"
 SELECT unhex(replace(id,'-','')) id,
        unhex(replace(?,'-','')) organization_id,
        unhex(replace(location_id,'-','')) location_id,
        name,serial_number,local_ip_address,device_type,device_sub_type,location,status,
        registered_kiosk,registration_status,
        unhex(replace(created_by,'-','')) created_by,
        unhex(replace(updated_by,'-','')) updated_by,
        created_at,updated_at,deleted_at
 FROM devices
"#;

#[derive(Clone)]
pub struct TenantDeviceRepository {
    db: Arc<TenantDb>,
}

impl TenantDeviceRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Device>, AppError> {
        Ok(sqlx::query_as::<_, Device>(&format!(
            "{DEVICE_SELECT} WHERE id=? AND deleted_at IS NULL"
        ))
        .bind(self.db.tenant_id().to_string())
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }

    pub async fn list(
        &self,
        filters: &DeviceFilterDto,
        allowed_locations: &[Uuid],
    ) -> Result<PaginationResult<Device>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let tenant = format!("'{}'", self.db.tenant_id());
        let mut query = QueryBuilder::<Sqlite>::new(DEVICE_SELECT.replacen('?', &tenant, 1));
        query.push(" WHERE deleted_at IS NULL");
        device_filters(&mut query, filters, allowed_locations);
        let sort = match filters.sort_by.as_deref() {
            Some("name") => "name",
            Some("status") => "status",
            _ => "created_at",
        };
        let direction = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        query
            .push(format!(
                " ORDER BY {sort} {direction},id {direction} LIMIT "
            ))
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1) * limit);
        let rows = query
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?;

        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM devices WHERE deleted_at IS NULL");
        device_filters(&mut count, filters, allowed_locations);
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(rows, total, page, limit))
    }

    pub async fn name_exists(&self, name: &str, exclude: Option<Uuid>) -> Result<bool, AppError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM devices WHERE lower(name)=lower(?) \
             AND (? IS NULL OR id<>?) AND deleted_at IS NULL)",
        )
        .bind(name)
        .bind(exclude.map(|id| id.to_string()))
        .bind(exclude.map(|id| id.to_string()))
        .fetch_one(&self.db.read_pool()?)
        .await?)
    }

    pub async fn find_registered_by_mac(&self, mac: &str) -> Result<Option<Device>, AppError> {
        Ok(sqlx::query_as::<_, Device>(&format!(
            "{DEVICE_SELECT} WHERE deleted_at IS NULL AND registration_status='registered'
             AND registered_kiosk IS NOT NULL
             AND lower(trim(json_extract(registered_kiosk,'$.mac')))=lower(trim(?))
             ORDER BY id LIMIT 1"
        ))
        .bind(self.db.tenant_id().to_string())
        .bind(mac)
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }

    pub async fn create(
        &self,
        dto: &CreateDeviceDto,
        actor: Option<Uuid>,
    ) -> Result<Device, AppError> {
        let id = Uuid::now_v7();
        let at = now()?;
        let location_id = dto
            .location_id
            .unwrap_or(crate::models::DEFAULT_VENUE_LOCATION_ID);
        let name = dto.name.clone();
        let serial = dto.serial_number.clone();
        let ip = dto.local_ip_address.clone();
        let device_type = dto.device_type.clone().unwrap_or_else(|| "OTHER".into());
        let sub_type = dto
            .device_sub_type
            .clone()
            .unwrap_or_else(|| "OTHER".into());
        let location = dto.location.clone();
        let status = dto.status.clone().unwrap_or_else(|| "available".into());
        let registration = dto
            .registration_status
            .clone()
            .unwrap_or_else(|| "unregistered".into());
        let db = self.db.clone();
        write(
            &db,
            Box::new(move |connection| {
                Box::pin(async move {
                    sqlx::query(
                        "INSERT INTO devices(id,location_id,name,serial_number,local_ip_address,\
                         device_type,device_sub_type,location,status,registration_status,created_by,\
                         updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                    )
                    .bind(id.to_string())
                    .bind(location_id.to_string())
                    .bind(&name)
                    .bind(serial)
                    .bind(ip)
                    .bind(device_type)
                    .bind(sub_type)
                    .bind(location)
                    .bind(&status)
                    .bind(registration)
                    .bind(actor.map(|id| id.to_string()))
                    .bind(actor.map(|id| id.to_string()))
                    .bind(&at)
                    .bind(&at)
                    .execute(&mut *connection)
                    .await?;
                    event(
                        connection,
                        "device",
                        id,
                        "device.created",
                        Some(location_id),
                        json!({"id":id,"locationId":location_id,"status":status,"updatedAt":at}),
                    )
                    .await
                })
            }),
        )
        .await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Created device disappeared".into()))
    }

    pub async fn update(
        &self,
        id: Uuid,
        dto: &UpdateDeviceDto,
        actor: Option<Uuid>,
    ) -> Result<Device, AppError> {
        let at = now()?;
        let dto = (
            dto.name.clone(),
            dto.serial_number.clone(),
            dto.local_ip_address.clone(),
            dto.device_type.clone(),
            dto.device_sub_type.clone(),
            dto.location.clone(),
            dto.location_id,
            dto.status.clone(),
            dto.registration_status.clone(),
        );
        let db = self.db.clone();
        write(&db, Box::new(move |connection| Box::pin(async move {
            if let Some(destination) = dto.6 {
                let current: Option<String> = sqlx::query_scalar("SELECT location_id FROM devices WHERE id=? AND deleted_at IS NULL")
                    .bind(id.to_string()).fetch_optional(&mut *connection).await?;
                if current.as_deref().is_some_and(|value| value != destination.to_string()) {
                    let in_use: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_sessions WHERE device_id=? AND end_time IS NULL AND deleted_at IS NULL UNION ALL SELECT 1 FROM kiosk_orders WHERE device_id=? AND status IN('pending','preparing'))")
                        .bind(id.to_string()).bind(id.to_string()).fetch_one(&mut *connection).await?;
                    if in_use { return Err(AppError::Conflict("Finish active sessions and kiosk orders before moving this device".into())); }
                }
            }
            let result=sqlx::query("UPDATE devices SET name=COALESCE(?,name),serial_number=COALESCE(?,serial_number),local_ip_address=COALESCE(?,local_ip_address),device_type=COALESCE(?,device_type),device_sub_type=COALESCE(?,device_sub_type),location=COALESCE(?,location),location_id=COALESCE(?,location_id),status=COALESCE(?,status),registration_status=COALESCE(?,registration_status),updated_by=COALESCE(?,updated_by),updated_at=? WHERE id=? AND deleted_at IS NULL")
                .bind(dto.0).bind(dto.1).bind(dto.2).bind(dto.3).bind(dto.4).bind(dto.5)
                .bind(dto.6.map(|v|v.to_string())).bind(dto.7).bind(dto.8)
                .bind(actor.map(|v|v.to_string())).bind(&at).bind(id.to_string())
                .execute(&mut *connection).await?;
            if result.rows_affected()==0{return Err(AppError::NotFound(format!("Device with ID {id} not found")));}
            let location:String=sqlx::query_scalar("SELECT location_id FROM devices WHERE id=?").bind(id.to_string()).fetch_one(&mut *connection).await?;
            let location=Uuid::parse_str(&location).map_err(|e|AppError::Internal(e.to_string()))?;
            event(connection,"device",id,"device.updated",Some(location),json!({"id":id,"locationId":location,"updatedAt":at})).await
        }))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Device with ID {id} not found")))
    }

    pub async fn update_status(&self, id: Uuid, status: &str) -> Result<Device, AppError> {
        let at = now()?;
        let status = status.to_owned();
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            let location:Option<String>=sqlx::query_scalar("UPDATE devices SET status=?,updated_at=? WHERE id=? AND deleted_at IS NULL RETURNING location_id").bind(&status).bind(&at).bind(id.to_string()).fetch_optional(&mut *connection).await?;
            let location=location.ok_or_else(||AppError::NotFound(format!("Device with ID {id} not found")))?;
            let location=Uuid::parse_str(&location).map_err(|e|AppError::Internal(e.to_string()))?;
            event(connection,"device",id,"device.status_changed",Some(location),json!({"id":id,"locationId":location,"status":status,"updatedAt":at})).await
        }))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Device with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let at = now()?;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_sessions WHERE device_id=? AND end_time IS NULL AND deleted_at IS NULL)").bind(id.to_string()).fetch_one(&mut *connection).await?;
            if active{return Err(AppError::Conflict("Cannot delete a device with an active session".into()));}
            let location:Option<String>=sqlx::query_scalar("UPDATE devices SET deleted_at=?,updated_at=? WHERE id=? AND deleted_at IS NULL RETURNING location_id").bind(&at).bind(&at).bind(id.to_string()).fetch_optional(&mut *connection).await?;
            let location=location.ok_or_else(||AppError::NotFound(format!("Device with ID {id} not found")))?;
            let location=Uuid::parse_str(&location).map_err(|e|AppError::Internal(e.to_string()))?;
            deleted_event(connection,"device",id,"device.deleted",Some(location),json!({"id":id,"locationId":location,"deletedAt":at})).await
        }))).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn provision(
        &self,
        id: Option<Uuid>,
        name: String,
        serial: Option<String>,
        device_type: String,
        sub_type: String,
        location: Option<String>,
        location_id: Uuid,
        fingerprint: String,
        actor: Option<Uuid>,
    ) -> Result<Device, AppError> {
        let id = id.unwrap_or_else(Uuid::now_v7);
        let at = now()?;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            sqlx::query("INSERT INTO devices(id,location_id,name,serial_number,device_type,device_sub_type,location,status,registered_kiosk,registration_status,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,'available',?,'registered',?,?,?,?) ON CONFLICT(id) DO UPDATE SET location_id=excluded.location_id,name=excluded.name,serial_number=excluded.serial_number,device_type=excluded.device_type,device_sub_type=excluded.device_sub_type,location=excluded.location,status='available',registered_kiosk=excluded.registered_kiosk,registration_status='registered',deleted_at=NULL,updated_by=excluded.updated_by,updated_at=excluded.updated_at")
                .bind(id.to_string()).bind(location_id.to_string()).bind(&name).bind(serial).bind(device_type).bind(sub_type).bind(location).bind(fingerprint)
                .bind(actor.map(|v|v.to_string())).bind(actor.map(|v|v.to_string())).bind(&at).bind(&at).execute(&mut *connection).await?;
            event(connection,"device",id,"device.provisioned",Some(location_id),json!({"id":id,"locationId":location_id,"name":name,"updatedAt":at})).await
        }))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Provisioned device disappeared".into()))
    }

    pub async fn update_fingerprint(&self, id: Uuid, fingerprint: String) -> Result<(), AppError> {
        let at = now()?;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            let location:Option<String>=sqlx::query_scalar("UPDATE devices SET registered_kiosk=?,updated_at=? WHERE id=? AND deleted_at IS NULL RETURNING location_id").bind(fingerprint).bind(&at).bind(id.to_string()).fetch_optional(&mut *connection).await?;
            let location=location.ok_or_else(||AppError::NotFound(format!("Device with ID {id} not found")))?;
            let location=Uuid::parse_str(&location).map_err(|e|AppError::Internal(e.to_string()))?;
            event(connection,"device",id,"device.fingerprint_updated",Some(location),json!({"id":id,"locationId":location,"updatedAt":at})).await
        }))).await
    }

    pub async fn has_active_work(&self, id: Uuid) -> Result<bool, AppError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_sessions WHERE device_id=? AND end_time IS NULL AND deleted_at IS NULL UNION ALL SELECT 1 FROM kiosk_orders WHERE device_id=? AND status IN ('pending','preparing'))").bind(id.to_string()).bind(id.to_string()).fetch_one(&self.db.read_pool()?).await?)
    }
}

fn device_filters<'a>(
    query: &mut QueryBuilder<'a, Sqlite>,
    filters: &'a DeviceFilterDto,
    allowed: &[Uuid],
) {
    if !allowed.is_empty() {
        query.push(" AND location_id IN (");
        let mut separated = query.separated(",");
        for id in allowed {
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(")");
    }
    if let Some(id) = filters.location_id {
        query.push(" AND location_id=").push_bind(id.to_string());
    }
    for (column, value) in [
        ("status", filters.status.as_ref()),
        ("device_type", filters.device_type.as_ref()),
        ("device_sub_type", filters.device_sub_type.as_ref()),
    ] {
        if let Some(value) = value {
            query.push(format!(" AND {column}=")).push_bind(value);
        }
    }
    for (column, value) in [
        ("location", filters.location.as_ref()),
        ("name", filters.name.as_ref()),
    ] {
        if let Some(value) = value {
            query
                .push(format!(" AND instr(lower(COALESCE({column},'')),lower("))
                .push_bind(value)
                .push("))>0");
        }
    }
}

const BALANCE_SELECT: &str = r#"
 SELECT unhex(replace(id,'-','')) id,unhex(replace(player_id,'-','')) player_id,
 device_type,device_sub_type,kind,remaining_minutes,expiry_date,window_start,window_end,status,
 unhex(replace(source_plan_id,'-','')) source_plan_id,allowed_days,allowed_months,deduction_profile,
 unhex(replace(created_by,'-','')) created_by,unhex(replace(updated_by,'-','')) updated_by,
 created_at,updated_at,deleted_at FROM player_plan_balances
"#;

#[derive(Clone)]
pub struct TenantBalanceRepository {
    db: Arc<TenantDb>,
}

impl TenantBalanceRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<PlayerPlanBalance>, AppError> {
        Ok(sqlx::query_as(&format!(
            "{BALANCE_SELECT} WHERE id=? AND deleted_at IS NULL"
        ))
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }
    pub async fn find_open_session_ids(
        &self,
        player: Uuid,
    ) -> Result<Option<(Uuid, Uuid)>, AppError> {
        Ok(sqlx::query_as("SELECT unhex(replace(s.id,'-','')),unhex(replace(s.device_id,'-','')) FROM usage_sessions s JOIN player_plan_balances b ON b.id=s.balance_id WHERE b.player_id=? AND s.end_time IS NULL AND s.deleted_at IS NULL ORDER BY s.start_time DESC,s.id DESC LIMIT 1").bind(player.to_string()).fetch_optional(&self.db.read_pool()?).await?)
    }
    pub async fn find_existing_for_scope(
        &self,
        player: Uuid,
        device_type: Option<&str>,
        sub_type: Option<&str>,
        kind: &str,
    ) -> Result<Option<PlayerPlanBalance>, AppError> {
        Ok(sqlx::query_as(&format!("{BALANCE_SELECT} WHERE player_id=? AND device_type IS ? AND device_sub_type IS ? AND kind=? AND deleted_at IS NULL ORDER BY created_at DESC,id DESC LIMIT 1"))
            .bind(player.to_string()).bind(device_type).bind(sub_type).bind(kind).fetch_optional(&self.db.read_pool()?).await?)
    }
    pub async fn list(
        &self,
        filters: &BalanceFilterDto,
    ) -> Result<PaginationResult<PlayerPlanBalanceResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let base="SELECT unhex(replace(b.id,'-','')) id,unhex(replace(b.player_id,'-','')) player_id,b.device_type,b.device_sub_type,b.kind,b.remaining_minutes,b.expiry_date,b.window_start,b.window_end,b.status,unhex(replace(b.source_plan_id,'-','')) source_plan_id,b.allowed_days,b.allowed_months,b.deduction_profile,unhex(replace(b.created_by,'-','')) created_by,unhex(replace(b.updated_by,'-','')) updated_by,b.created_at,b.updated_at,b.deleted_at,u.username player_username,u.first_name player_first_name,u.last_name player_last_name,p.name plan_name,p.plan_type,p.price/10000.0 plan_price,p.time_credits plan_time_credits FROM player_plan_balances b LEFT JOIN users u ON u.id=b.player_id AND u.deleted_at IS NULL LEFT JOIN plans p ON p.id=b.source_plan_id AND p.deleted_at IS NULL WHERE b.deleted_at IS NULL";
        let mut query = QueryBuilder::<Sqlite>::new(base);
        balance_filters(&mut query, filters)?;
        let sort = match filters.sort_by.as_deref() {
            Some("expiryDate") => "b.expiry_date",
            Some("remainingMinutes") => "b.remaining_minutes",
            Some("status") => "b.status",
            _ => "b.created_at",
        };
        let dir = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        query
            .push(format!(" ORDER BY {sort} {dir},b.id {dir} LIMIT "))
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1) * limit);
        let rows: Vec<BalanceRow> = query
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        let mut count = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM player_plan_balances b WHERE b.deleted_at IS NULL",
        );
        balance_filters(&mut count, filters)?;
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(
            rows.into_iter().map(BalanceRow::into_response).collect(),
            total,
            page,
            limit,
        ))
    }

    pub async fn purchase_or_recharge(
        &self,
        dto: &PurchaseBalanceDto,
        actor: Option<Uuid>,
    ) -> Result<PlayerPlanBalance, AppError> {
        let at = Utc::now();
        let at_text = timestamp(&at)?;
        let player = dto.player_id;
        let plan_id = dto.plan_id;
        // API-0027 owns transaction creation. API-0026 may attach wallet ledger
        // rows only to an already-existing tenant-local transaction.
        let transaction = dto.transaction_id.ok_or_else(|| {
            AppError::BadRequest("Tenant balance purchase requires an existing transaction".into())
        })?;
        let db = self.db.clone();
        let balance_id=write(&db,Box::new(move|connection|Box::pin(async move{
            let transaction_valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM transactions WHERE id=? AND player_id=? AND plan_id=? AND transaction_type='plan_purchase' AND deleted_at IS NULL)").bind(transaction.to_string()).bind(player.to_string()).bind(plan_id.to_string()).fetch_one(&mut *connection).await?;
            if !transaction_valid{return Err(AppError::Conflict("Transaction is not a tenant-local purchase for this player and plan".into()));}
            grant_balance_on_connection(
                connection,
                player,
                plan_id,
                transaction,
                actor,
                &at_text,
            )
            .await
        }))).await?;
        self.find_by_id(balance_id)
            .await?
            .ok_or_else(|| AppError::Internal("Balance disappeared".into()))
    }

    pub async fn set_status(&self, id: Uuid, status: &str) -> Result<(), AppError> {
        let at = now()?;
        let status = status.to_owned();
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            let changed=sqlx::query("UPDATE player_plan_balances SET status=?,updated_at=? WHERE id=? AND deleted_at IS NULL AND status<>?").bind(&status).bind(&at).bind(id.to_string()).bind(&status).execute(&mut *connection).await?;
            if changed.rows_affected()>0{event(connection,"balance",id,"balance.updated",None,json!({"id":id,"status":status,"updatedAt":at})).await?;}Ok(())
        }))).await
    }
}

/// Grants a purchased plan while the caller already owns the tenant writer transaction.
///
/// The ledger transaction ID is the idempotency key, so create and status-completion retries
/// cannot grant the same purchase twice.
pub(crate) async fn grant_balance_on_connection(
    connection: &mut SqliteConnection,
    player: Uuid,
    plan_id: Uuid,
    transaction: Uuid,
    actor: Option<Uuid>,
    at_text: &str,
) -> Result<Uuid, AppError> {
    let granted: Option<String> = sqlx::query_scalar(
        "SELECT balance_id FROM player_plan_ledger
         WHERE transaction_id=? ORDER BY created_at,id LIMIT 1",
    )
    .bind(transaction.to_string())
    .fetch_optional(&mut *connection)
    .await?;
    if let Some(balance_id) = granted {
        return Uuid::parse_str(&balance_id).map_err(|error| AppError::Internal(error.to_string()));
    }
    let at = at_text
        .parse::<DateTime<Utc>>()
        .map_err(|error| AppError::Internal(format!("invalid tenant timestamp: {error}")))?;
    let plan: Option<(
        i32,
        i32,
        Option<String>,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        bool,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT validity_days,time_credits,device_type,device_sub_type,plan_type,
                time_window_start,time_window_end,allowed_days,allowed_months,
                dynamic_deduction_enabled,deduction_profile
         FROM plans WHERE id=? AND is_active=1 AND deleted_at IS NULL",
    )
    .bind(plan_id.to_string())
    .fetch_optional(&mut *connection)
    .await?;
    let Some((
        validity,
        minutes,
        device_type,
        sub_type,
        plan_type,
        window_start,
        window_end,
        allowed_days,
        allowed_months,
        dynamic,
        deduction,
    )) = plan
    else {
        return Err(AppError::NotFound(format!(
            "Plan with ID {plan_id} not found"
        )));
    };
    let kind = if plan_type == "weekend_special" {
        plan_kind::HAPPY_HOURS
    } else {
        plan_kind::TIME
    };
    let existing: Option<(String, i32, String, String)> = sqlx::query_as(
        "SELECT id,remaining_minutes,expiry_date,status FROM player_plan_balances
         WHERE player_id=? AND device_type IS ? AND device_sub_type IS ? AND kind=?
           AND deleted_at IS NULL ORDER BY created_at DESC,id DESC LIMIT 1",
    )
    .bind(player.to_string())
    .bind(&device_type)
    .bind(&sub_type)
    .bind(kind)
    .fetch_optional(&mut *connection)
    .await?;
    let fresh = timestamp(&(at + Duration::days(i64::from(validity))))?;
    let (id, after, expiry, reason) = if let Some((id, current, old_expiry, status)) = existing {
        let carry = status == balance_status::ACTIVE && old_expiry.as_str() > at_text;
        let expiry = if kind == plan_kind::HAPPY_HOURS && carry {
            old_expiry
        } else {
            fresh
        };
        let after = if carry {
            current.checked_add(minutes).ok_or_else(|| {
                AppError::Conflict("Plan balance would exceed supported range".into())
            })?
        } else {
            minutes
        };
        sqlx::query(
            "UPDATE player_plan_balances SET remaining_minutes=?,expiry_date=?,
                 source_plan_id=?,deduction_profile=CASE WHEN ? THEN ? ELSE deduction_profile END,
                 status='active',updated_by=COALESCE(?,updated_by),updated_at=? WHERE id=?",
        )
        .bind(after)
        .bind(&expiry)
        .bind(plan_id.to_string())
        .bind(dynamic)
        .bind(deduction)
        .bind(actor.map(|value| value.to_string()))
        .bind(at_text)
        .bind(&id)
        .execute(&mut *connection)
        .await?;
        (
            Uuid::parse_str(&id).map_err(|error| AppError::Internal(error.to_string()))?,
            after,
            expiry,
            if carry {
                ledger_reason::RECHARGE
            } else {
                ledger_reason::PURCHASE
            },
        )
    } else {
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO player_plan_balances(id,player_id,device_type,device_sub_type,kind,
                 remaining_minutes,expiry_date,window_start,window_end,status,source_plan_id,
                 allowed_days,allowed_months,deduction_profile,created_by,updated_by,created_at,
                 updated_at) VALUES(?,?,?,?,?,?,?,?,?,'active',?,?,?,?,?,?,?,?)",
        )
        .bind(id.to_string())
        .bind(player.to_string())
        .bind(device_type)
        .bind(sub_type)
        .bind(kind)
        .bind(minutes)
        .bind(&fresh)
        .bind(window_start)
        .bind(window_end)
        .bind(plan_id.to_string())
        .bind(allowed_days)
        .bind(allowed_months)
        .bind(dynamic.then_some(deduction).flatten())
        .bind(actor.map(|value| value.to_string()))
        .bind(actor.map(|value| value.to_string()))
        .bind(at_text)
        .bind(at_text)
        .execute(&mut *connection)
        .await?;
        (id, minutes, fresh, ledger_reason::PURCHASE)
    };
    append_ledger(
        connection,
        id,
        player,
        minutes,
        reason,
        Some(transaction),
        None,
        after,
        &expiry,
        actor,
        at_text,
    )
    .await?;
    event(
        connection,
        "balance",
        id,
        "balance.updated",
        None,
        json!({"id":id,"playerId":player,"remainingMinutes":after,
               "expiryDate":expiry,"reason":reason,"updatedAt":at_text}),
    )
    .await?;
    Ok(id)
}

fn balance_filters<'a>(
    q: &mut QueryBuilder<'a, Sqlite>,
    f: &'a BalanceFilterDto,
) -> Result<(), AppError> {
    if let Some(v) = f.player_id {
        q.push(" AND b.player_id=").push_bind(v.to_string());
    }
    for (c, v) in [
        ("b.kind", f.kind.as_ref()),
        ("b.status", f.status.as_ref()),
        ("b.device_type", f.device_type.as_ref()),
        ("b.device_sub_type", f.device_sub_type.as_ref()),
    ] {
        if let Some(v) = v {
            q.push(format!(" AND {c}=")).push_bind(v);
        }
    }
    if f.usable_only == Some(true) {
        let now = timestamp(&Utc::now())?;
        q.push(" AND b.expiry_date>")
            .push_bind(now)
            .push(" AND b.remaining_minutes>0");
    }
    Ok(())
}

async fn append_ledger(
    connection: &mut SqliteConnection,
    balance: Uuid,
    player: Uuid,
    delta: i32,
    reason: &str,
    transaction: Option<Uuid>,
    session: Option<Uuid>,
    after: i32,
    expiry: &str,
    actor: Option<Uuid>,
    at: &str,
) -> Result<(), AppError> {
    if delta == 0 {
        return Ok(());
    }
    sqlx::query("INSERT INTO player_plan_ledger(id,balance_id,player_id,delta_minutes,reason,transaction_id,session_id,balance_after,expiry_after,created_at,created_by) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
        .bind(Uuid::now_v7().to_string()).bind(balance.to_string()).bind(player.to_string()).bind(delta).bind(reason)
        .bind(transaction.map(|v|v.to_string())).bind(session.map(|v|v.to_string())).bind(after).bind(expiry).bind(at).bind(actor.map(|v|v.to_string())).execute(connection).await?;
    Ok(())
}

const SESSION_SELECT:&str="SELECT unhex(replace(id,'-','')) id,unhex(replace(balance_id,'-','')) balance_id,unhex(replace(device_id,'-','')) device_id,unhex(replace(shift_id,'-','')) shift_id,start_time,end_time,duration_minutes,time_credits_consumed,wallet_minutes_at_start,unhex(replace(source_plan_id_at_start,'-','')) source_plan_id_at_start,deduction_profile_snapshot,unhex(replace(created_by,'-','')) created_by,unhex(replace(updated_by,'-','')) updated_by,created_at,updated_at,deleted_at FROM usage_sessions";

#[derive(Clone)]
pub struct TenantSessionRepository {
    db: Arc<TenantDb>,
}
pub struct TenantSessionMutation {
    pub session: UsageSession,
    pub balance: PlayerPlanBalance,
    pub player_id: Uuid,
}
impl TenantSessionRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<UsageSession>, AppError> {
        Ok(sqlx::query_as(&format!(
            "{SESSION_SELECT} WHERE id=? AND deleted_at IS NULL"
        ))
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }
    pub async fn location_id(&self, id: Uuid) -> Result<Uuid, AppError> {
        sqlx::query_scalar("SELECT unhex(replace(location_id,'-','')) FROM usage_sessions WHERE id=? AND deleted_at IS NULL")
            .bind(id.to_string()).fetch_optional(&self.db.read_pool()?).await?
            .ok_or_else(|| AppError::NotFound(format!("Session with ID {id} not found")))
    }
    pub async fn find_open_for_player(
        &self,
        player: Uuid,
    ) -> Result<Option<super::session_repo::PlayerOpenSession>, AppError> {
        let row:Option<(Uuid,Uuid,String,DateTime<Utc>,Uuid,i32)>=sqlx::query_as("SELECT unhex(replace(s.id,'-','')),unhex(replace(s.device_id,'-','')),d.name,s.start_time,unhex(replace(s.balance_id,'-','')),b.remaining_minutes FROM usage_sessions s JOIN player_plan_balances b ON b.id=s.balance_id JOIN devices d ON d.id=s.device_id WHERE s.player_id=? AND s.end_time IS NULL AND s.deleted_at IS NULL ORDER BY s.start_time DESC,s.id DESC LIMIT 1").bind(player.to_string()).fetch_optional(&self.db.read_pool()?).await?;
        Ok(row.map(
            |(session_id, device_id, device_name, start_time, balance_id, remaining_minutes)| {
                super::session_repo::PlayerOpenSession {
                    session_id,
                    device_id,
                    device_name,
                    start_time,
                    balance_id,
                    remaining_minutes,
                }
            },
        ))
    }
    pub async fn find_enriched_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<UsageSessionResponse>, AppError> {
        let row = sqlx::query_as::<_, UsageSessionRow>(&format!(
            "{TENANT_SESSION_ENRICHED} WHERE s.id=? AND s.deleted_at IS NULL"
        ))
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?;
        Ok(row.map(UsageSessionRow::into_response))
    }
    pub async fn list(
        &self,
        filters: &SessionFilterDto,
        locations: &[Uuid],
    ) -> Result<PaginationResult<UsageSessionResponse>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let mut q = QueryBuilder::<Sqlite>::new(format!(
            "{TENANT_SESSION_ENRICHED} WHERE s.deleted_at IS NULL"
        ));
        session_filters(&mut q, filters, locations)?;
        let sort = match filters.sort_by.as_deref() {
            Some("endTime") => "s.end_time",
            Some("durationMinutes") => "s.duration_minutes",
            Some("createdAt") => "s.created_at",
            _ => "s.start_time",
        };
        let dir = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        q.push(format!(" ORDER BY {sort} {dir},s.id {dir} LIMIT "))
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1) * limit);
        let rows: Vec<UsageSessionRow> =
            q.build_query_as().fetch_all(&self.db.read_pool()?).await?;
        let mut count = QueryBuilder::<Sqlite>::new(
            "SELECT COUNT(*) FROM usage_sessions s WHERE s.deleted_at IS NULL",
        );
        session_filters(&mut count, filters, locations)?;
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(
            rows.into_iter()
                .map(UsageSessionRow::into_response)
                .collect(),
            total,
            page,
            limit,
        ))
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn start(
        &self,
        player: Uuid,
        balance: Uuid,
        device: Uuid,
        location: Uuid,
        shift: Option<Uuid>,
        start: DateTime<Utc>,
        actor: Option<Uuid>,
        snapshot: Value,
    ) -> Result<TenantSessionMutation, AppError> {
        let id = Uuid::now_v7();
        let at = now()?;
        let start_text = timestamp(&start)?;
        let snapshot_value = snapshot.clone();
        let timezone = snapshot_value
            .get("policyTimezone")
            .cloned()
            .unwrap_or_else(|| Value::String("UTC".into()));
        let snapshot =
            serde_json::to_string(&snapshot).map_err(|e| AppError::BadRequest(e.to_string()))?;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            let wallet:Option<(i32,Option<String>,String,String,String)>=sqlx::query_as("SELECT remaining_minutes,source_plan_id,status,expiry_date,player_id FROM player_plan_balances WHERE id=? AND deleted_at IS NULL").bind(balance.to_string()).fetch_optional(&mut *connection).await?;
            let Some((minutes,source,status,expiry,owner))=wallet else{return Err(AppError::NotFound(format!("Balance with ID {balance} not found")))};
            if owner!=player.to_string(){return Err(AppError::Forbidden("Balance does not belong to this player".into()));}
            if status!="active"||minutes<=0||expiry<=start_text{return Err(AppError::Forbidden("Balance access denied".into()));}
            let device_name:Option<String>=sqlx::query_scalar("SELECT name FROM devices WHERE id=? AND location_id=? AND deleted_at IS NULL AND status IN ('available','operational')").bind(device.to_string()).bind(location.to_string()).fetch_optional(&mut *connection).await?;
            let Some(device_name)=device_name else{return Err(AppError::BadRequest("Device is not available".into()));};
            if let Some(shift) = shift {
                let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM shifts WHERE id=? AND location_id=? AND user_id=? AND status='active' AND clock_out IS NULL)")
                    .bind(shift.to_string()).bind(location.to_string()).bind(actor.map(|id|id.to_string())).fetch_one(&mut *connection).await?;
                if !valid {return Err(AppError::Forbidden("An active shift at this device's location is required".into()));}
            }
            sqlx::query("INSERT INTO usage_sessions(id,player_id,balance_id,device_id,location_id,shift_id,start_time,time_credits_consumed,wallet_minutes_at_start,source_plan_id_at_start,deduction_profile_snapshot,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,0,?,?,?,?,?,?,?)")
                .bind(id.to_string()).bind(player.to_string()).bind(balance.to_string()).bind(device.to_string()).bind(location.to_string()).bind(shift.map(|v|v.to_string())).bind(&start_text).bind(minutes).bind(source.clone()).bind(&snapshot).bind(actor.map(|v|v.to_string())).bind(actor.map(|v|v.to_string())).bind(&at).bind(&at).execute(&mut *connection).await?;
            sqlx::query("UPDATE devices SET status='in_use',updated_at=? WHERE id=?").bind(&at).bind(device.to_string()).execute(&mut *connection).await?;
            event(connection,"session",id,"session.started",Some(location),json!({
                "id":id,"playerId":player,"balanceId":balance,"deviceId":device,
                "deviceName":device_name,"locationId":location,"startTime":start_text,
                "walletMinutesAtStart":minutes,"sourcePlanIdAtStart":source,
                "remainingMinutes":minutes,"deductionProfile":snapshot_value,
                "cafeTimezone":timezone,"updatedAt":at
            })).await?;
            super::TenantNotificationRepository::record_at_on(connection, crate::services::notification_service::RecordNotification {
                kind: crate::models::activity_kind::SESSION_STARTED.into(), title: "Session started".into(),
                summary: Some(format!("Player session on {device_name} · {minutes} min at login")),
                payload: json!({"sessionId":id,"deviceId":device,"locationId":location,"playerId":player,"balanceId":balance,
                    "startTime":start_text,"walletMinutesAtStart":minutes,"sourcePlanIdAtStart":source,
                    "remainingMinutes":minutes,"deductionProfile":snapshot_value,"cafeTimezone":timezone}),
                actor_user_id: actor, entity_type: Some("session".into()), entity_id: Some(id),
                recipients: crate::services::notification_service::Recipients::Users(vec![]),
            }, Some(location)).await?;
            event(connection,"device",device,"device.status_changed",Some(location),json!({"id":device,"status":"in_use","updatedAt":at})).await?;
            Ok(())
        }))).await?;
        self.mutation(id).await
    }
    pub async fn charge(
        &self,
        id: Uuid,
        total: i32,
        end: Option<(DateTime<Utc>, i32, String)>,
        actor: Option<Uuid>,
    ) -> Result<TenantSessionMutation, AppError> {
        let at = now()?;
        let db = self.db.clone();
        write(&db,Box::new(move|connection|Box::pin(async move{
            let row:Option<(String,String,String,String,i32,Option<i32>,String,i32,Option<String>,Option<String>,String)>=sqlx::query_as("SELECT s.balance_id,s.player_id,s.device_id,s.location_id,COALESCE(s.time_credits_consumed,0),s.duration_minutes,s.start_time,s.wallet_minutes_at_start,s.source_plan_id_at_start,s.deduction_profile_snapshot,d.name FROM usage_sessions s JOIN devices d ON d.id=s.device_id WHERE s.id=? AND s.deleted_at IS NULL AND s.end_time IS NULL").bind(id.to_string()).fetch_optional(&mut *connection).await?;
            let Some((balance_text,player_text,device_text,location_text,charged,_,start_text,wallet_at_start,source_plan,deduction_snapshot,device_name))=row else{return Err(AppError::NotFound(format!("Active session with ID {id} not found")))};
            let balance=Uuid::parse_str(&balance_text).map_err(|e|AppError::Internal(e.to_string()))?;let player=Uuid::parse_str(&player_text).map_err(|e|AppError::Internal(e.to_string()))?;
            let device=Uuid::parse_str(&device_text).map_err(|e|AppError::Internal(e.to_string()))?;let location=Uuid::parse_str(&location_text).map_err(|e|AppError::Internal(e.to_string()))?;
            let delta=(total.max(charged)-charged).max(0);
            let wallet:Option<(i32,String)>=sqlx::query_as("SELECT remaining_minutes,expiry_date FROM player_plan_balances WHERE id=? AND deleted_at IS NULL").bind(&balance_text).fetch_optional(&mut *connection).await?;
            let Some((remaining,expiry))=wallet else{return Err(AppError::NotFound(format!("Balance with ID {balance} not found")))};
            let deducted=delta.min(remaining);let after=remaining-deducted;let persisted=charged+deducted;
            if deducted>0{
                sqlx::query("UPDATE player_plan_balances SET remaining_minutes=?,status=CASE WHEN ?=0 THEN 'exhausted' ELSE status END,updated_at=? WHERE id=?").bind(after).bind(after).bind(&at).bind(&balance_text).execute(&mut *connection).await?;
                append_ledger(connection,balance,player,-deducted,ledger_reason::SESSION_USAGE,None,Some(id),after,&expiry,actor,&at).await?;
                event(connection,"balance",balance,"balance.updated",Some(location),json!({"id":balance,"playerId":player,"deviceId":device,"remainingMinutes":after,"sessionId":id,"updatedAt":at})).await?;
            }
            let end=end.or_else(||{
                if after>0{return None;}
                let end_time=Utc::now();
                let start=start_text.parse::<DateTime<Utc>>().ok()?;
                Some((end_time,((end_time-start).num_seconds().max(0) as f64/60.0).ceil() as i32,"auto".to_string()))
            });
            if let Some((end_time,duration,reason))=end{
                let end_text=timestamp(&end_time)?;
                sqlx::query("UPDATE usage_sessions SET end_time=?,duration_minutes=?,time_credits_consumed=?,end_reason=?,updated_by=COALESCE(?,updated_by),updated_at=? WHERE id=?").bind(&end_text).bind(duration).bind(persisted).bind(&reason).bind(actor.map(|v|v.to_string())).bind(&at).bind(id.to_string()).execute(&mut *connection).await?;
                sqlx::query("UPDATE devices SET status='available',updated_at=? WHERE id=?").bind(&at).bind(&device_text).execute(&mut *connection).await?;
                let deduction_profile=deduction_snapshot.as_deref().and_then(|value|serde_json::from_str::<Value>(value).ok()).unwrap_or_else(||json!({}));
                let cafe_timezone=deduction_profile.get("policyTimezone").cloned().unwrap_or_else(||Value::String("UTC".into()));
                event(connection,"session",id,"session.ended",Some(location),json!({
                    "id":id,"playerId":player,"balanceId":balance,"deviceId":device,
                    "deviceName":device_name,"locationId":location,"startTime":start_text,
                    "walletMinutesAtStart":wallet_at_start,"sourcePlanIdAtStart":source_plan,
                    "remainingMinutes":after,"deductionProfile":deduction_profile,
                    "cafeTimezone":cafe_timezone,"endTime":end_text,"reason":reason,"updatedAt":at
                })).await?;
                super::TenantNotificationRepository::record_at_on(connection, crate::services::notification_service::RecordNotification {
                    kind: crate::models::activity_kind::SESSION_ENDED.into(), title: "Session ended".into(), summary: Some(reason.clone()),
                    payload: json!({"sessionId":id,"deviceId":device,"locationId":location,"playerId":player,"balanceId":balance,
                        "remainingMinutes":after,"endTime":end_text,"reason":reason}),
                    actor_user_id: actor, entity_type: Some("session".into()), entity_id: Some(id),
                    recipients: crate::services::notification_service::Recipients::Users(vec![]),
                }, Some(location)).await?;
                event(connection,"device",device,"device.status_changed",Some(location),json!({"id":device,"status":"available","updatedAt":at})).await?;
            }else{
                sqlx::query("UPDATE usage_sessions SET time_credits_consumed=?,updated_at=? WHERE id=?").bind(persisted).bind(&at).bind(id.to_string()).execute(&mut *connection).await?;
                event(connection,"session",id,"session.heartbeat",Some(location),json!({"id":id,"playerId":player,"deviceId":device,"locationId":location,"timeCreditsConsumed":persisted,"remainingMinutes":after,"updatedAt":at})).await?;
            }
            Ok(())
        }))).await?;
        self.mutation(id).await
    }
    async fn mutation(&self, id: Uuid) -> Result<TenantSessionMutation, AppError> {
        let session = self
            .find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Session disappeared".into()))?;
        let balance = TenantBalanceRepository::new(self.db.clone())
            .find_by_id(session.balance_id)
            .await?
            .ok_or_else(|| AppError::Internal("Balance disappeared".into()))?;
        Ok(TenantSessionMutation {
            player_id: balance.player_id,
            session,
            balance,
        })
    }
}

fn session_filters<'a>(
    q: &mut QueryBuilder<'a, Sqlite>,
    f: &'a SessionFilterDto,
    locations: &[Uuid],
) -> Result<(), AppError> {
    if !locations.is_empty() {
        q.push(" AND s.location_id IN (");
        let mut x = q.separated(",");
        for id in locations {
            x.push_bind(id.to_string());
        }
        x.push_unseparated(")");
    }
    for (c, v) in [
        ("s.balance_id", f.balance_id),
        ("s.device_id", f.device_id),
        ("s.player_id", f.player_id),
        ("s.shift_id", f.shift_id),
    ] {
        if let Some(v) = v {
            q.push(format!(" AND {c}=")).push_bind(v.to_string());
        }
    }
    if let Some(active) = f.is_active {
        q.push(if active == 1 {
            " AND s.end_time IS NULL"
        } else {
            " AND s.end_time IS NOT NULL"
        });
    }
    if let Some(v) = f.start_time_from {
        q.push(" AND s.start_time>=").push_bind(timestamp(&v)?);
    }
    if let Some(v) = f.start_time_to {
        q.push(" AND s.start_time<=").push_bind(timestamp(&v)?);
    }
    Ok(())
}

const TENANT_SESSION_ENRICHED:&str="SELECT unhex(replace(s.id,'-','')) id,unhex(replace(s.balance_id,'-','')) balance_id,unhex(replace(s.device_id,'-','')) device_id,unhex(replace(s.shift_id,'-','')) shift_id,s.start_time,s.end_time,s.duration_minutes,s.time_credits_consumed,s.wallet_minutes_at_start,unhex(replace(s.source_plan_id_at_start,'-','')) source_plan_id_at_start,unhex(replace(s.created_by,'-','')) created_by,unhex(replace(s.updated_by,'-','')) updated_by,s.created_at,s.updated_at,s.deleted_at,unhex(replace(b.player_id,'-','')) bal_player_id,b.kind bal_kind,b.remaining_minutes bal_remaining_minutes,b.status bal_status,unhex(replace(b.source_plan_id,'-','')) bal_source_plan_id,u.username player_username,u.first_name player_first_name,u.last_name player_last_name,p.name plan_name,p.plan_type,p.time_credits plan_time_credits,d.name device_name,d.device_type,d.location device_location,d.status device_status,COALESCE(s.deduction_profile_snapshot,b.deduction_profile) bal_deduction_profile,b.expiry_date bal_expiry_date,ps.name plan_start_name,ps.plan_type plan_start_type,ps.time_credits plan_start_time_credits FROM usage_sessions s LEFT JOIN player_plan_balances b ON b.id=s.balance_id AND b.deleted_at IS NULL LEFT JOIN users u ON u.id=b.player_id AND u.deleted_at IS NULL LEFT JOIN plans p ON p.id=b.source_plan_id AND p.deleted_at IS NULL LEFT JOIN plans ps ON ps.id=s.source_plan_id_at_start AND ps.deleted_at IS NULL LEFT JOIN devices d ON d.id=s.device_id AND d.deleted_at IS NULL";

#[derive(Clone)]
pub struct TenantPlayerPlanRepository {
    db: Arc<TenantDb>,
}
impl TenantPlayerPlanRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<PlayerPlan>, AppError> {
        Ok(sqlx::query_as(&format!(
            "{PLAYER_PLAN_SELECT} WHERE id=? AND deleted_at IS NULL"
        ))
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?)
    }
    pub async fn create(
        &self,
        v: &PlayerPlanCreateValues,
        actor: Option<Uuid>,
    ) -> Result<PlayerPlan, AppError> {
        let id = Uuid::now_v7();
        let purchase = timestamp(&v.purchase_date)?;
        let expiry = timestamp(&v.expiry_date)?;
        let at = now()?;
        let player = v.player_id;
        let plan = v.plan_id;
        let usage = v.remaining_usage_count;
        let credits = v.remaining_time_credits;
        let status = v.status.clone();
        let db = self.db.clone();
        write(&db,Box::new(move|c|Box::pin(async move{sqlx::query("INSERT INTO player_plans(id,player_id,plan_id,purchase_date,expiry_date,remaining_usage_count,remaining_time_credits,status,created_by,updated_by,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(player.to_string()).bind(plan.to_string()).bind(&purchase).bind(&expiry).bind(usage).bind(credits).bind(&status).bind(actor.map(|v|v.to_string())).bind(actor.map(|v|v.to_string())).bind(&at).bind(&at).execute(&mut *c).await?;event(c,"player_plan",id,"player_plan.created",None,json!({"id":id,"playerId":player,"planId":plan,"expiryDate":expiry,"updatedAt":at})).await}))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::Internal("Player plan disappeared".into()))
    }
    pub async fn update(
        &self,
        id: Uuid,
        v: &PlayerPlanUpdateValues,
        actor: Option<Uuid>,
    ) -> Result<PlayerPlan, AppError> {
        let at = now()?;
        let activation = v.activation_date.map(|v| timestamp(&v)).transpose()?;
        let values = (
            v.status.clone(),
            v.remaining_time_credits,
            v.remaining_usage_count,
            activation,
        );
        let db = self.db.clone();
        write(&db,Box::new(move|c|Box::pin(async move{let r=sqlx::query("UPDATE player_plans SET status=COALESCE(?,status),remaining_time_credits=COALESCE(?,remaining_time_credits),remaining_usage_count=COALESCE(?,remaining_usage_count),activation_date=COALESCE(?,activation_date),updated_by=COALESCE(?,updated_by),updated_at=? WHERE id=? AND deleted_at IS NULL").bind(values.0).bind(values.1).bind(values.2).bind(values.3).bind(actor.map(|v|v.to_string())).bind(&at).bind(id.to_string()).execute(&mut *c).await?;if r.rows_affected()==0{return Err(AppError::NotFound(format!("Player plan with ID {id} not found")));}event(c,"player_plan",id,"player_plan.updated",None,json!({"id":id,"updatedAt":at})).await}))).await?;
        self.find_by_id(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Player plan with ID {id} not found")))
    }
    pub async fn list(
        &self,
        f: &PlayerPlanFilterDto,
    ) -> Result<PaginationResult<PlayerPlanResponse>, AppError> {
        let page = f.page.unwrap_or(1).max(1);
        let limit = f.limit.unwrap_or(10).clamp(1, 100);
        let base="SELECT unhex(replace(pp.id,'-','')) id,unhex(replace(pp.player_id,'-','')) player_id,unhex(replace(pp.plan_id,'-','')) plan_id,pp.purchase_date,pp.activation_date,pp.expiry_date,pp.remaining_usage_count,pp.remaining_time_credits,pp.status,unhex(replace(pp.moved_to_plan_id,'-','')) moved_to_plan_id,pp.moved_credits_count,unhex(replace(pp.created_by,'-','')) created_by,unhex(replace(pp.updated_by,'-','')) updated_by,pp.created_at,pp.updated_at,pp.deleted_at,u.username player_username,u.first_name player_first_name,u.last_name player_last_name,p.name plan_name,p.plan_type,p.price/10000.0 plan_price,p.time_credits plan_time_credits FROM player_plans pp LEFT JOIN users u ON u.id=pp.player_id AND u.deleted_at IS NULL LEFT JOIN plans p ON p.id=pp.plan_id AND p.deleted_at IS NULL WHERE pp.deleted_at IS NULL";
        let mut q = QueryBuilder::<Sqlite>::new(base);
        player_plan_filters(&mut q, f)?;
        let sort = match f.sort_by.as_deref() {
            Some("expiryDate") => "pp.expiry_date",
            Some("status") => "pp.status",
            Some("createdAt") => "pp.created_at",
            _ => "pp.purchase_date",
        };
        let direction = if f.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        q.push(format!(
            " ORDER BY {sort} {direction},pp.id {direction} LIMIT "
        ))
        .push_bind(limit)
        .push(" OFFSET ")
        .push_bind((page - 1) * limit);
        let rows: Vec<PlayerPlanRow> = q.build_query_as().fetch_all(&self.db.read_pool()?).await?;
        let mut count=QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM player_plans pp LEFT JOIN plans p ON p.id=pp.plan_id WHERE pp.deleted_at IS NULL");
        player_plan_filters(&mut count, f)?;
        let total = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(
            rows.into_iter().map(PlayerPlanRow::into_response).collect(),
            total,
            page,
            limit,
        ))
    }
}
const PLAYER_PLAN_SELECT:&str="SELECT unhex(replace(id,'-','')) id,unhex(replace(player_id,'-','')) player_id,unhex(replace(plan_id,'-','')) plan_id,purchase_date,activation_date,expiry_date,remaining_usage_count,remaining_time_credits,status,unhex(replace(moved_to_plan_id,'-','')) moved_to_plan_id,moved_credits_count,unhex(replace(created_by,'-','')) created_by,unhex(replace(updated_by,'-','')) updated_by,created_at,updated_at,deleted_at FROM player_plans";
fn player_plan_filters<'a>(
    q: &mut QueryBuilder<'a, Sqlite>,
    f: &'a PlayerPlanFilterDto,
) -> Result<(), AppError> {
    if let Some(v) = f.player_id {
        q.push(" AND pp.player_id=").push_bind(v.to_string());
    }
    if let Some(v) = f.plan_id {
        q.push(" AND pp.plan_id=").push_bind(v.to_string());
    }
    if let Some(v) = &f.status {
        q.push(" AND pp.status=").push_bind(v);
    }
    for (c, v, operator) in [
        ("pp.purchase_date", f.purchase_date_from.as_ref(), ">="),
        ("pp.purchase_date", f.purchase_date_to.as_ref(), "<="),
        ("pp.expiry_date", f.expiry_date_from.as_ref(), ">="),
        ("pp.expiry_date", f.expiry_date_to.as_ref(), "<="),
    ] {
        if let Some(v) = v {
            if let Ok(dt) = v.parse::<DateTime<Utc>>() {
                q.push(format!(" AND {c}{operator}"))
                    .push_bind(timestamp(&dt)?);
            }
        }
    }
    if let Some(v) = f.min_remaining_usage_count {
        q.push(" AND pp.remaining_usage_count>=").push_bind(v);
    }
    if let Some(v) = f.min_remaining_time_credits {
        q.push(" AND pp.remaining_time_credits>=").push_bind(v);
    }
    if let Some(v) = f.is_expired {
        let now = timestamp(&Utc::now())?;
        q.push(if v {
            " AND pp.expiry_date<"
        } else {
            " AND pp.expiry_date>="
        })
        .push_bind(now);
    }
    if let Some(v) = &f.device_type {
        q.push(" AND p.device_type=").push_bind(v);
    }
    if let Some(v) = &f.device_sub_type {
        q.push(" AND p.device_sub_type=").push_bind(v);
    }
    Ok(())
}
