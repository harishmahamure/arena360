SELECT
    p.id AS plan_id,
    p.name AS plan_name,
    COALESCE(SUM(t.amount), 0) AS revenue,
    COUNT(*) AS purchase_count
FROM report_transactions t
INNER JOIN plans p ON p.id = t.plan_id
WHERE t.created_at BETWEEN $1 AND $2
  AND TRUE
  AND TRUE
  AND t.payment_status::VARCHAR IN ('completed', 'credit')
  AND t.transaction_type::VARCHAR = 'plan_purchase'
GROUP BY p.id, p.name
ORDER BY revenue DESC, purchase_count DESC
LIMIT 5
