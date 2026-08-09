use crate::domain::{Snapshot, TaskSummary};
use std::{collections::HashSet, sync::{Arc, Mutex}};
use tokio::sync::watch;

#[derive(Clone)]
pub struct SnapshotStore {
    state: Arc<Mutex<Snapshot>>,
    acknowledged: Arc<Mutex<HashSet<String>>>,
    tx: watch::Sender<Snapshot>,
}

impl SnapshotStore {
    pub fn new(initial: Snapshot) -> (Self, watch::Receiver<Snapshot>) {
        let (tx, rx) = watch::channel(initial.clone());
        (Self { state: Arc::new(Mutex::new(initial)), acknowledged: Arc::new(Mutex::new(HashSet::new())), tx }, rx)
    }

    pub fn current(&self) -> Snapshot { self.state.lock().expect("snapshot lock").clone() }

    pub fn publish(&self, mut snapshot: Snapshot) {
        let acknowledged = self.acknowledged.lock().expect("ack lock").clone();
        for task in &mut snapshot.tasks { task.acknowledged = acknowledged.contains(&task.id); }
        let active = snapshot.tasks.iter().filter(|task| !task.acknowledged && !matches!(task.status, crate::domain::TaskStatus::Completed | crate::domain::TaskStatus::None)).count();
        snapshot.active_task_count = active as u32;
        *self.state.lock().expect("snapshot lock") = snapshot.clone();
        let _ = self.tx.send(snapshot);
    }

    pub fn acknowledge(&self, task_id: &str) -> bool {
        let mut state = self.state.lock().expect("snapshot lock");
        let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) else { return false; };
        task.acknowledged = true;
        self.acknowledged.lock().expect("ack lock").insert(task_id.to_owned());
        let _ = self.tx.send(state.clone());
        true
    }

    pub fn subscribe(&self) -> watch::Receiver<Snapshot> { self.tx.subscribe() }
}

pub fn empty_snapshot() -> Snapshot {
    Snapshot { status: crate::domain::DataStatus::Stale, fetched_at: None, quota_remaining_percent: None, quota_resets_at: None, plan: None, reset_credits: None, today_tokens: None, active_task_count: 0, tasks: Vec::<TaskSummary>::new(), error: Some("等待首次连接".into()), schema_version: "1.0".into() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DataStatus, TaskStatus};
    fn snapshot() -> Snapshot { Snapshot { status: DataStatus::Fresh, fetched_at: Some(1), quota_remaining_percent: Some(72), quota_resets_at: None, plan: Some("Plus".into()), reset_credits: Some(1), today_tokens: Some(128400), active_task_count: 1, tasks: vec![TaskSummary { id: "t".into(), title: "任务".into(), status: TaskStatus::Completed, token_count: None, updated_at: 1, acknowledged: false }], error: None, schema_version: "1.0".into() } }
    #[test]
    fn acknowledgement_survives_refresh() { let (store, _) = SnapshotStore::new(snapshot()); assert!(store.acknowledge("t")); let mut next = snapshot(); next.status = DataStatus::Stale; store.publish(next); assert!(store.current().tasks[0].acknowledged); }
    #[test]
    fn unknown_ack_is_false() { let (store, _) = SnapshotStore::new(snapshot()); assert!(!store.acknowledge("missing")); }
}
