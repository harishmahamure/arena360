use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::connection::Connection;

#[derive(Default)]
pub struct ConnectionRegistry {
    connections: RwLock<HashMap<Uuid, Arc<RwLock<Connection>>>>,
    channels: RwLock<HashMap<String, HashSet<Uuid>>>,
}

impl ConnectionRegistry {
    pub async fn insert(&self, id: Uuid, connection: Arc<RwLock<Connection>>) {
        self.connections.write().await.insert(id, connection);
    }
    pub async fn remove(&self, id: Uuid) {
        self.connections.write().await.remove(&id);
        let mut channels = self.channels.write().await;
        channels.retain(|_, subscribers| {
            subscribers.remove(&id);
            !subscribers.is_empty()
        });
    }
    pub async fn subscribe(&self, id: Uuid, channels: &[String]) {
        let mut index = self.channels.write().await;
        for channel in channels {
            index.entry(channel.clone()).or_default().insert(id);
        }
    }
    pub async fn unsubscribe(&self, id: Uuid, channels: &[String]) {
        let mut index = self.channels.write().await;
        for channel in channels {
            if let Some(subscribers) = index.get_mut(channel) {
                subscribers.remove(&id);
            }
        }
        index.retain(|_, subscribers| !subscribers.is_empty());
    }
    pub async fn subscribers(&self, channel: &str) -> Vec<Arc<RwLock<Connection>>> {
        let ids = self
            .channels
            .read()
            .await
            .get(channel)
            .cloned()
            .unwrap_or_default();
        let connections = self.connections.read().await;
        ids.into_iter()
            .filter_map(|id| connections.get(&id).cloned())
            .collect()
    }
}
