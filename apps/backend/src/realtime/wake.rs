use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealtimeWake {
    Postgres(i64),
    Tenant {
        tenant_id: Uuid,
        sequences: Vec<i64>,
    },
}

#[derive(Default)]
struct Pending {
    // Pending work is process-local by design. It survives broadcast lag and handle
    // eviction, but not process restart, matching the former LISTEN/NOTIFY gap.
    postgres: BTreeSet<i64>,
    tenants: HashMap<Uuid, BTreeSet<i64>>,
    tenant_cursors: HashMap<Uuid, i64>,
}

#[derive(Clone)]
pub struct RealtimeHub {
    sender: broadcast::Sender<RealtimeWake>,
    pending: Arc<Mutex<Pending>>,
}

impl RealtimeHub {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            pending: Arc::new(Mutex::new(Pending::default())),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<RealtimeWake> {
        self.sender.subscribe()
    }

    pub fn wake_postgres(&self, id: i64) {
        self.pending
            .lock()
            .expect("realtime pending lock")
            .postgres
            .insert(id);
        let _ = self.sender.send(RealtimeWake::Postgres(id));
    }

    pub fn register_tenant(&self, tenant_id: Uuid, current_sequence: i64) {
        // First registration starts at MAX(sequence): historical analytics rows are
        // not realtime replay. `or_insert` keeps the runtime cursor across DB eviction.
        self.pending
            .lock()
            .expect("realtime pending lock")
            .tenant_cursors
            .entry(tenant_id)
            .or_insert(current_sequence);
    }

    pub fn wake_tenant(&self, tenant_id: Uuid, sequences: &[i64]) {
        if sequences.is_empty() {
            return;
        }
        let mut pending = self.pending.lock().expect("realtime pending lock");
        let cursor = pending.tenant_cursors.entry(tenant_id).or_insert(0);
        let fresh = sequences
            .iter()
            .copied()
            .filter(|sequence| *sequence > *cursor)
            .collect::<Vec<_>>();
        pending
            .tenants
            .entry(tenant_id)
            .or_default()
            .extend(fresh.iter().copied());
        drop(pending);
        if !fresh.is_empty() {
            let _ = self.sender.send(RealtimeWake::Tenant {
                tenant_id,
                sequences: fresh,
            });
        }
    }

    pub fn pending_snapshot(&self) -> (Vec<i64>, Vec<(Uuid, Vec<i64>)>) {
        // The dispatcher drains these sets after every wake, on Lagged, and on its
        // retry tick. A dropped broadcast signal therefore cannot drop runtime work.
        let pending = self.pending.lock().expect("realtime pending lock");
        (
            pending.postgres.iter().copied().collect(),
            pending
                .tenants
                .iter()
                .map(|(tenant_id, sequences)| {
                    (*tenant_id, sequences.iter().copied().collect::<Vec<_>>())
                })
                .collect(),
        )
    }

    pub fn complete_postgres(&self, id: i64) {
        self.pending
            .lock()
            .expect("realtime pending lock")
            .postgres
            .remove(&id);
    }

    pub fn complete_tenant(&self, tenant_id: Uuid, sequence: i64) {
        let mut pending = self.pending.lock().expect("realtime pending lock");
        if let Some(sequences) = pending.tenants.get_mut(&tenant_id) {
            sequences.remove(&sequence);
            if sequences.is_empty() {
                pending.tenants.remove(&tenant_id);
            }
        }
        pending
            .tenant_cursors
            .entry(tenant_id)
            .and_modify(|cursor| *cursor = (*cursor).max(sequence))
            .or_insert(sequence);
    }
}

impl crate::tenancy::TenantCommitNotifier for RealtimeHub {
    fn registered(&self, tenant_id: Uuid, current_sequence: i64) {
        self.register_tenant(tenant_id, current_sequence);
    }

    fn committed(&self, tenant_id: Uuid, sequences: &[i64]) {
        self.wake_tenant(tenant_id, sequences);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_wakes_are_pending_once_and_registration_skips_history() {
        let hub = RealtimeHub::new(4);
        let tenant_id = Uuid::new_v4();
        hub.register_tenant(tenant_id, 7);
        hub.wake_tenant(tenant_id, &[6, 8, 8, 9]);
        hub.wake_tenant(tenant_id, &[8, 9]);
        let (_, tenants) = hub.pending_snapshot();
        assert_eq!(tenants, vec![(tenant_id, vec![8, 9])]);
    }

    #[tokio::test]
    async fn broadcast_lag_does_not_drop_process_local_pending_work() {
        let hub = RealtimeHub::new(1);
        let mut receiver = hub.subscribe();
        hub.wake_postgres(10);
        hub.wake_postgres(11);
        assert!(matches!(
            receiver.recv().await,
            Err(broadcast::error::RecvError::Lagged(_))
        ));
        assert_eq!(hub.pending_snapshot().0, vec![10, 11]);
        hub.complete_postgres(10);
        assert_eq!(hub.pending_snapshot().0, vec![11]);
    }
}
