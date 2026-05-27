use serde::Serialize;
use utoipa::ToSchema;

use crate::dto::auth_dto::{
    AuthResponseDto, OtpPendingResponse, RegisterResponseDto,
};
pub use crate::models::{
    AssignPlanDto, CreateDeviceDto, CreateDeviceGameDto, CreateFileDto, CreateGameDto,
    CreatePlanDto, CreateProductDto, CreateSessionDto, CreateTransactionDto, CreateUnitDto,
    Device, DeviceFilterDto, DeviceGameFilterDto, DeviceGameResponse, EndSessionDto, FileFilterDto,
    FileRecord, FileWithDownloadUrlDto, Game, GameFilterDto, GenerateDownloadUrlDto,
    GenerateUploadUrlDto, Plan, PlanFilterDto, PlayerPlan, PlayerPlanFilterDto, Product,
    ProductFilterDto, SessionFilterDto, StorageStatsDto, Transaction, TransactionFilterDto, Unit,
    UnitFilterDto, UpdateDeviceDto, UpdateDeviceStatusDto, UpdateFileDto, UpdateGameDto,
    UpdatePlanDto, UpdateProductDto, UpdateUnitDto, UpdateUserDto, UsageSession, User,
    UserFilterDto, ValidationResult,
};
use crate::services::stats_service::{
    DashboardStatsDto, PeriodPairRevenueByPaymentMethod, PeriodPairUsageStats,
};
use crate::services::storage_service::{
    ListObjectsResponse, PresignedDownloadUrlResponse, PresignedUploadUrlResponse,
};

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEnvelope {
    pub status_code: u16,
    pub message: String,
    pub error: String,
    pub timestamp: String,
}

#[derive(Serialize, ToSchema)]
pub struct LegacyHealthResponse {
    pub status: String,
    pub info: serde_json::Value,
    pub error: serde_json::Value,
    pub details: serde_json::Value,
}

macro_rules! success_envelope {
    ($name:ident, $data:ty) => {
        #[derive(Serialize, ToSchema)]
        #[serde(rename_all = "camelCase")]
        pub struct $name {
            pub success: bool,
            pub status_code: u16,
            pub timestamp: String,
            pub data: $data,
        }
    };
}

macro_rules! pagination_envelope {
    ($env:ident, $page:ident, $item:ty) => {
        #[derive(Serialize, ToSchema)]
        #[serde(rename_all = "camelCase")]
        pub struct $page {
            pub data: Vec<$item>,
            pub total: i64,
            pub page: i64,
            pub limit: i64,
            pub total_pages: i64,
        }

        #[derive(Serialize, ToSchema)]
        #[serde(rename_all = "camelCase")]
        pub struct $env {
            pub success: bool,
            pub status_code: u16,
            pub timestamp: String,
            pub data: $page,
        }
    };
}

success_envelope!(HealthEnvelope, crate::handlers::health::HealthData);
success_envelope!(LiveHealthEnvelope, crate::handlers::health::LiveHealthData);
success_envelope!(OtpPendingEnvelope, OtpPendingResponse);
success_envelope!(AuthResponseEnvelope, AuthResponseDto);
success_envelope!(RegisterResponseEnvelope, RegisterResponseDto);
success_envelope!(DashboardStatsEnvelope, DashboardStatsDto);
success_envelope!(RevenueByPaymentMethodEnvelope, PeriodPairRevenueByPaymentMethod);
success_envelope!(UsageStatsEnvelope, PeriodPairUsageStats);
success_envelope!(DeviceEnvelope, Device);
success_envelope!(GameEnvelope, Game);
success_envelope!(PlanEnvelope, Plan);
success_envelope!(ActivePlansEnvelope, Vec<Plan>);
success_envelope!(PlayerPlanEnvelope, PlayerPlan);
success_envelope!(ValidationResultEnvelope, ValidationResult);
success_envelope!(UnitEnvelope, Unit);
success_envelope!(DeviceGameEnvelope, DeviceGameResponse);
success_envelope!(SessionEnvelope, UsageSession);
success_envelope!(TransactionEnvelope, Transaction);
success_envelope!(ProductEnvelope, Product);
success_envelope!(UserEnvelope, User);
success_envelope!(FileEnvelope, FileRecord);
success_envelope!(FileWithDownloadUrlEnvelope, FileWithDownloadUrlDto);
success_envelope!(StorageStatsEnvelope, StorageStatsDto);
success_envelope!(PresignedUploadUrlEnvelope, PresignedUploadUrlResponse);
success_envelope!(PresignedDownloadUrlEnvelope, PresignedDownloadUrlResponse);
success_envelope!(ListObjectsEnvelope, ListObjectsResponse);

pagination_envelope!(DevicePaginationEnvelope, DevicePaginationPage, Device);
pagination_envelope!(GamePaginationEnvelope, GamePaginationPage, Game);
pagination_envelope!(PlanPaginationEnvelope, PlanPaginationPage, Plan);
pagination_envelope!(PlayerPlanPaginationEnvelope, PlayerPlanPaginationPage, PlayerPlan);
pagination_envelope!(UnitPaginationEnvelope, UnitPaginationPage, Unit);
pagination_envelope!(DeviceGamePaginationEnvelope, DeviceGamePaginationPage, DeviceGameResponse);
pagination_envelope!(SessionPaginationEnvelope, SessionPaginationPage, UsageSession);
pagination_envelope!(TransactionPaginationEnvelope, TransactionPaginationPage, Transaction);
pagination_envelope!(ProductPaginationEnvelope, ProductPaginationPage, Product);
pagination_envelope!(UserPaginationEnvelope, UserPaginationPage, User);
pagination_envelope!(FilePaginationEnvelope, FilePaginationPage, FileRecord);
