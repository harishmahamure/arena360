//! Private transport to the process that owns SQLite/DuckDB. No SQL crosses this boundary.
use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{HeaderName, HeaderValue, Request, Response, StatusCode},
    response::IntoResponse,
    Router,
};
use tenant_protocol::storage::{
    tenant_storage_service_client::TenantStorageServiceClient,
    tenant_storage_service_server::{TenantStorageService, TenantStorageServiceServer},
    ApiRequest, ApiResponse, ProvisionRequest,
};
use tokio::sync::RwLock;
use tonic::{
    transport::{Channel, Endpoint},
    Request as RpcRequest, Response as RpcResponse, Status,
};
use tower::ServiceExt;
use uuid::Uuid;

use crate::{
    error::AppError,
    tenancy::{ProvisionTenant, TenantProvisioner},
};

const MAX_BODY: usize = 2 * 1024 * 1024;
const MAX_MESSAGE: usize = 4 * 1024 * 1024;
const TOKEN_HEADER: &str = "x-arena-service-token";

#[derive(Clone)]
pub struct StorageClient {
    token: tonic::metadata::MetadataValue<tonic::metadata::Ascii>,
    channels: Arc<RwLock<HashMap<String, Channel>>>,
}

#[derive(Clone)]
pub struct RemoteStorage {
    pub client: StorageClient,
    pub address: String,
    pub cell_id: Uuid,
}

pub fn validate_token(token: &str) -> Result<(), String> {
    if token.len() < 32 || token.trim() != token || !token.is_ascii() {
        return Err("STORAGE_SERVICE_TOKEN must be at least 32 ASCII characters without surrounding whitespace".into());
    }
    token
        .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
        .map_err(|_| "STORAGE_SERVICE_TOKEN is not valid gRPC metadata".to_string())?;
    Ok(())
}

impl StorageClient {
    pub fn new(token: &str) -> Result<Self, String> {
        validate_token(token)?;
        Ok(Self {
            token: token.parse().map_err(|_| "Invalid service token")?,
            channels: Default::default(),
        })
    }

    async fn client(&self, address: &str) -> Result<TenantStorageServiceClient<Channel>, AppError> {
        let mut channels = self.channels.write().await;
        let channel = if let Some(channel) = channels.get(address) {
            channel.clone()
        } else {
            let mut endpoint = Endpoint::from_shared(address.to_string())
                .map_err(|_| unavailable())?
                .connect_timeout(Duration::from_secs(2))
                .timeout(Duration::from_secs(30));
            if address.starts_with("https://") {
                endpoint = endpoint
                    .tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
                    .map_err(|_| unavailable())?;
            }
            let channel = endpoint.connect_lazy();
            channels.insert(address.to_string(), channel.clone());
            channel
        };
        Ok(TenantStorageServiceClient::new(channel)
            .max_decoding_message_size(MAX_MESSAGE)
            .max_encoding_message_size(MAX_MESSAGE))
    }

    fn request<T>(&self, message: T) -> RpcRequest<T> {
        let mut request = RpcRequest::new(message);
        request
            .metadata_mut()
            .insert(TOKEN_HEADER, self.token.clone());
        request.set_timeout(Duration::from_secs(30));
        request
    }

    pub async fn invoke(
        &self,
        address: &str,
        request: Request<Body>,
    ) -> Result<Response<Body>, AppError> {
        let (parts, body) = request.into_parts();
        let body = to_bytes(body, MAX_BODY)
            .await
            .map_err(|_| AppError::BadRequest("Request body exceeds storage RPC limit".into()))?;
        let headers = parts
            .headers
            .iter()
            .filter(|(name, _)| forwarded_header(name.as_str()))
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.to_string(), value.to_string()))
            })
            .collect();
        let input = ApiRequest {
            method: parts.method.to_string(),
            path: parts.uri.path().to_string(),
            query: parts.uri.query().unwrap_or_default().to_string(),
            body: body.to_vec(),
            headers,
        };
        let output = self
            .client(address)
            .await?
            .invoke(self.request(input))
            .await
            .map_err(|error| {
                tracing::warn!(code=?error.code(), "Storage RPC failed");
                unavailable()
            })?
            .into_inner();
        decode_response(output)
    }

    pub async fn provision(
        &self,
        address: &str,
        input: &ProvisionTenant,
    ) -> Result<serde_json::Value, AppError> {
        let tenant_json =
            serde_json::to_vec(input).map_err(|error| AppError::Internal(error.to_string()))?;
        let output = self
            .client(address)
            .await?
            .provision(self.request(ProvisionRequest { tenant_json }))
            .await
            .map_err(|error| {
                tracing::warn!(code=?error.code(), "Storage provisioning RPC failed");
                unavailable()
            })?
            .into_inner();
        let status =
            StatusCode::from_u16(output.status_code.try_into().map_err(|_| unavailable())?)
                .map_err(|_| unavailable())?;
        let body: serde_json::Value =
            serde_json::from_slice(&output.body).map_err(|_| unavailable())?;
        if !status.is_success() {
            return Err(AppError::Api {
                code: body
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("PROVISIONING_FAILED")
                    .into(),
                status,
                details: body.get("details").cloned(),
            });
        }
        Ok(body)
    }
}

fn unavailable() -> AppError {
    AppError::Api {
        code: "TENANT_STORAGE_UNAVAILABLE".into(),
        status: StatusCode::SERVICE_UNAVAILABLE,
        details: None,
    }
}

fn forwarded_header(name: &str) -> bool {
    matches!(
        name,
        "authorization"
            | "x-player-token"
            | "x-request-id"
            | "x-location-id"
            | "content-type"
            | "accept"
    )
}

fn decode_response(output: ApiResponse) -> Result<Response<Body>, AppError> {
    let status = StatusCode::from_u16(output.status_code.try_into().map_err(|_| unavailable())?)
        .map_err(|_| unavailable())?;
    let mut response = Response::builder().status(status);
    for (name, value) in output.headers {
        if let (Ok(name), Ok(value)) = (HeaderName::try_from(name), HeaderValue::try_from(value)) {
            response = response.header(name, value);
        }
    }
    response
        .body(Body::from(output.body))
        .map_err(|_| unavailable())
}

#[derive(Clone)]
pub struct StorageRpc {
    api: Router,
    provisioner: Arc<TenantProvisioner>,
}

impl StorageRpc {
    pub fn new(api: Router, provisioner: Arc<TenantProvisioner>) -> Self {
        Self { api, provisioner }
    }
}

#[tonic::async_trait]
impl TenantStorageService for StorageRpc {
    async fn invoke(
        &self,
        request: RpcRequest<ApiRequest>,
    ) -> Result<RpcResponse<ApiResponse>, Status> {
        let input = request.into_inner();
        let method = input
            .method
            .parse::<axum::http::Method>()
            .map_err(|_| Status::invalid_argument("Invalid method"))?;
        if !matches!(
            method,
            axum::http::Method::GET
                | axum::http::Method::POST
                | axum::http::Method::PUT
                | axum::http::Method::PATCH
                | axum::http::Method::DELETE
        ) || !input.path.starts_with('/')
            || input.path.starts_with("//")
            || input.path.contains(['?', '#'])
            || input.path.starts_with("/platform")
            || input.path.starts_with("/arena360.")
            || input.path == "/realtime"
        {
            return Err(Status::invalid_argument("Invalid tenant API request"));
        }
        if input.body.len() > MAX_BODY {
            return Err(Status::resource_exhausted("Request body too large"));
        }
        let uri = if input.query.is_empty() {
            input.path
        } else {
            format!("{}?{}", input.path, input.query)
        };
        let mut builder = Request::builder().method(method).uri(uri);
        for (name, value) in input.headers {
            if forwarded_header(&name.to_ascii_lowercase()) {
                builder = builder.header(name, value);
            }
        }
        if let Some(headers) = builder.headers_mut() {
            headers
                .entry(axum::http::header::CONTENT_TYPE)
                .or_insert(HeaderValue::from_static("application/json"));
        }
        let request = builder
            .body(Body::from(input.body))
            .map_err(|_| Status::invalid_argument("Invalid request"))?;
        let response = self
            .api
            .clone()
            .oneshot(request)
            .await
            .unwrap_or_else(|never| match never {});
        encode_response(response).await.map(RpcResponse::new)
    }

    async fn provision(
        &self,
        request: RpcRequest<ProvisionRequest>,
    ) -> Result<RpcResponse<ApiResponse>, Status> {
        let input: ProvisionTenant = serde_json::from_slice(&request.into_inner().tenant_json)
            .map_err(|_| Status::invalid_argument("Invalid provisioning payload"))?;
        let response = match self.provisioner.provision(input).await {
            Ok(result) => axum::Json(result.tenant).into_response(),
            Err(error) => error.into_response(),
        };
        encode_response(response).await.map(RpcResponse::new)
    }
}

async fn encode_response(response: Response<Body>) -> Result<ApiResponse, Status> {
    let (parts, body) = response.into_parts();
    let headers = parts
        .headers
        .iter()
        .filter(|(name, _)| {
            matches!(
                name.as_str(),
                "content-type" | "content-disposition" | "retry-after" | "x-request-id"
            )
        })
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_string()))
        })
        .collect();
    let body = to_bytes(body, MAX_MESSAGE - 4096)
        .await
        .map_err(|_| Status::resource_exhausted("Storage API response exceeds RPC limit"))?;
    Ok(ApiResponse {
        status_code: u32::from(parts.status.as_u16()),
        body: body.to_vec(),
        headers,
    })
}

pub fn router(service: StorageRpc, token: &str) -> Result<Router, String> {
    validate_token(token)?;
    let expected = token.to_string();
    let service =
        TenantStorageServiceServer::with_interceptor(service, move |request: RpcRequest<()>| {
            if request
                .metadata()
                .get(TOKEN_HEADER)
                .and_then(|v| v.to_str().ok())
                != Some(expected.as_str())
            {
                return Err(Status::unauthenticated("Service authentication required"));
            }
            Ok(request)
        });
    // The public gRPC router owns the shared unknown-service fallback. Keeping
    // Tonic's second fallback makes Axum panic when the worker merges routers.
    Ok(tonic::service::Routes::new(service)
        .into_axum_router()
        .reset_fallback())
}
