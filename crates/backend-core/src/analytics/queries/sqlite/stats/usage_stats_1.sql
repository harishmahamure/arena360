SELECT
    COUNT(*),
    COUNT(*) FILTER (WHERE end_time IS NULL),
    COUNT(*) FILTER (WHERE end_time IS NOT NULL),
    COALESCE(SUM(duration_minutes), 0),
    avg(duration_minutes)
FROM report_sessions
WHERE start_time BETWEEN $1 AND $2 AND TRUE
