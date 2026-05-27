DROP TABLE IF EXISTS expenses CASCADE;
DROP TABLE IF EXISTS vendors CASCADE;
DROP TABLE IF EXISTS expense_categories CASCADE;

CREATE TABLE expense_categories (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(100) NOT NULL,
  description TEXT,
  "parentId" UUID REFERENCES expense_categories(id),
  "isActive" BOOLEAN NOT NULL DEFAULT true,
  "budgetAmount" DECIMAL(19,4),
  "budgetPeriod" VARCHAR(20),
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE vendors (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name VARCHAR(200) NOT NULL,
  "contactPerson" VARCHAR(200),
  phone VARCHAR(20),
  email VARCHAR(200),
  address TEXT,
  "gstNumber" VARCHAR(20),
  "isActive" BOOLEAN NOT NULL DEFAULT true,
  notes TEXT,
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE expenses (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "categoryId" UUID NOT NULL REFERENCES expense_categories(id),
  "vendorId" UUID REFERENCES vendors(id),
  amount DECIMAL(19,4) NOT NULL,
  "paymentMethod" VARCHAR(20) NOT NULL,
  "paymentAccount" VARCHAR(100),
  description TEXT,
  "receiptUrl" TEXT,
  "expenseDate" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "isRecurring" BOOLEAN NOT NULL DEFAULT false,
  "recurrencePattern" VARCHAR(20),
  "nextRecurrenceDate" TIMESTAMPTZ,
  "approvalStatus" VARCHAR(20) NOT NULL DEFAULT 'pending',
  "approvedBy" UUID REFERENCES users(id),
  "approvedAt" TIMESTAMPTZ,
  "rejectionReason" TEXT,
  "shiftId" UUID REFERENCES shifts(id),
  "cashRegisterEntryId" UUID,
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "deletedAt" TIMESTAMPTZ
);

CREATE INDEX idx_expenses_category ON expenses("categoryId");
CREATE INDEX idx_expenses_vendor ON expenses("vendorId");
CREATE INDEX idx_expenses_status ON expenses("approvalStatus");
CREATE INDEX idx_expenses_date ON expenses("expenseDate" DESC);
CREATE INDEX idx_expenses_shift ON expenses("shiftId");
CREATE INDEX idx_vendors_name ON vendors(name);
CREATE INDEX idx_expense_categories_parent ON expense_categories("parentId");
