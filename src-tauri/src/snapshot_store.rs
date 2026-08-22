use crate::domain::{Snapshot, TaskSummary, UsagePoint};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

#[derive(Clone)]
pub struct SnapshotStore {
    state: Arc<Mutex<Snapshot>>,
    acknowledged: Arc<Mutex<HashSet<String>>>,
    tx: watch::Sender<Snapshot>,
}

impl SnapshotStore {
    pub fn new(mut initial: Snapshot) -> (Self, watch::Receiver<Snapshot>) {
        if initial.status == crate::domain::DataStatus::Fresh {
            if let Some(at) = initial.fetched_at.or(initial.changed_at) {
                let point = UsagePoint {
                    at,
                    quota_remaining_percent: initial.quota_remaining_percent,
                };
                if initial.history.last() != Some(&point) {
                    initial.history.push(point);
                }
            }
        }
        if initial.history.len() > 24 {
            initial.history.drain(0..initial.history.len() - 24);
        }
        let acknowledged = initial
            .tasks
            .iter()
            .filter(|task| task.acknowledged)
            .map(|task| task.id.clone())
            .collect();
        let (tx, rx) = watch::channel(initial.clone());
        (
            Self {
                state: Arc::new(Mutex::new(initial)),
                acknowledged: Arc::new(Mutex::new(acknowledged)),
                tx,
            },
            rx,
        )
    }

    pub fn current(&self) -> Snapshot {
        self.state.lock().expect("snapshot lock").clone()
    }

    fn normalize(&self, mut snapshot: Snapshot) -> Snapshot {
        let previous_history = self
            .state
            .lock()
            .expect("snapshot lock")
            .history
            .clone();
        let acknowledged = self.acknowledged.lock().expect("ack lock").clone();
        for task in &mut snapshot.tasks {
            task.acknowledged = acknowledged.contains(&task.id);
        }
        let active = snapshot
            .tasks
            .iter()
            .filter(|task| {
                !task.acknowledged
                    && !matches!(
                        task.status,
                        crate::domain::TaskStatus::Completed | crate::domain::TaskStatus::None
                    )
            })
            .count();
        snapshot.active_task_count = active as u32;
        snapshot.task_counts = crate::domain::TaskCounts::from_tasks(&snapshot.tasks);
        let mut history = previous_history;
        if snapshot.status == crate::domain::DataStatus::Fresh {
            if let Some(at) = snapshot.fetched_at.or(snapshot.changed_at) {
                let point = UsagePoint {
                    at,
                    quota_remaining_percent: snapshot.quota_remaining_percent,
                };
                if history.last() != Some(&point) {
                    history.push(point);
                }
            }
        }
        if history.len() > 24 {
            history.drain(0..history.len() - 24);
        }
        snapshot.history = history;
        snapshot
    }

    pub fn publish(&self, snapshot: Snapshot) {
        let snapshot = self.normalize(snapshot);
        *self.state.lock().expect("snapshot lock") = snapshot.clone();
        let _ = self.tx.send(snapshot);
    }

    pub fn publish_if_changed(&self, snapshot: Snapshot) -> bool {
        let snapshot = self.normalize(snapshot);
        let mut current = self.state.lock().expect("snapshot lock");
        let mut current_content = current.clone();
        current_content.changed_at = None;
        current_content.source = None;
        let mut next_content = snapshot.clone();
        next_content.changed_at = None;
        next_content.source = None;
        if current_content == next_content {
            return false;
        }
        *current = snapshot.clone();
        let _ = self.tx.send(snapshot);
        true
    }

    pub fn acknowledge(&self, task_id: &str) -> bool {
        let mut state = self.state.lock().expect("snapshot lock");
        let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) else {
            return false;
        };
        if !matches!(task.status, crate::domain::TaskStatus::Completed) {
            return false;
        }
        task.acknowledged = true;
        self.acknowledged
            .lock()
            .expect("ack lock")
            .insert(task_id.to_owned());
        let _ = self.tx.send(state.clone());
        true
    }

    pub fn resolve_approval(&self, task_id: &str, decision: &str) -> bool {
        let mut state = self.state.lock().expect("snapshot lock");
        let Some(task) = state.tasks.iter_mut().find(|task| task.id == task_id) else {
            return false;
        };
        if task.status != crate::domain::TaskStatus::NeedsAction {
            return false;
        }
        task.status = if decision == "accept" {
            crate::domain::TaskStatus::Running
        } else {
            crate::domain::TaskStatus::None
        };
        task.waiting_reason = None;
        task.approval_request_id = None;
        state.active_task_count = state
            .tasks
            .iter()
            .filter(|task| {
                !task.acknowledged
                    && matches!(
                        task.status,
                        crate::domain::TaskStatus::Running | crate::domain::TaskStatus::NeedsAction
                    )
            })
            .count() as u32;
        let _ = self.tx.send(state.clone());
        true
    }

    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.tx.subscribe()
    }
}

pub fn empty_snapshot() -> Snapshot {
    Snapshot {
        status: crate::domain::DataStatus::Stale,
        changed_at: None,
        source: None,
        fetched_at: None,
        quota_remaining_percent: None,
        quota_resets_at: None,
        plan: None,
        reset_credits: None,
        today_tokens: None,
        usage_date: None,
        active_task_count: 0,
        task_counts: crate::domain::TaskCounts::default(),
        tasks: Vec::<TaskSummary>::new(),
        error: Some("等待首次连接".into()),
        history: Vec::new(),
        schema_version: "1.0".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DataStatus, TaskStatus};
    fn snapshot() -> Snapshot {
        Snapshot {
            status: DataStatus::Fresh,
            changed_at: Some(1),
            source: Some("test".into()),
            fetched_at: Some(1),
            quota_remaining_percent: Some(72),
            quota_resets_at: None,
            plan: Some("Plus".into()),
            reset_credits: Some(1),
            today_tokens: Some(128400),
            usage_date: None,
            active_task_count: 0,
            task_counts: crate::domain::TaskCounts::default(),
            tasks: vec![TaskSummary {
                id: "t".into(),
                title: "任务".into(),
                activity: None,
                waiting_reason: None,
                approval_request_id: None,
                status: TaskStatus::Completed,
                token_count: None,
                updated_at: 1,
                acknowledged: false,
            }],
            error: None,
            history: Vec::new(),
            schema_version: "1.0".into(),
        }
    }
    #[test]
    fn acknowledgement_survives_refresh() {
        let (store, _) = SnapshotStore::new(snapshot());
        assert!(store.acknowledge("t"));
        let mut next = snapshot();
        next.status = DataStatus::Stale;
        store.publish(next);
        assert!(store.current().tasks[0].acknowledged);
    }
    #[test]
    fn unknown_ack_is_false() {
        let (store, _) = SnapshotStore::new(snapshot());
        assert!(!store.acknowledge("missing"));
    }
    #[test]
    fn running_task_cannot_be_acknowledged() {
        let (store, _) = SnapshotStore::new(snapshot());
        let mut next = snapshot();
        next.tasks[0].status = TaskStatus::Running;
        store.publish(next);
        assert!(!store.acknowledge("t"));
    }

    #[test]
    fn resolving_approval_moves_task_out_of_red_state() {
        let mut current = snapshot();
        current.tasks[0].status = TaskStatus::NeedsAction;
        current.tasks[0].approval_request_id = Some("req-1".into());
        let (store, _) = SnapshotStore::new(current);
        assert!(store.resolve_approval("t", "accept"));
        assert_eq!(store.current().tasks[0].status, TaskStatus::Running);
        assert!(store.current().tasks[0].approval_request_id.is_none());
    }
    #[test]
    fn duplicate_snapshot_is_not_published() {
        let (store, receiver) = SnapshotStore::new(snapshot());
        assert!(!store.publish_if_changed(snapshot()));
        assert!(!receiver.has_changed().unwrap());
        let mut metadata_only = snapshot();
        metadata_only.changed_at = Some(99);
        metadata_only.source = Some("metrics-poll".into());
        assert!(!store.publish_if_changed(metadata_only));
        let mut changed = snapshot();
        changed.today_tokens = Some(128401);
        assert!(store.publish_if_changed(changed));
        assert!(receiver.has_changed().unwrap());
    }
}
