SELECT start_time,end_time FROM report_sessions WHERE start_time<$2 AND (end_time IS NULL OR end_time>$1) ORDER BY start_time,id
