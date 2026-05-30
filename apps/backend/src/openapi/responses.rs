use serde::Serialize;
use utoipa::ToSchema;

use crate::dto::auth_dto::{AuthResponseDto, OtpPendingResponse, RegisterResponseDto};
pub use crate::models::{
    AssignPlanDto, CreateDeviceDto, CreatePlanDto, CreateProductDto, CreateSessionDto,
    CreateTransactionDto, CreateUnitDto, Device, DeviceFilterDto, EndSessionDto, Plan,
    PlanFilterDto, PlayerPlan, PlayerPlanFilterDto, PlayerPlanResponse, Product, ProductFilterDto,
    SessionFilterDto, Transaction, TransactionFilterDto, Unit, UnitFilterDto, UpdateDeviceDto,
    UpdateDeviceStatusDto, UpdatePlanDto, UpdateProductDto, UpdateUnitDto, UpdateUserDto,
    UsageSession, UsageSessionResponse, User, UserFilterDto, ValidationResult,
};
use crate::services::stats_service::{
    DashboardStatsDto, PeriodPairRevenueByPaymentMethod, PeriodPairUsageStats,
};

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEnvelope {
    pub status_code: u16,
    pub message: String,
    pub error: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
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
success_envelope!(
    StaffDashboardStatsEnvelope,
    crate::services::stats_service::StaffDashboardStatsDto
);
success_envelope!(
    RevenueByPaymentMethodEnvelope,
    PeriodPairRevenueByPaymentMethod
);
success_envelope!(UsageStatsEnvelope, PeriodPairUsageStats);
success_envelope!(DeviceEnvelope, Device);
success_envelope!(PlanEnvelope, Plan);
success_envelope!(ActivePlansEnvelope, Vec<Plan>);
success_envelope!(PlayerPlanEnvelope, PlayerPlanResponse);
success_envelope!(PlayerPlanFlatEnvelope, PlayerPlan);
success_envelope!(ValidationResultEnvelope, ValidationResult);
success_envelope!(
    BalanceEnvelope,
    crate::models::PlayerPlanBalanceResponse
);
success_envelope!(
    BalanceFlatEnvelope,
    crate::models::PlayerPlanBalance
);
success_envelope!(
    BalanceValidationEnvelope,
    crate::models::BalanceValidationResult
);
success_envelope!(UnitEnvelope, Unit);
success_envelope!(SessionEnvelope, UsageSessionResponse);
success_envelope!(SessionFlatEnvelope, UsageSession);
success_envelope!(TransactionEnvelope, Transaction);
success_envelope!(
    TransactionWithLineItemsEnvelope,
    crate::models::TransactionWithLineItems
);
success_envelope!(ProductEnvelope, Product);
success_envelope!(UserEnvelope, User);
success_envelope!(ConfigurationEnvelope, crate::models::Configuration);
success_envelope!(ConfigurationListEnvelope, Vec<crate::models::Configuration>);
success_envelope!(ShiftEnvelope, crate::models::Shift);
success_envelope!(ShiftActiveEnvelope, Option<crate::models::Shift>);

pagination_envelope!(DevicePaginationEnvelope, DevicePaginationPage, Device);
pagination_envelope!(PlanPaginationEnvelope, PlanPaginationPage, Plan);
pagination_envelope!(
    PlayerPlanPaginationEnvelope,
    PlayerPlanPaginationPage,
    PlayerPlanResponse
);
pagination_envelope!(
    BalancePaginationEnvelope,
    BalancePaginationPage,
    crate::models::PlayerPlanBalanceResponse
);
pagination_envelope!(UnitPaginationEnvelope, UnitPaginationPage, Unit);
pagination_envelope!(
    SessionPaginationEnvelope,
    SessionPaginationPage,
    UsageSessionResponse
);
pagination_envelope!(
    ShiftPaginationEnvelope,
    ShiftPaginationPage,
    crate::models::Shift
);
pagination_envelope!(
    TransactionPaginationEnvelope,
    TransactionPaginationPage,
    Transaction
);
pagination_envelope!(ProductPaginationEnvelope, ProductPaginationPage, Product);
pagination_envelope!(UserPaginationEnvelope, UserPaginationPage, User);

use crate::models::{
    CashRegister, CashRegisterEntry, CashRegisterWithEntries, Expense, ExpenseCategory,
    ExpenseSummaryDto, Vendor,
};

success_envelope!(CashRegisterEnvelope, CashRegister);
success_envelope!(CashRegisterWithEntriesEnvelope, CashRegisterWithEntries);
success_envelope!(CashRegisterEntryEnvelope, CashRegisterEntry);
success_envelope!(CashRegisterEntriesEnvelope, Vec<CashRegisterEntry>);
pagination_envelope!(
    CashRegisterPaginationEnvelope,
    CashRegisterPaginationPage,
    CashRegister
);

success_envelope!(ExpenseCategoryEnvelope, ExpenseCategory);
pagination_envelope!(
    ExpenseCategoryPaginationEnvelope,
    ExpenseCategoryPaginationPage,
    ExpenseCategory
);

success_envelope!(VendorEnvelope, Vendor);
pagination_envelope!(VendorPaginationEnvelope, VendorPaginationPage, Vendor);

success_envelope!(ExpenseEnvelope, Expense);
success_envelope!(ExpenseSummaryListEnvelope, Vec<ExpenseSummaryDto>);
pagination_envelope!(ExpensePaginationEnvelope, ExpensePaginationPage, Expense);

success_envelope!(TotpSetupEnvelope, crate::models::TotpSetupResponseDto);
success_envelope!(
    ShiftHandoverResponseEnvelope,
    crate::models::ShiftHandoverResponseDto
);
success_envelope!(
    ShiftCloseResponseEnvelope,
    crate::models::ShiftCloseResponseDto
);
success_envelope!(
    ExpectedClosingEnvelope,
    crate::handlers::cash_registers::ExpectedClosingResponse
);

use crate::models::CashDeposit;

success_envelope!(CashDepositEnvelope, CashDeposit);
pagination_envelope!(
    CashDepositPaginationEnvelope,
    CashDepositPaginationPage,
    CashDeposit
);

success_envelope!(CreditSummaryEnvelope, crate::models::CreditSummary);
success_envelope!(PlayerCreditDetailEnvelope, crate::models::PlayerCreditDetail);
success_envelope!(CreditSettlementEnvelope, crate::models::CreditSettlement);
pagination_envelope!(
    CreditPlayerPaginationEnvelope,
    CreditPlayerPaginationPage,
    crate::models::CreditPlayerRow
);
