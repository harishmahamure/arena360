-- Migration: staff gaming allowance plan kind and ledger reasons
-- ADR: DRAFT-0044-staff-gaming-allowances

BEGIN;

ALTER TYPE plan_kind ADD VALUE IF NOT EXISTS 'staff_allowance';
ALTER TYPE ledger_reason ADD VALUE IF NOT EXISTS 'staff_allowance_grant';
ALTER TYPE ledger_reason ADD VALUE IF NOT EXISTS 'staff_allowance_renewal';

COMMIT;
