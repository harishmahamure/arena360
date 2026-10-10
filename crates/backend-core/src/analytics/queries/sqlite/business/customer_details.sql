SELECT
    CAST(v.player AS TEXT),
    coalesce(u.username,'Deleted player'),
    v.visits,
    report_local_date(v.first_visit),
    report_local_date(v.last_visit),
    report_money(coalesce(t.spend,0)),
    date_diff('day',v.last_visit,$2)
FROM visits v LEFT JOIN report_users u ON u.id=v.player
 LEFT JOIN (SELECT player_id,sum(amount) AS spend FROM report_transactions
 WHERE TRUE AND payment_status IN ('completed','credit') AND occurred_at>=$1 AND occurred_at<$2 GROUP BY player_id) t ON t.player_id=v.player
 ORDER BY v.last_visit DESC LIMIT 500
