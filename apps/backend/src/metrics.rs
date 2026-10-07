use axum::{http::header, response::IntoResponse};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct Metrics {
    rate_limit_allowed: AtomicU64,
    rate_limit_rejected: AtomicU64,
    rate_limit_fail_open: AtomicU64,
    redis_failures: AtomicU64,
    active_websockets: AtomicU64,
    slow_consumers_dropped: AtomicU64,
    rpc_requests: AtomicU64,
    rpc_errors: AtomicU64,
    rpc_latency_micros: AtomicU64,
    protobuf_request_bytes: AtomicU64,
    protobuf_response_bytes: AtomicU64,
    gzip_responses: AtomicU64,
    outbox_lag_millis: AtomicU64,
    publish_pending: AtomicU64,
    publish_bytes: AtomicU64,
    publish_oldest_millis: AtomicU64,
    publish_failures: AtomicU64,
    published: AtomicU64,
    analytics_events: AtomicU64,
    analytics_gaps: AtomicU64,
    analytics_failures: AtomicU64,
    analytics_rebuild_started: AtomicU64,
    analytics_rebuild_completed: AtomicU64,
    analytics_rebuild_millis: AtomicU64,
    analytics_retention_runs: AtomicU64,
    analytics_retention_rows: AtomicU64,
}

impl Metrics {
    pub fn rate_allowed(&self) {
        self.rate_limit_allowed.fetch_add(1, Ordering::Relaxed);
    }
    pub fn rate_rejected(&self) {
        self.rate_limit_rejected.fetch_add(1, Ordering::Relaxed);
    }
    pub fn rate_fail_open(&self) {
        self.rate_limit_fail_open.fetch_add(1, Ordering::Relaxed);
        self.redis_failures.fetch_add(1, Ordering::Relaxed);
    }
    pub fn websocket_opened(&self) {
        self.active_websockets.fetch_add(1, Ordering::Relaxed);
    }
    pub fn websocket_closed(&self) {
        self.active_websockets.fetch_sub(1, Ordering::Relaxed);
    }
    pub fn slow_consumer_dropped(&self) {
        self.slow_consumers_dropped.fetch_add(1, Ordering::Relaxed);
    }
    pub fn rpc_completed(
        &self,
        error: bool,
        latency_micros: u64,
        request_bytes: usize,
        response_bytes: usize,
        gzip: bool,
    ) {
        self.rpc_requests.fetch_add(1, Ordering::Relaxed);
        self.rpc_errors
            .fetch_add(u64::from(error), Ordering::Relaxed);
        self.rpc_latency_micros
            .fetch_add(latency_micros, Ordering::Relaxed);
        self.protobuf_request_bytes
            .fetch_add(request_bytes as u64, Ordering::Relaxed);
        self.protobuf_response_bytes
            .fetch_add(response_bytes as u64, Ordering::Relaxed);
        self.gzip_responses
            .fetch_add(u64::from(gzip), Ordering::Relaxed);
    }
    pub fn set_outbox_lag(&self, millis: u64) {
        self.outbox_lag_millis.store(millis, Ordering::Relaxed);
    }
    pub fn outbox_published(&self) {
        self.published.fetch_add(1, Ordering::Relaxed);
    }
    pub fn outbox_publish_failed(&self) {
        self.publish_failures.fetch_add(1, Ordering::Relaxed);
    }
    pub fn set_publish_backlog(&self, pending: u64, bytes: u64, oldest_millis: u64) {
        self.publish_pending.store(pending, Ordering::Relaxed);
        self.publish_bytes.store(bytes, Ordering::Relaxed);
        self.publish_oldest_millis
            .store(oldest_millis, Ordering::Relaxed);
    }
    pub fn analytics_ingested(&self, events: u64) { self.analytics_events.fetch_add(events, Ordering::Relaxed); }
    pub fn analytics_gap(&self) { self.analytics_gaps.fetch_add(1, Ordering::Relaxed); }
    pub fn analytics_retained(&self, rows:u64) { self.analytics_retention_runs.fetch_add(1,Ordering::Relaxed);self.analytics_retention_rows.fetch_add(rows,Ordering::Relaxed); }
    pub fn analytics_rebuild_started(&self) { self.analytics_rebuild_started.fetch_add(1, Ordering::Relaxed); }
    pub fn analytics_rebuild_finished(&self, success: bool, millis: u64) {
        if success { self.analytics_rebuild_completed.fetch_add(1, Ordering::Relaxed); }
        self.analytics_rebuild_millis.fetch_add(millis, Ordering::Relaxed);
    }
    pub fn analytics_failed(&self) { self.analytics_failures.fetch_add(1, Ordering::Relaxed); }
    pub fn render(&self) -> String {
        format!(
            concat!(
                "# TYPE arena360_rate_limit_decisions_total counter\n",
                "arena360_rate_limit_decisions_total{{decision=\"allowed\"}} {}\n",
                "arena360_rate_limit_decisions_total{{decision=\"rejected\"}} {}\n",
                "arena360_rate_limit_decisions_total{{decision=\"fail_open\"}} {}\n",
                "# TYPE arena360_redis_failures_total counter\n",
                "arena360_redis_failures_total {}\n",
                "# TYPE arena360_active_websockets gauge\n",
                "arena360_active_websockets {}\n",
                "# TYPE arena360_slow_consumers_dropped_total counter\n",
                "arena360_slow_consumers_dropped_total {}\n",
                "# TYPE arena360_rpc_requests_total counter\n",
                "arena360_rpc_requests_total {}\n",
                "arena360_rpc_errors_total {}\n",
                "arena360_rpc_latency_microseconds_total {}\n",
                "arena360_protobuf_request_bytes_total {}\n",
                "arena360_protobuf_response_bytes_total {}\n",
                "arena360_gzip_responses_total {}\n",
                "# TYPE arena360_outbox_lag_milliseconds gauge\n",
                "arena360_outbox_lag_milliseconds {}\n",
                "# TYPE arena360_outbox_pending_events gauge\n",
                "arena360_outbox_pending_events {}\n",
                "# TYPE arena360_outbox_payload_bytes gauge\n",
                "arena360_outbox_payload_bytes {}\n",
                "# TYPE arena360_outbox_oldest_age_milliseconds gauge\n",
                "arena360_outbox_oldest_age_milliseconds {}\n",
                "# TYPE arena360_outbox_publish_failures_total counter\n",
                "arena360_outbox_publish_failures_total {}\n",
                "# TYPE arena360_outbox_published_total counter\n",
                "arena360_outbox_published_total {}\n",
                "# TYPE arena360_analytics_ingested_events_total counter\n",
                "arena360_analytics_ingested_events_total {}\n",
                "# TYPE arena360_analytics_sequence_gaps_total counter\n",
                "arena360_analytics_sequence_gaps_total {}\n",
                "# TYPE arena360_analytics_failures_total counter\n",
                "arena360_analytics_failures_total {}\n",
                "# TYPE arena360_analytics_rebuild_started_total counter\n",
                "arena360_analytics_rebuild_started_total {}\n",
                "# TYPE arena360_analytics_rebuild_completed_total counter\n",
                "arena360_analytics_rebuild_completed_total {}\n",
                "# TYPE arena360_analytics_rebuild_duration_milliseconds_total counter\n",
                "arena360_analytics_rebuild_duration_milliseconds_total {}\n",
                "# TYPE arena360_analytics_retention_runs_total counter\n",
                "arena360_analytics_retention_runs_total {}\n",
                "# TYPE arena360_analytics_retention_deleted_rows_total counter\n",
                "arena360_analytics_retention_deleted_rows_total {}\n"
            ),
            self.rate_limit_allowed.load(Ordering::Relaxed),
            self.rate_limit_rejected.load(Ordering::Relaxed),
            self.rate_limit_fail_open.load(Ordering::Relaxed),
            self.redis_failures.load(Ordering::Relaxed),
            self.active_websockets.load(Ordering::Relaxed),
            self.slow_consumers_dropped.load(Ordering::Relaxed),
            self.rpc_requests.load(Ordering::Relaxed),
            self.rpc_errors.load(Ordering::Relaxed),
            self.rpc_latency_micros.load(Ordering::Relaxed),
            self.protobuf_request_bytes.load(Ordering::Relaxed),
            self.protobuf_response_bytes.load(Ordering::Relaxed),
            self.gzip_responses.load(Ordering::Relaxed),
            self.outbox_lag_millis.load(Ordering::Relaxed),
            self.publish_pending.load(Ordering::Relaxed),
            self.publish_bytes.load(Ordering::Relaxed),
            self.publish_oldest_millis.load(Ordering::Relaxed),
            self.publish_failures.load(Ordering::Relaxed),
            self.published.load(Ordering::Relaxed),
            self.analytics_events.load(Ordering::Relaxed),
            self.analytics_gaps.load(Ordering::Relaxed),
            self.analytics_failures.load(Ordering::Relaxed),
            self.analytics_rebuild_started.load(Ordering::Relaxed),
            self.analytics_rebuild_completed.load(Ordering::Relaxed),
            self.analytics_rebuild_millis.load(Ordering::Relaxed),
            self.analytics_retention_runs.load(Ordering::Relaxed),
            self.analytics_retention_rows.load(Ordering::Relaxed),
        )
    }
}

pub async fn prometheus(
    axum::extract::State(state): axum::extract::State<std::sync::Arc<crate::app::AppState>>,
) -> impl IntoResponse {
    let pool_size = state.control_db.as_ref().map_or(0, |pool| pool.size());
    let pool_idle = state.control_db.as_ref().map_or(0, |pool| pool.num_idle());
    let body = format!("{}# TYPE arena360_sql_pool_connections gauge\narena360_sql_pool_connections{{state=\"total\"}} {pool_size}\narena360_sql_pool_connections{{state=\"idle\"}} {pool_idle}\n", state.metrics.render());
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], body)
}
