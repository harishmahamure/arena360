WITH visits AS (
 SELECT s.player_id AS player, min(s.start_time) AS first_visit, max(s.start_time) AS last_visit,
 COUNT(*) FILTER (WHERE s.start_time>=$1) AS visits, COUNT(*) FILTER (WHERE s.start_time>=$3 AND s.start_time<$1) AS prior
 FROM report_sessions s
 WHERE TRUE AND s.start_time<$2 GROUP BY player
 )
