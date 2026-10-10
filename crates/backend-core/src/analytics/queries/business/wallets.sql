SELECT COUNT(DISTINCT player_id) FILTER (WHERE status='active' AND expiry_date>$1 AND remaining_minutes>0),
 COUNT(*) FILTER (WHERE status='active' AND expiry_date>$1 AND remaining_minutes>0),
 COUNT(*) FILTER (WHERE status='active' AND expiry_date>$1 AND expiry_date<=$1+INTERVAL 7 DAY AND remaining_minutes>0),
 COUNT(*) FILTER (WHERE status='expired' OR expiry_date<=$1),
 coalesce(COALESCE(SUM(remaining_minutes) FILTER (WHERE status='active' AND expiry_date>$1 AND remaining_minutes>0),0),0)/60.0
 FROM report_wallets WHERE TRUE AND coalesce(kind,'time')!='staff_allowance'
