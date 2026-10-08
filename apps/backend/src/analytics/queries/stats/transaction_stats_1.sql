SELECT
    COUNT(*),
    COUNT(*) FILTER (WHERE payment_status::VARCHAR IN ('completed', 'credit')),
    COUNT(*) FILTER (WHERE payment_status = 'pending'),
    COUNT(*) FILTER (WHERE payment_status = 'failed'),
    avg(amount)
FROM report_transactions
WHERE created_at BETWEEN $1 AND $2 AND TRUE
