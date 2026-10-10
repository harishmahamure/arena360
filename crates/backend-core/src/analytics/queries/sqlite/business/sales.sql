SELECT
    report_local_date(occurred_at),
    report_money(sum(amount)),
    report_money(COALESCE(SUM(amount) FILTER (WHERE transaction_type='plan_purchase'),0)),
    report_money(COALESCE(SUM(amount) FILTER (WHERE transaction_type='product_purchase'),0)),
    count(*),
    COUNT(DISTINCT player_id)
FROM report_transactions WHERE TRUE AND payment_status IN ('completed','credit')
 AND occurred_at>=$1 AND occurred_at<$2 GROUP BY 1 ORDER BY 1
