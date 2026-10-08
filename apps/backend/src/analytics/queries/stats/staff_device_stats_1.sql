SELECT
    COUNT(*),
    COUNT(*) FILTER (WHERE status IN ('available', 'operational')),
    COUNT(*) FILTER (WHERE status = 'in_use')
FROM report_devices
WHERE TRUE
