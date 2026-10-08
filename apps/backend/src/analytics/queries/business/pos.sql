WITH visitors AS (SELECT DISTINCT s.player_id AS player FROM report_sessions s
 WHERE TRUE AND s.start_time>=$1 AND s.start_time<$2)
 SELECT coalesce(sum(amount),0),count(*),(SELECT count(*) FROM visitors),
 COUNT(DISTINCT player_id) FILTER (WHERE player_id IN (SELECT player FROM visitors)) FROM report_transactions
 WHERE TRUE AND payment_status IN ('completed','credit') AND transaction_type='product_purchase'
 AND occurred_at>=$1 AND occurred_at<$2
