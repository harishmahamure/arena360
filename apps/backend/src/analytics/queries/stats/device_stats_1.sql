SELECT COUNT(*), COUNT(*) FILTER (WHERE status IN ('operational', 'available', 'in_use'))
FROM report_devices WHERE TRUE
