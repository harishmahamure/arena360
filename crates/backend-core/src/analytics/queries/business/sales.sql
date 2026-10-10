SELECT CAST(CAST(timezone('__TIMEZONE__',timezone('UTC',occurred_at)) AS DATE) AS VARCHAR),
 sum(amount), COALESCE(SUM(amount) FILTER (WHERE transaction_type='plan_purchase'),0),
 COALESCE(SUM(amount) FILTER (WHERE transaction_type='product_purchase'),0), count(*), COUNT(DISTINCT player_id)
 FROM report_transactions WHERE TRUE AND payment_status IN ('completed','credit')
 AND occurred_at>=$1 AND occurred_at<$2 GROUP BY 1 ORDER BY 1
