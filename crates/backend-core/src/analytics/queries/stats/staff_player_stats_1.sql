SELECT
    COUNT(*) FILTER (WHERE role = 'player' AND is_active = true),
    COUNT(*) FILTER (WHERE role = 'player' AND created_at BETWEEN $1 AND $2)
FROM report_users
WHERE TRUE
