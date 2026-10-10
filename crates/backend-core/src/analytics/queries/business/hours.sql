SELECT CAST(h.local_date AS VARCHAR),h.weekday,h.local_hour,
 SUM(greatest(0,date_diff('second',greatest(s.start_time,h.hour_start,$1),
   least(s.end_time,$2,date_trunc('second',greatest(s.start_time,h.hour_start)) + h.occupied_seconds*INTERVAL 1 SECOND))))/3600.0,
 COUNT(*) FILTER (WHERE h.is_start_hour AND s.start_time >= $1 AND s.start_time < $2)
 FROM report_session_hours h JOIN report_sessions s ON s.id=h.session_id
 WHERE h.occupied_seconds>0 AND s.start_time<$2 AND s.end_time>$1
   AND h.hour_start<$2
   AND date_trunc('second',greatest(s.start_time,h.hour_start))+h.occupied_seconds*INTERVAL 1 SECOND>$1
 GROUP BY 1,2,3 ORDER BY 1,3
