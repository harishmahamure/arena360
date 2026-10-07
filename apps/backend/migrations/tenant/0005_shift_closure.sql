-- Preserve public completed/force_closed states while the core shift state remains active/closed.
ALTER TABLE shifts ADD COLUMN close_kind TEXT CHECK (close_kind IS NULL OR close_kind IN ('normal','force'));
-- Financial source links allow one withdrawal and, for a rejected deposit, one reversal.
CREATE UNIQUE INDEX cash_register_entries_financial_source
 ON cash_register_entries(reference_type,reference_id,entry_type)
 WHERE reference_type IN ('expense','cash_deposit') AND reference_id IS NOT NULL;
