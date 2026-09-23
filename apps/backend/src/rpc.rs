use std::sync::Arc;

use axum::{
    body::Body,
    http::{HeaderName, HeaderValue, Method, Request},
    Router,
};
use http_body_util::BodyExt;
use tonic::{
    codec::CompressionEncoding, Request as TonicRequest, Response as TonicResponse, Status,
};
use tower::ServiceExt;

use crate::proto::arena360::v1::{
    auth_service_server::{AuthService, AuthServiceServer},
    configuration_service_server::{ConfigurationService, ConfigurationServiceServer},
    device_service_server::{DeviceService, DeviceServiceServer},
    inventory_service_server::{InventoryService, InventoryServiceServer},
    notification_service_server::{NotificationService, NotificationServiceServer},
    plan_service_server::{PlanService, PlanServiceServer},
    session_service_server::{SessionService, SessionServiceServer},
    shift_cash_service_server::{ShiftCashService, ShiftCashServiceServer},
    statistics_service_server::{StatisticsService, StatisticsServiceServer},
    transaction_service_server::{TransactionService, TransactionServiceServer},
    upload_service_server::{UploadService, UploadServiceServer},
    user_service_server::{UserService, UserServiceServer},
    ProxyRequest, ProxyResponse,
};

const MAX_PROTOBUF_MESSAGE_SIZE: usize = 4 * 1024 * 1024;

/// Binary transport adapter for the existing application router. Keeping the
/// application router behind this boundary gives the cutover identical auth,
/// validation and error behaviour while the public REST routes are disabled.
#[derive(Clone)]
pub struct RpcGateway {
    business_router: Router,
    metrics: Arc<crate::metrics::Metrics>,
}

impl RpcGateway {
    pub fn new(business_router: Router, metrics: Arc<crate::metrics::Metrics>) -> Self {
        Self {
            business_router,
            metrics,
        }
    }

    async fn invoke(
        &self,
        request: TonicRequest<ProxyRequest>,
    ) -> Result<TonicResponse<ProxyResponse>, Status> {
        let started = std::time::Instant::now();
        let metadata = request.metadata().clone();
        let gzip = metadata
            .get("grpc-accept-encoding")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("gzip"));
        let input = request.into_inner();
        let method = Method::from_bytes(input.method.as_bytes())
            .map_err(|_| Status::invalid_argument("unsupported HTTP method"))?;
        if !matches!(
            method,
            Method::GET | Method::POST | Method::PUT | Method::PATCH | Method::DELETE
        ) {
            return Err(Status::invalid_argument("unsupported HTTP method"));
        }
        if !input.path.starts_with('/') || input.path.starts_with("//") {
            return Err(Status::invalid_argument(
                "path must be an absolute application path",
            ));
        }

        let uri = if input.query.is_empty() {
            input.path.clone()
        } else {
            format!("{}?{}", input.path, input.query)
        };
        let content_type = input
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .and_then(|(_, value)| HeaderValue::from_str(value).ok())
            .unwrap_or_else(|| HeaderValue::from_static("application/json"));
        let mut builder = Request::builder().method(method).uri(uri);
        let headers = builder
            .headers_mut()
            .ok_or_else(|| Status::internal("failed to construct request"))?;
        headers.insert(axum::http::header::CONTENT_TYPE, content_type);

        for name in ["authorization", "x-player-token", "x-request-id"] {
            if let Some(value) = metadata.get(name).and_then(|v| v.to_str().ok()) {
                if let Ok(value) = HeaderValue::from_str(value) {
                    headers.insert(HeaderName::from_static(name), value);
                }
            }
        }
        for (name, value) in input.headers {
            let lower = name.to_ascii_lowercase();
            if !matches!(
                lower.as_str(),
                "authorization" | "x-player-token" | "x-request-id" | "content-type" | "accept"
            ) {
                continue;
            }
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_bytes(lower.as_bytes()),
                HeaderValue::from_str(&value),
            ) {
                headers.insert(name, value);
            }
        }

        let request_bytes = input.body_json.len();
        let request = builder
            .body(Body::from(input.body_json))
            .map_err(|_| Status::invalid_argument("invalid request"))?;
        let response = self
            .business_router
            .clone()
            .oneshot(request)
            .await
            .unwrap_or_else(|never| match never {});
        let status_code = u32::from(response.status().as_u16());
        let response_headers = response.headers().clone();
        let body_json = response
            .into_body()
            .collect()
            .await
            .map_err(|error| Status::internal(format!("response body failed: {error}")))?
            .to_bytes();

        let mut headers = std::collections::HashMap::new();
        for name in [
            "content-type",
            "content-disposition",
            "content-length",
            "retry-after",
            "x-request-id",
        ] {
            if let Some(value) = response_headers.get(name).and_then(|v| v.to_str().ok()) {
                headers.insert(name.to_string(), value.to_string());
            }
        }

        self.metrics.rpc_completed(
            status_code >= 400,
            started.elapsed().as_micros() as u64,
            request_bytes,
            body_json.len(),
            gzip,
        );
        Ok(TonicResponse::new(ProxyResponse {
            status_code,
            body_json: body_json.to_vec(),
            headers,
        }))
    }
}

macro_rules! implement_gateway_service {
    ($trait_name:ident) => {
        #[tonic::async_trait]
        impl $trait_name for RpcGateway {
            async fn invoke(
                &self,
                request: TonicRequest<ProxyRequest>,
            ) -> Result<TonicResponse<ProxyResponse>, Status> {
                self.invoke(request).await
            }
        }
    };
}

implement_gateway_service!(AuthService);
implement_gateway_service!(UserService);
implement_gateway_service!(DeviceService);
implement_gateway_service!(PlanService);
implement_gateway_service!(SessionService);
implement_gateway_service!(TransactionService);
implement_gateway_service!(ShiftCashService);
implement_gateway_service!(InventoryService);
implement_gateway_service!(NotificationService);
implement_gateway_service!(ConfigurationService);
implement_gateway_service!(StatisticsService);
implement_gateway_service!(UploadService);

macro_rules! configured_service {
    ($server:ident, $gateway:expr) => {
        $server::new($gateway)
            .accept_compressed(CompressionEncoding::Gzip)
            .send_compressed(CompressionEncoding::Gzip)
            .max_decoding_message_size(MAX_PROTOBUF_MESSAGE_SIZE)
            .max_encoding_message_size(MAX_PROTOBUF_MESSAGE_SIZE)
    };
}

pub fn router(business_router: Router, metrics: Arc<crate::metrics::Metrics>) -> Router {
    let gateway = RpcGateway::new(business_router, metrics);
    let routes =
        tonic::service::Routes::new(configured_service!(AuthServiceServer, gateway.clone()))
            .add_service(configured_service!(UserServiceServer, gateway.clone()))
            .add_service(configured_service!(DeviceServiceServer, gateway.clone()))
            .add_service(configured_service!(PlanServiceServer, gateway.clone()))
            .add_service(configured_service!(SessionServiceServer, gateway.clone()))
            .add_service(configured_service!(
                TransactionServiceServer,
                gateway.clone()
            ))
            .add_service(configured_service!(ShiftCashServiceServer, gateway.clone()))
            .add_service(configured_service!(InventoryServiceServer, gateway.clone()))
            .add_service(configured_service!(
                NotificationServiceServer,
                gateway.clone()
            ))
            .add_service(configured_service!(
                ConfigurationServiceServer,
                gateway.clone()
            ))
            .add_service(configured_service!(
                StatisticsServiceServer,
                gateway.clone()
            ))
            .add_service(configured_service!(UploadServiceServer, gateway));

    routes
        .into_axum_router()
        .layer(tonic_web::GrpcWebLayer::new())
}
