SELECT
    d.id AS device_id,
    d.name AS device_name,
    COUNT(s.id) AS total_sessions,
    COALESCE(SUM(s.duration_minutes), 0) / 60.0 AS total_hours
FROM report_devices d
INNER JOIN report_sessions s ON s.device_id = d.id
WHERE TRUE
  AND TRUE
  AND s.start_time BETWEEN $1 AND $2
GROUP BY d.id, d.name
ORDER BY total_hours DESC, total_sessions DESC
LIMIT 10
