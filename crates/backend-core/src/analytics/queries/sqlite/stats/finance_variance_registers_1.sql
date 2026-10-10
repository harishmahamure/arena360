SELECT
    id,
    shift_id as shift_id,
    status,
    report_money(variance) as variance,
    report_money(closing_balance) as closing_balance,
    report_money(expected_closing) as expected_closing,
    updated_at as updated_at
FROM report_cash_registers
WHERE variance IS NOT NULL
  AND status IN ('closed', 'reconciled')
  AND updated_at BETWEEN $1 AND $2
ORDER BY ABS(variance) DESC, updated_at DESC
LIMIT 50
