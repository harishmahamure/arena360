SELECT
    CAST(p.id AS TEXT),
    p.name,
    sum(t.purchases),
    count(*),
    COUNT(*) FILTER (WHERE t.purchases>1),
    report_money(sum(t.revenue)),
    sum(t.purchases*coalesce(p.time_credit_minutes,0))/60.0
FROM (SELECT plan_id,player_id,count(*) AS purchases,sum(amount) AS revenue FROM report_transactions
 WHERE TRUE AND payment_status IN ('completed','credit') AND transaction_type='plan_purchase'
 AND occurred_at>=$1 AND occurred_at<$2 GROUP BY plan_id,player_id) t
 INNER JOIN plans p ON p.id=t.plan_id GROUP BY p.id,p.name ORDER BY 6 DESC
