use sqlx::{PgPool, QueryBuilder, Postgres};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{CreateDeviceDto, Device, DeviceFilterDto, UpdateDeviceDto};

#[derive(Clone)]
pub struct DeviceRepository {
    pool: PgPool,
}

impl DeviceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const SELECT: &'static str = r#"
        SELECT id, name, "serialNumber" as serial_number,
               "localIpAddress" as local_ip_address,
               "deviceType" as device_type, "deviceSubType" as device_sub_type,
               location, status, "registeredKiosk" as registered_kiosk,
               "registrationStatus" as registration_status,
               "createdAt" as created_at, "updatedAt" as updated_at,
               "deletedAt" as deleted_at
        FROM devices
    "#;

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Device>, AppError> {
        let query = format!("{} WHERE id = $1 AND \"deletedAt\" IS NULL", Self::SELECT);
        let device = sqlx::query_as::<_, Device>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(device)
    }

    pub async fn list(&self, filters: &DeviceFilterDto) -> Result<PaginationResult<Device>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(10).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT id, name, \"serialNumber\" as serial_number, \"localIpAddress\" as local_ip_address, \
             \"deviceType\" as device_type, \"deviceSubType\" as device_sub_type, location, status, \
             \"registeredKiosk\" as registered_kiosk, \"registrationStatus\" as registration_status, \
             \"createdAt\" as created_at, \"updatedAt\" as updated_at, \"deletedAt\" as deleted_at \
             FROM devices WHERE \"deletedAt\" IS NULL",
        );

        if let Some(status) = &filters.status {
            builder.push(" AND status = ");
            builder.push_bind(status);
        }
        if let Some(device_type) = &filters.device_type {
            builder.push(" AND \"deviceType\" = ");
            builder.push_bind(device_type);
        }
        if let Some(device_sub_type) = &filters.device_sub_type {
            builder.push(" AND \"deviceSubType\" = ");
            builder.push_bind(device_sub_type);
        }
        if let Some(location) = &filters.location {
            builder.push(" AND location ILIKE ");
            builder.push_bind(format!("%{location}%"));
        }
        if let Some(name) = &filters.name {
            builder.push(" AND name ILIKE ");
            builder.push_bind(format!("%{name}%"));
        }

        let sort_by = filters.sort_by.as_deref().unwrap_or("createdAt");
        let sort_col = match sort_by {
            "name" => "name",
            "status" => "status",
            _ => "\"createdAt\"",
        };
        let sort_order = if filters.sort_order.as_deref() == Some("ASC") {
            "ASC"
        } else {
            "DESC"
        };
        builder.push(format!(" ORDER BY {sort_col} {sort_order} LIMIT "));
        builder.push_bind(limit);
        builder.push(" OFFSET ");
        builder.push_bind(offset);

        let devices = builder.build_query_as::<Device>().fetch_all(&self.pool).await?;

        let mut count_builder: QueryBuilder<Postgres> =
            QueryBuilder::new("SELECT COUNT(*) FROM devices WHERE \"deletedAt\" IS NULL");
        if let Some(status) = &filters.status {
            count_builder.push(" AND status = ");
            count_builder.push_bind(status);
        }
        if let Some(device_type) = &filters.device_type {
            count_builder.push(" AND \"deviceType\" = ");
            count_builder.push_bind(device_type);
        }
        if let Some(device_sub_type) = &filters.device_sub_type {
            count_builder.push(" AND \"deviceSubType\" = ");
            count_builder.push_bind(device_sub_type);
        }
        if let Some(location) = &filters.location {
            count_builder.push(" AND location ILIKE ");
            count_builder.push_bind(format!("%{location}%"));
        }
        if let Some(name) = &filters.name {
            count_builder.push(" AND name ILIKE ");
            count_builder.push_bind(format!("%{name}%"));
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        Ok(PaginationResult::new(devices, total.0, page, limit))
    }

    pub async fn create(&self, dto: &CreateDeviceDto) -> Result<Device, AppError> {
        let device = sqlx::query_as::<_, Device>(
            r#"
            INSERT INTO devices (
                id, name, "serialNumber", "localIpAddress", "deviceType", "deviceSubType",
                location, status, "registrationStatus", "createdAt", "updatedAt"
            )
            VALUES (
                gen_random_uuid(), $1, $2, $3,
                COALESCE($4, 'other'), COALESCE($5, 'other'),
                $6, COALESCE($7, 'available'), COALESCE($8, 'unregistered'), NOW(), NOW()
            )
            RETURNING id, name, "serialNumber" as serial_number,
                      "localIpAddress" as local_ip_address,
                      "deviceType" as device_type, "deviceSubType" as device_sub_type,
                      location, status, "registeredKiosk" as registered_kiosk,
                      "registrationStatus" as registration_status,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(&dto.name)
        .bind(&dto.serial_number)
        .bind(&dto.local_ip_address)
        .bind(&dto.device_type)
        .bind(&dto.device_sub_type)
        .bind(&dto.location)
        .bind(&dto.status)
        .bind(&dto.registration_status)
        .fetch_one(&self.pool)
        .await?;

        Ok(device)
    }

    pub async fn update(&self, id: Uuid, dto: &UpdateDeviceDto) -> Result<Device, AppError> {
        let device = sqlx::query_as::<_, Device>(
            r#"
            UPDATE devices SET
                name = COALESCE($2, name),
                "serialNumber" = COALESCE($3, "serialNumber"),
                "localIpAddress" = COALESCE($4, "localIpAddress"),
                "deviceType" = COALESCE($5, "deviceType"),
                "deviceSubType" = COALESCE($6, "deviceSubType"),
                location = COALESCE($7, location),
                status = COALESCE($8, status),
                "registrationStatus" = COALESCE($9, "registrationStatus"),
                "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, "serialNumber" as serial_number,
                      "localIpAddress" as local_ip_address,
                      "deviceType" as device_type, "deviceSubType" as device_sub_type,
                      location, status, "registeredKiosk" as registered_kiosk,
                      "registrationStatus" as registration_status,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(&dto.name)
        .bind(&dto.serial_number)
        .bind(&dto.local_ip_address)
        .bind(&dto.device_type)
        .bind(&dto.device_sub_type)
        .bind(&dto.location)
        .bind(&dto.status)
        .bind(&dto.registration_status)
        .fetch_optional(&self.pool)
        .await?;

        device.ok_or_else(|| AppError::NotFound(format!("Device with ID {id} not found")))
    }

    pub async fn update_status(&self, id: Uuid, status: &str) -> Result<Device, AppError> {
        let device = sqlx::query_as::<_, Device>(
            r#"
            UPDATE devices SET status = $2, "updatedAt" = NOW()
            WHERE id = $1 AND "deletedAt" IS NULL
            RETURNING id, name, "serialNumber" as serial_number,
                      "localIpAddress" as local_ip_address,
                      "deviceType" as device_type, "deviceSubType" as device_sub_type,
                      location, status, "registeredKiosk" as registered_kiosk,
                      "registrationStatus" as registration_status,
                      "createdAt" as created_at, "updatedAt" as updated_at,
                      "deletedAt" as deleted_at
            "#,
        )
        .bind(id)
        .bind(status)
        .fetch_optional(&self.pool)
        .await?;

        device.ok_or_else(|| AppError::NotFound(format!("Device with ID {id} not found")))
    }

    pub async fn soft_delete(&self, id: Uuid) -> Result<(), AppError> {
        let result = sqlx::query(
            r#"UPDATE devices SET "deletedAt" = NOW() WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Device with ID {id} not found")));
        }
        Ok(())
    }

    pub async fn name_exists(&self, name: &str, exclude_id: Option<Uuid>) -> Result<bool, AppError> {
        let exists: (bool,) = match exclude_id {
            Some(id) => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM devices WHERE LOWER(name) = LOWER($1) AND id != $2 AND "deletedAt" IS NULL)"#,
                )
                .bind(name)
                .bind(id)
                .fetch_one(&self.pool)
                .await?
            }
            None => {
                sqlx::query_as(
                    r#"SELECT EXISTS(SELECT 1 FROM devices WHERE LOWER(name) = LOWER($1) AND "deletedAt" IS NULL)"#,
                )
                .bind(name)
                .fetch_one(&self.pool)
                .await?
            }
        };
        Ok(exists.0)
    }
}
