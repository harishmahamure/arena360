SELECT
    report_money(COALESCE(SUM(variance), 0)),
    report_money(COALESCE(avg(variance), 0)),
    COUNT(*) FILTER (WHERE variance > 0),
    COUNT(*) FILTER (WHERE variance < 0),
    COUNT(*) FILTER (WHERE variance = 0),
    COUNT(*)
FROM report_cash_registers
WHERE variance IS NOT NULL
  AND status IN ('closed', 'reconciled')
  AND updated_at BETWEEN $1 AND $2
