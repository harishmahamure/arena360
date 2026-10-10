//! Explicit entry points for the two deployment units.
use std::sync::Arc;

use crate::{
    app,
    config::{Roles, Settings},
    storage_rpc::{RemoteStorage, StorageClient, StorageRpc},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    Storage,
    Gateway,
}

impl Service {
    pub fn roles(self) -> Roles {
        match self {
            Self::Storage => Roles {
                control: false,
                cell: true,
                router: false,
            },
            Self::Gateway => Roles {
                control: true,
                cell: false,
                router: true,
            },
        }
    }
}

pub async fn run(service: Service) {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gaming_cafe_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    crate::config::load_dotenv();
    let mut settings = Settings::from_env();
    settings.roles = service.roles();
    settings
        .control_database_url
        .as_ref()
        .expect("CONTROL_DATABASE_URL is required by split services");
    let token = std::env::var("STORAGE_SERVICE_TOKEN")
        .expect("STORAGE_SERVICE_TOKEN is required by split services");
    let client = StorageClient::new(&token).expect("Invalid STORAGE_SERVICE_TOKEN");
    let remote = if service == Service::Gateway {
        // Even when a shared .env contains a cell ID, the gateway never claims local ownership.
        settings.cell_id = None;
        let address = std::env::var("STORAGE_SERVICE_URL")
            .expect("STORAGE_SERVICE_URL is required for tenant provisioning");
        validate_origin(&address).expect("Invalid STORAGE_SERVICE_URL");
        let cell_id = std::env::var("STORAGE_CELL_ID")
            .expect("STORAGE_CELL_ID is required for tenant provisioning")
            .parse()
            .expect("STORAGE_CELL_ID must be a UUID");
        Some(RemoteStorage {
            client: client.clone(),
            address,
            cell_id,
        })
    } else {
        settings
            .cell_id
            .expect("ARENA_CELL_ID is required by tenant-storage");
        None
    };
    // Private business invocation uses build_business_router directly. Public REST remains opt-in.
    let port = settings.port;
    let mut state = app::build_state_with_settings(Arc::new(settings)).await;
    if service == Service::Gateway {
        let state = Arc::get_mut(&mut state).expect("Unshared startup state");
        configure_gateway(state, remote.expect("Gateway storage configuration"));
    }
    let router = if service == Service::Storage {
        let private = crate::storage_rpc::router(
            StorageRpc::new(
                app::build_business_router(state.clone()),
                state
                    .tenant_provisioner
                    .clone()
                    .expect("Storage provisioning requires ownership leases"),
            ),
            &token,
        )
        .expect("Storage RPC configuration");
        app::build_router(state).merge(private)
    } else {
        app::build_router(state)
    };
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::UNSPECIFIED, port))
        .await
        .expect("Bind service port");
    tracing::info!(?service, port, "Starting tenant service");
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await
    .expect("Serve tenant service");
}

/// Attach private storage transport without creating local tenant storage.
pub fn configure_gateway(state: &mut app::AppState, remote: RemoteStorage) {
    assert!(
        state.tenant_dbs.is_none() && state.tenant_provisioner.is_none(),
        "Gateway must not own tenant files"
    );
    let routing = state
        .routing
        .as_mut()
        .expect("Gateway routing requires PostgreSQL");
    *routing = Arc::new(routing.as_ref().clone().with_storage(remote.client.clone()));
    state.remote_storage = Some(remote);
}

fn validate_origin(address: &str) -> Result<(), &'static str> {
    let url = reqwest::Url::parse(address).map_err(|_| "Expected HTTP(S) origin")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Expected HTTP(S) origin without credentials, path or query");
    }
    Ok(())
}

async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Install SIGTERM handler");
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gateway_is_stateless_and_storage_is_the_only_cell() {
        assert_eq!(
            Service::Gateway.roles(),
            Roles::parse("control,router").unwrap()
        );
        assert_eq!(Service::Storage.roles(), Roles::parse("cell").unwrap());
    }
    #[test]
    fn provisioning_requires_an_origin() {
        assert!(validate_origin("http://tenant-storage:3001").is_ok());
        for address in [
            "file:///tmp/sqlite",
            "http://user:pass@host",
            "http://host/path",
            "http://host?query",
        ] {
            assert!(validate_origin(address).is_err());
        }
    }
}
