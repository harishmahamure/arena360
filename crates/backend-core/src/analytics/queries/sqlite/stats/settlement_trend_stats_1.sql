SELECT
    report_local_date(cs.settled_at) AS date,
    report_money(COALESCE(SUM(
        CASE
            WHEN cs.payment_method = 'cash' THEN cs.amount
            WHEN cs.payment_method = 'split_payment' THEN COALESCE(cs.cash_amount, 0)
            ELSE 0
        END
    ), 0)) AS cash_revenue,
    report_money(COALESCE(SUM(
        CASE
            WHEN cs.payment_method = 'online' THEN cs.amount
            WHEN cs.payment_method = 'split_payment' THEN COALESCE(cs.online_amount, 0)
            ELSE 0
        END
    ), 0)) AS online_revenue,
    report_money(COALESCE(SUM(cs.amount), 0)) AS total_revenue
FROM report_credit_settlements cs
WHERE cs.settled_at BETWEEN $1 AND $2
  AND TRUE
GROUP BY report_local_date(cs.settled_at)
ORDER BY date ASC
