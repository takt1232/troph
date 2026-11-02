use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSnapshot {
    pub id: String,
    pub name: String,
    pub loaded: u64,
    pub total: Option<u64>,
}

#[derive(Clone, Debug)]
struct TransferEntry {
    sequence: u64,
    name: String,
    loaded: u64,
    total: Option<u64>,
}

#[derive(Default)]
struct Inner {
    next_sequence: u64,
    active: HashMap<String, TransferEntry>,
}

#[derive(Clone, Default)]
pub struct TransferTracker {
    inner: Arc<Mutex<Inner>>,
}

impl TransferTracker {
    pub fn start(&self, name: impl Into<String>, total: Option<u64>) -> TransferGuard {
        let id = Uuid::new_v4().to_string();
        let mut inner = self.lock();
        let sequence = inner.next_sequence;
        inner.next_sequence += 1;
        inner.active.insert(
            id.clone(),
            TransferEntry {
                sequence,
                name: name.into(),
                loaded: 0,
                total,
            },
        );
        drop(inner);
        TransferGuard {
            tracker: self.clone(),
            id,
        }
    }

    pub fn update(&self, id: &str, loaded: u64) {
        if let Some(entry) = self.lock().active.get_mut(id) {
            entry.loaded = loaded;
        }
    }

    pub fn finish(&self, id: &str) {
        self.lock().active.remove(id);
    }

    pub fn snapshot(&self) -> Vec<TransferSnapshot> {
        let inner = self.lock();
        let mut entries: Vec<_> = inner.active.iter().collect();
        entries.sort_by_key(|(_, entry)| entry.sequence);
        entries
            .into_iter()
            .map(|(id, entry)| TransferSnapshot {
                id: id.clone(),
                name: entry.name.clone(),
                loaded: entry.loaded,
                total: entry.total,
            })
            .collect()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub struct TransferGuard {
    tracker: TransferTracker,
    id: String,
}

impl TransferGuard {
    pub fn id(&self) -> &str {
        &self.id
    }
}

impl Drop for TransferGuard {
    fn drop(&mut self) {
        self.tracker.finish(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_progress_in_start_order_and_cleans_up() {
        let tracker = TransferTracker::default();
        let first = tracker.start("folder/first.bin", Some(20));
        tracker.update(first.id(), 5);
        let second = tracker.start("second.bin", None);

        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].name, "folder/first.bin");
        assert_eq!(snapshot[0].loaded, 5);
        assert_eq!(snapshot[0].total, Some(20));
        assert_eq!(snapshot[1].name, "second.bin");
        assert_eq!(snapshot[1].total, None);

        drop(first);
        let snapshot = tracker.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].id, second.id());

        drop(second);
        assert!(tracker.snapshot().is_empty());
    }
}
