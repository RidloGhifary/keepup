-- Composite indexes for rapid telemetry filtering, percentiles, and SLA computations
CREATE INDEX IF NOT EXISTS idx_check_logs_monitor_created ON check_logs(monitor_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_check_logs_created_at_desc ON check_logs(created_at DESC);
