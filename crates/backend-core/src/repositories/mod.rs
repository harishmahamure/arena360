pub mod tenant_catalog_repo;
pub mod tenant_commerce_repo;
pub mod tenant_user_repo;
pub mod tenant_venue_repo;

pub use tenant_catalog_repo::{
    TenantGameRepository, TenantPlanCreateValues, TenantPlanRepository,
    TenantPricingPolicyRepository, TenantProductRecipeRepository, TenantProductRepository,
    TenantSettingsRepository, TenantUnitRepository,
};
pub use tenant_commerce_repo::{
    TenantCreditRepository, TenantKioskOrderRepository, TenantTransactionRepository,
};
pub use tenant_user_repo::{
    StaffProjectionResult, TenantCreatePlayer, TenantLocationRoleGrant, TenantStaffProjection,
    TenantUserRepository,
};
pub use tenant_venue_repo::{
    TenantBalanceRepository, TenantDeviceRepository, TenantPlayerPlanRepository,
    TenantSessionMutation, TenantSessionRepository,
};

pub(crate) mod tenant_back_office;
pub mod tenant_inventory_repo;
pub mod tenant_vendor_repo;
pub use tenant_inventory_repo::TenantInventoryRepository;
pub use tenant_vendor_repo::TenantVendorRepository;

pub mod tenant_finance_repo;
pub use tenant_finance_repo::{TenantShiftRepository,TenantCashRegisterRepository,TenantCashDepositRepository,TenantExpenseRepository,TenantExpenseCategoryRepository};

pub mod tenant_config_repo;
pub mod tenant_notification_repo;
pub use tenant_config_repo::TenantConfigRepository;
pub use tenant_notification_repo::TenantNotificationRepository;

mod tenant_settings_access;

pub mod tenant_access_repo;
pub use tenant_access_repo::{TenantAccessRepository,TenantRoleDto,TenantMemberDto,TenantLocationRoleDto};

pub(crate) mod tenant_activity;
