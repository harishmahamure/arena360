use axum::{
    extract::{Query, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use std::{convert::Infallible, sync::Arc};
use tokio_stream::StreamExt;
use utoipa::ToSchema;

use crate::app::AppState;
use crate::openapi::responses::ErrorEnvelope;

#[derive(serde::Deserialize, ToSchema)]
pub struct SseQuery {
    pub topics: Option<String>,
}

#[utoipa::path(
    get,
    path = "/sse",
    params(
        ("topics" = Option<String>, Query, description = "Comma-separated topic prefixes to filter events"),
    ),
    responses(
        (status = 200, description = "Server-sent events stream", content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "sse"
)]
pub async fn sse_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SseQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let topics: Vec<String> = query
        .topics
        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let rx = state.events.broadcaster().subscribe();

    let stream =
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(move |result| match result {
            Ok(event) => {
                if topics.is_empty() || topics.iter().any(|t| event.topic.starts_with(t)) {
                    Some(Ok(Event::default()
                        .event(&event.topic)
                        .json_data(&event.payload)
                        .unwrap()))
                } else {
                    None
                }
            }
            Err(_) => None,
        });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
