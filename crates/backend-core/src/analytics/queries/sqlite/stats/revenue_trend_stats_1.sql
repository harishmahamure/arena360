SELECT
    report_local_date(t.created_at) AS date,
    report_money(COALESCE(SUM(
        CASE
            WHEN t.payment_method = 'cash' THEN t.amount
            WHEN t.payment_method = 'split_payment' THEN COALESCE(t.cash_amount, 0)
            ELSE 0
        END
    ), 0)) AS cash_revenue,
    report_money(COALESCE(SUM(
        CASE
            WHEN t.payment_method = 'online' THEN t.amount
            WHEN t.payment_method = 'split_payment' THEN COALESCE(t.online_amount, 0)
            ELSE 0
        END
    ), 0)) AS online_revenue,
    report_money(COALESCE(SUM(t.amount), 0)) AS total_revenue,
    COUNT(*) AS transaction_count
FROM report_transactions t
WHERE t.created_at BETWEEN $1 AND $2
  AND TRUE
  AND t.payment_status IN ('completed', 'credit')
GROUP BY report_local_date(t.created_at)
ORDER BY date ASC
