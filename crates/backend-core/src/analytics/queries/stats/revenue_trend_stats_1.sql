SELECT
    CAST(timezone('__TIMEZONE__',timezone('UTC',t.created_at)) AS DATE) AS date,
    COALESCE(SUM(
        CASE
            WHEN t.payment_method::VARCHAR = 'cash' THEN t.amount
            WHEN t.payment_method::VARCHAR = 'split_payment' THEN COALESCE(t.cash_amount, 0)
            ELSE 0
        END
    ), 0) AS cash_revenue,
    COALESCE(SUM(
        CASE
            WHEN t.payment_method::VARCHAR = 'online' THEN t.amount
            WHEN t.payment_method::VARCHAR = 'split_payment' THEN COALESCE(t.online_amount, 0)
            ELSE 0
        END
    ), 0) AS online_revenue,
    COALESCE(SUM(t.amount), 0) AS total_revenue,
    COUNT(*) AS transaction_count
FROM report_transactions t
WHERE t.created_at BETWEEN $1 AND $2
  AND TRUE
  AND t.payment_status::VARCHAR IN ('completed', 'credit')
GROUP BY CAST(timezone('__TIMEZONE__',timezone('UTC',t.created_at)) AS DATE)
ORDER BY date ASC
