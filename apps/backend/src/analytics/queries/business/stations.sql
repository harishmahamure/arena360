SELECT CAST(d.id AS VARCHAR), d.name, coalesce(nullif(d.area_label,''),'Unassigned'), d.status,
 COUNT(*) FILTER (WHERE s.start_time>=$1 AND s.start_time<$2),
 coalesce(sum(greatest(0,date_diff('second',greatest(s.start_time,$1),least(coalesce(s.end_time,$2),$2)))),0)/3600.0
 FROM report_device_names d LEFT JOIN report_sessions s ON s.device_id=d.id AND TRUE
 AND s.start_time<$2 AND coalesce(s.end_time,$2)>$1
 WHERE TRUE GROUP BY d.id,d.name,d.area_label,d.status ORDER BY 6 DESC,d.name
