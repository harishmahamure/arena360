SELECT
    COUNT(*),
    COUNT(*) FILTER (WHERE is_active = true),
    COUNT(*) FILTER (WHERE role = 'player'),
    COUNT(*) FILTER (WHERE role = 'player' AND is_active = true),
    COUNT(*) FILTER (WHERE created_at BETWEEN $1 AND $2)
FROM report_users
WHERE TRUE
