SELECT
    COUNT(*) FILTER (WHERE status = 'open'),
    COUNT(*) FILTER (WHERE status = 'closed'),
    COUNT(*) FILTER (WHERE status = 'reconciled'),
    COUNT(*) FILTER (WHERE status = 'closed'),
    report_money(COALESCE((
        SELECT SUM(d.amount)
        FROM report_cash_deposits d
        WHERE d.status = 'approved'
          AND d.created_at BETWEEN $1 AND $2
    ), 0))
FROM report_cash_registers
WHERE created_at BETWEEN $1 AND $2
