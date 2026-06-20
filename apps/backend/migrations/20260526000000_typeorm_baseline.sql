-- TypeORM baseline schema for fresh Postgres (docker-compose / greenfield).
-- Captured from legacy NestJS schema; later Rust migrations assume these tables exist.
-- Safe to skip on databases that already have the TypeORM layout.

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

DO $$ BEGIN CREATE TYPE devices_devicesubtype_enum AS ENUM (
  'HIGH_END_PCS', 'MID_RANGE_PCS', 'PREMIUM_TV_CONSOLES', 'STANDARD_TV_CONSOLES', 'OTHER'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE devices_devicetype_enum AS ENUM (
  'PC', 'CONSOLE', 'PS5', 'PS4', 'OTHER'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE devices_registrationstatus_enum AS ENUM (
  'registered', 'unregistered'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE devices_status_enum AS ENUM (
  'operational', 'under_maintenance', 'out_of_service', 'in_use', 'available'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE plans_devicesubtype_enum AS ENUM (
  'HIGH_END_PCS', 'MID_RANGE_PCS', 'PREMIUM_TV_CONSOLES', 'STANDARD_TV_CONSOLES', 'OTHER'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE plans_devicetype_enum AS ENUM (
  'PC', 'CONSOLE', 'PS5', 'PS4', 'OTHER'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE plans_plantype_enum AS ENUM (
  'time_based', 'session_based', 'unlimited_daily', 'hourly_rental', 'monthly_subscription', 'weekend_special'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE player_plans_status_enum AS ENUM (
  'active', 'expired', 'exhausted', 'cancelled', 'moved_to_next_plan'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE products_category_enum AS ENUM (
  'beverage', 'snack', 'meal', 'other'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE transactions_paymentmethod_enum AS ENUM (
  'cash', 'online', 'split_payment', 'credit'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE transactions_paymentstatus_enum AS ENUM (
  'pending', 'completed', 'failed', 'refunded', 'credit'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE transactions_transactiontype_enum AS ENUM (
  'plan_purchase', 'product_purchase'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE units_type_enum AS ENUM (
  'piece', 'box', 'carton', 'pack', 'bottle', 'can', 'kilogram', 'gram', 'liter', 'milliliter', 'other'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE files_category_enum AS ENUM (
  'image', 'video', 'document', 'other'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE files_status_enum AS ENUM (
  'active', 'archived', 'deleted'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

DO $$ BEGIN CREATE TYPE files_visibility_enum AS ENUM (
  'public', 'private', 'internal'
); EXCEPTION WHEN duplicate_object THEN NULL; END $$;

CREATE TABLE IF NOT EXISTS users (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  email VARCHAR(255),
  username VARCHAR(100) NOT NULL,
  "isActive" BOOLEAN NOT NULL DEFAULT TRUE,
  "firstName" VARCHAR(50),
  "lastName" VARCHAR(50),
  role VARCHAR(20) NOT NULL DEFAULT 'player',
  password_hash VARCHAR(255) NOT NULL,
  "sessionOtpId" VARCHAR(255),
  "sessionOtp" VARCHAR(255),
  "phoneNumber" VARCHAR(255),
  CONSTRAINT "UQ_users_username" UNIQUE (username)
);

CREATE TABLE IF NOT EXISTS devices (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  name VARCHAR(100) NOT NULL,
  "deviceType" devices_devicetype_enum NOT NULL DEFAULT 'OTHER',
  location VARCHAR(200),
  status devices_status_enum NOT NULL DEFAULT 'available',
  "serialNumber" VARCHAR(100),
  "localIpAddress" VARCHAR(100),
  "registeredKiosk" VARCHAR,
  "registrationStatus" devices_registrationstatus_enum NOT NULL DEFAULT 'unregistered',
  "deviceSubType" devices_devicesubtype_enum NOT NULL DEFAULT 'OTHER',
  CONSTRAINT "UQ_devices_name" UNIQUE (name)
);

CREATE TABLE IF NOT EXISTS plans (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  name VARCHAR(100) NOT NULL,
  description TEXT,
  price NUMERIC(10,2) NOT NULL,
  "planType" plans_plantype_enum NOT NULL,
  "durationMinutes" INTEGER NOT NULL DEFAULT 60,
  "validityDays" INTEGER NOT NULL DEFAULT 30,
  "timeWindowStart" TIME,
  "timeWindowEnd" TIME,
  "timeCredits" INTEGER NOT NULL DEFAULT 60,
  "perMinuteRate" NUMERIC(5,2) NOT NULL DEFAULT 1,
  "maxSessions" INTEGER,
  "isActive" BOOLEAN NOT NULL DEFAULT TRUE,
  "deviceType" plans_devicetype_enum,
  "deviceSubType" plans_devicesubtype_enum
);

CREATE TABLE IF NOT EXISTS units (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  name VARCHAR(100) NOT NULL,
  abbreviation VARCHAR(20) NOT NULL,
  type units_type_enum NOT NULL DEFAULT 'other',
  description TEXT,
  "isActive" BOOLEAN NOT NULL DEFAULT TRUE,
  CONSTRAINT "UQ_units_name" UNIQUE (name),
  CONSTRAINT "UQ_units_abbreviation" UNIQUE (abbreviation)
);

CREATE TABLE IF NOT EXISTS products (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  name VARCHAR(255) NOT NULL,
  description TEXT,
  price NUMERIC(10,2) NOT NULL,
  category products_category_enum NOT NULL DEFAULT 'other',
  sku VARCHAR(50),
  "stockQuantity" INTEGER NOT NULL DEFAULT 0,
  "isActive" BOOLEAN NOT NULL DEFAULT TRUE,
  "purchasePrice" NUMERIC(10,2),
  "purchaseUnitId" UUID REFERENCES units(id),
  "sellUnitId" UUID REFERENCES units(id),
  "conversionFactor" NUMERIC(10,4) NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS player_plans (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  "playerId" UUID NOT NULL REFERENCES users(id),
  "planId" UUID NOT NULL REFERENCES plans(id),
  "purchaseDate" TIMESTAMPTZ NOT NULL,
  "activationDate" TIMESTAMPTZ,
  "expiryDate" TIMESTAMPTZ NOT NULL,
  "remainingUsageCount" INTEGER,
  "remainingTimeCredits" INTEGER,
  status player_plans_status_enum NOT NULL DEFAULT 'active',
  "movedToPlanId" UUID,
  "movedCreditsCount" INTEGER
);

CREATE TABLE IF NOT EXISTS transactions (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  "playerId" UUID NOT NULL REFERENCES users(id),
  "planId" UUID REFERENCES plans(id),
  amount NUMERIC(10,2) NOT NULL,
  "paymentMethod" transactions_paymentmethod_enum NOT NULL,
  "paymentStatus" transactions_paymentstatus_enum NOT NULL DEFAULT 'pending',
  notes TEXT,
  "transactionDate" TIMESTAMPTZ NOT NULL,
  "transactionType" transactions_transactiontype_enum NOT NULL,
  "cashAmount" NUMERIC(10,2),
  "onlineAmount" NUMERIC(10,2)
);

CREATE TABLE IF NOT EXISTS usage_sessions (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  "playerPlanId" UUID REFERENCES player_plans(id),
  "deviceId" UUID NOT NULL REFERENCES devices(id),
  "startTime" TIMESTAMPTZ NOT NULL,
  "endTime" TIMESTAMPTZ,
  "durationMinutes" INTEGER,
  "timeCreditsConsumed" INTEGER,
  "gameId" UUID
);

-- Legacy inventory tables removed in 20260528000001; stubs satisfy earlier ALTER migrations.
CREATE TABLE IF NOT EXISTS games (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  name VARCHAR(255)
);

CREATE TABLE IF NOT EXISTS device_games (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  "deviceId" UUID REFERENCES devices(id),
  "gameId" UUID REFERENCES games(id)
);

CREATE TABLE IF NOT EXISTS files (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ,
  category files_category_enum NOT NULL DEFAULT 'other',
  status files_status_enum NOT NULL DEFAULT 'active',
  visibility files_visibility_enum NOT NULL DEFAULT 'private',
  url TEXT
);
