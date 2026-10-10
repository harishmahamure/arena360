WITH hours AS (SELECT user_id AS actor,
 sum(greatest(0,date_diff('second',greatest(clock_in,$1),least(coalesce(clock_out,$2),$2))))/3600.0 AS hours
 FROM report_shifts WHERE clock_in<$2 AND coalesce(clock_out,$2)>$1 GROUP BY actor),
 sales AS (SELECT created_by AS actor,sum(amount) AS revenue,count(*) AS report_transactions FROM report_transactions
 WHERE TRUE AND payment_status IN ('completed','credit') AND occurred_at>=$1 AND occurred_at<$2 GROUP BY actor),
 sessions AS (SELECT created_by AS actor,count(*) AS sessions FROM report_sessions WHERE TRUE AND start_time>=$1 AND start_time<$2 GROUP BY actor)
 SELECT CAST(u.id AS VARCHAR),u.username,coalesce(h.hours,0),coalesce(t.revenue,0),coalesce(t.report_transactions,0),coalesce(s.sessions,0)
 FROM report_staff u LEFT JOIN hours h ON h.actor=u.id LEFT JOIN sales t ON t.actor=u.id LEFT JOIN sessions s ON s.actor=u.id
 WHERE u.role IN ('admin','staff') AND TRUE ORDER BY 4 DESC
