SELECT
    u.id AS player_id,
    COALESCE(
        NULLIF(TRIM(CONCAT(COALESCE(u.first_name, ''), ' ', COALESCE(u.last_name, ''))), ''),
        u.username
    ) AS player_name,
    COALESCE(SUM(t.amount), 0) AS total_spent,
    COALESCE(session_counts.total_sessions, 0) AS total_sessions
FROM report_transactions t
INNER JOIN report_users u ON u.id = t.player_id
LEFT JOIN (
    SELECT
        s.player_id AS player_id,
        COUNT(*) AS total_sessions
    FROM report_sessions s
    WHERE TRUE
      AND TRUE
      AND s.start_time BETWEEN $1 AND $2
    GROUP BY s.player_id
) session_counts ON session_counts.player_id = u.id
WHERE t.created_at BETWEEN $1 AND $2
  AND TRUE
  AND TRUE
  AND u.role = 'player'
  AND t.payment_status::VARCHAR IN ('completed', 'credit')
GROUP BY u.id, u.username, u.first_name, u.last_name, session_counts.total_sessions
ORDER BY total_spent DESC, total_sessions DESC
LIMIT 5
