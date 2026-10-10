SELECT
    COUNT(*) FILTER (WHERE status = 'pending'),
    report_money(COALESCE(COALESCE(SUM(amount) FILTER (WHERE status = 'pending'),0), 0)),
    COUNT(*) FILTER (WHERE status = 'approved'),
    report_money(COALESCE(COALESCE(SUM(amount) FILTER (WHERE status = 'approved'),0), 0)),
    COUNT(*) FILTER (WHERE status = 'rejected'),
    report_money(COALESCE(COALESCE(SUM(amount) FILTER (WHERE status = 'rejected'),0), 0)),
    report_money(COALESCE(COALESCE(SUM(amount) FILTER (WHERE status = 'approved' AND deposit_type = 'bank'),0), 0)),
    report_money(COALESCE(COALESCE(SUM(amount) FILTER (WHERE status = 'approved' AND deposit_type = 'home'),0), 0))
FROM report_cash_deposits
WHERE created_at BETWEEN $1 AND $2
