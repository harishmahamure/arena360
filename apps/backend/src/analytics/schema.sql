CREATE TABLE IF NOT EXISTS transactions_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `playerId` Nullable(UUID),
  `planId` Nullable(UUID),
  `amount` Decimal(18,2),
  `paymentMethod` String,
  `paymentStatus` String,
  `transactionType` String,
  `cashAmount` Nullable(Decimal(18,2)),
  `onlineAmount` Nullable(Decimal(18,2)),
  `paidAmount` Decimal(19,4),
  `transactionDate` DateTime64(6,'UTC'),
  `shiftId` Nullable(UUID),
  `createdBy` Nullable(UUID),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

ALTER TABLE transactions_versions ADD COLUMN IF NOT EXISTS `shiftId` Nullable(UUID);

ALTER TABLE transactions_versions ADD COLUMN IF NOT EXISTS `createdBy` Nullable(UUID);

CREATE OR REPLACE VIEW transactions AS SELECT * EXCEPT (_version, _deleted) FROM transactions_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS credit_settlements_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `playerId` UUID,
  `amount` Decimal(19,4),
  `paymentMethod` String,
  `cashAmount` Nullable(Decimal(19,4)),
  `onlineAmount` Nullable(Decimal(19,4)),
  `settledAt` DateTime64(6,'UTC'),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW credit_settlements AS SELECT * EXCEPT (_version, _deleted) FROM credit_settlements_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS credit_settlement_items_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `settlementId` UUID,
  `transactionId` UUID,
  `amountApplied` Decimal(19,4),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW credit_settlement_items AS SELECT * EXCEPT (_version, _deleted) FROM credit_settlement_items_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS usage_sessions_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `deviceId` UUID,
  `balanceId` UUID,
  `startTime` DateTime64(6,'UTC'),
  `endTime` Nullable(DateTime64(6,'UTC')),
  `durationMinutes` Nullable(Int64),
  `shiftId` Nullable(UUID),
  `createdBy` Nullable(UUID),
  `sourcePlanIdAtStart` Nullable(UUID),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

ALTER TABLE usage_sessions_versions ADD COLUMN IF NOT EXISTS `shiftId` Nullable(UUID);

ALTER TABLE usage_sessions_versions ADD COLUMN IF NOT EXISTS `createdBy` Nullable(UUID);

ALTER TABLE usage_sessions_versions ADD COLUMN IF NOT EXISTS `sourcePlanIdAtStart` Nullable(UUID);

CREATE OR REPLACE VIEW usage_sessions AS SELECT * EXCEPT (_version, _deleted) FROM usage_sessions_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS users_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `username` String,
  `firstName` Nullable(String),
  `lastName` Nullable(String),
  `role` String,
  `isActive` Bool,
  `creditLimit` Decimal(19,4),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW users AS SELECT * EXCEPT (_version, _deleted) FROM users_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS plans_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `name` String,
  `price` Nullable(Decimal(18,2)),
  `timeCredits` Nullable(Int64),
  `planType` Nullable(String),
  `validityDays` Nullable(Int64),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

ALTER TABLE plans_versions ADD COLUMN IF NOT EXISTS `price` Nullable(Decimal(18,2));

ALTER TABLE plans_versions ADD COLUMN IF NOT EXISTS `timeCredits` Nullable(Int64);

ALTER TABLE plans_versions ADD COLUMN IF NOT EXISTS `planType` Nullable(String);

ALTER TABLE plans_versions ADD COLUMN IF NOT EXISTS `validityDays` Nullable(Int64);

CREATE OR REPLACE VIEW plans AS SELECT * EXCEPT (_version, _deleted) FROM plans_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS player_plan_balances_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `playerId` UUID,
  `status` String,
  `kind` Nullable(String),
  `remainingMinutes` Nullable(Int64),
  `expiryDate` Nullable(DateTime64(6,'UTC')),
  `sourcePlanId` Nullable(UUID),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

ALTER TABLE player_plan_balances_versions ADD COLUMN IF NOT EXISTS `kind` Nullable(String);

ALTER TABLE player_plan_balances_versions ADD COLUMN IF NOT EXISTS `remainingMinutes` Nullable(Int64);

ALTER TABLE player_plan_balances_versions ADD COLUMN IF NOT EXISTS `expiryDate` Nullable(DateTime64(6,'UTC'));

ALTER TABLE player_plan_balances_versions ADD COLUMN IF NOT EXISTS `sourcePlanId` Nullable(UUID);

CREATE OR REPLACE VIEW player_plan_balances AS SELECT * EXCEPT (_version, _deleted) FROM player_plan_balances_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS devices_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `name` String,
  `status` String,
  `location` Nullable(String),
  `deviceType` Nullable(String),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

ALTER TABLE devices_versions ADD COLUMN IF NOT EXISTS `location` Nullable(String);

ALTER TABLE devices_versions ADD COLUMN IF NOT EXISTS `deviceType` Nullable(String);

CREATE OR REPLACE VIEW devices AS SELECT * EXCEPT (_version, _deleted) FROM devices_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS cash_registers_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `shiftId` UUID,
  `status` String,
  `variance` Nullable(Decimal(19,4)),
  `closingBalance` Nullable(Decimal(19,4)),
  `expectedClosing` Nullable(Decimal(19,4)),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW cash_registers AS SELECT * EXCEPT (_version, _deleted) FROM cash_registers_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS cash_deposits_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `status` String,
  `amount` Decimal(19,4),
  `depositType` Nullable(String),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW cash_deposits AS SELECT * EXCEPT (_version, _deleted) FROM cash_deposits_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS expenses_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `categoryId` UUID,
  `amount` Decimal(19,4),
  `approvalStatus` String,
  `expenseDate` DateTime64(6,'UTC'),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW expenses AS SELECT * EXCEPT (_version, _deleted) FROM expenses_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS expense_categories_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `name` String,
  `isActive` Bool,
  `budgetAmount` Nullable(Decimal(19,4)),
  `budgetPeriod` Nullable(String),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW expense_categories AS SELECT * EXCEPT (_version, _deleted) FROM expense_categories_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS products_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `name` String,
  `purchasePricePerBox` Nullable(Decimal(19,4)),
  `purchasePrice` Nullable(Decimal(19,4)),
  `unitsPerPurchaseUnit` Nullable(Int64),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW products AS SELECT * EXCEPT (_version, _deleted) FROM products_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS vendors_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `name` String,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW vendors AS SELECT * EXCEPT (_version, _deleted) FROM vendors_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS inventory_locations_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `name` String,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW inventory_locations AS SELECT * EXCEPT (_version, _deleted) FROM inventory_locations_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS stock_receipts_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `vendorId` Nullable(UUID),
  `locationId` UUID,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW stock_receipts AS SELECT * EXCEPT (_version, _deleted) FROM stock_receipts_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS stock_receipt_lines_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `receiptId` UUID,
  `productId` UUID,
  `boxQuantity` Int64,
  `piecesAdded` Int64,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW stock_receipt_lines AS SELECT * EXCEPT (_version, _deleted) FROM stock_receipt_lines_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS stock_waste_events_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `locationId` UUID,
  `status` String,
  `approvedAt` Nullable(DateTime64(6,'UTC')),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW stock_waste_events AS SELECT * EXCEPT (_version, _deleted) FROM stock_waste_events_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS stock_waste_lines_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `wasteEventId` UUID,
  `productId` UUID,
  `reasonCode` String,
  `quantityPieces` Int64,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW stock_waste_lines AS SELECT * EXCEPT (_version, _deleted) FROM stock_waste_lines_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS location_stock_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `locationId` UUID,
  `productId` UUID,
  `quantityPieces` Int64,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW location_stock AS SELECT * EXCEPT (_version, _deleted) FROM location_stock_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS inventory_reorder_rules_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `locationId` UUID,
  `productId` UUID,
  `isActive` Bool,
  `minimumPieces` Int64,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW inventory_reorder_rules AS SELECT * EXCEPT (_version, _deleted) FROM inventory_reorder_rules_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS purchase_orders_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `status` String,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW purchase_orders AS SELECT * EXCEPT (_version, _deleted) FROM purchase_orders_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS stock_transfer_requests_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `status` String,
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW stock_transfer_requests AS SELECT * EXCEPT (_version, _deleted) FROM stock_transfer_requests_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS stock_movements_versions (
  `id` UUID,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  `locationId` UUID,
  `productId` UUID,
  `delta` Int64,
  `movementType` String,
  `referenceId` Nullable(UUID),
  `referenceType` Nullable(String),
  `createdBy` Nullable(UUID),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW stock_movements AS SELECT * EXCEPT (_version, _deleted) FROM stock_movements_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS transaction_products_versions (
  `id` UUID,
  `transactionId` UUID,
  `productId` UUID,
  `quantity` Int64,
  `unitPrice` Decimal(18,2),
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW transaction_products AS SELECT * EXCEPT (_version, _deleted) FROM transaction_products_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS shifts_versions (
  `id` UUID,
  `userId` UUID,
  `clockIn` DateTime64(6,'UTC'),
  `clockOut` Nullable(DateTime64(6,'UTC')),
  `status` String,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW shifts AS SELECT * EXCEPT (_version, _deleted) FROM shifts_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS games_versions (
  `id` UUID,
  `name` String,
  `isActive` Bool,
  `createdAt` Nullable(DateTime64(6,'UTC')),
  `updatedAt` Nullable(DateTime64(6,'UTC')),
  `deletedAt` Nullable(DateTime64(6,'UTC')),
  _version UInt64,
  _deleted UInt8
) ENGINE = ReplacingMergeTree(_version) ORDER BY id;

CREATE OR REPLACE VIEW games AS SELECT * EXCEPT (_version, _deleted) FROM games_versions FINAL WHERE _deleted = 0;

CREATE TABLE IF NOT EXISTS analytics_ready (id UInt8, completed_at DateTime64(6, 'UTC')) ENGINE = ReplacingMergeTree(completed_at) ORDER BY id;
