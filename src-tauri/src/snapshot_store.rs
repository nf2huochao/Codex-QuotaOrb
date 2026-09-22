use crate::domain::{Snapshot, TaskSummary, UsagePoint};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

fn local_hour_bucket(at: i64) -> Option<(i64, String)> {
    let utc = time::OffsetDateTime::from_unix_timestamp(at).ok()?;
    let local = utc.to_offset(time::UtcOffset::current_local_offset().ok()?);
    let bucket = local
        .replace_minute(0)
        .ok()?
        .replace_second(0)
        .ok()?
        .replace_nanosecond(0)
        .ok()?;
    Some((bucket.unix_timestamp(), local.date().to_string()))
}

fn merge_hourly_history(
    previous: &[UsagePoint],
    at: Option<i64>,
    quota: Option<u8>,
) -> Vec<UsagePoint> {
    let Some(at) = at else {
        return previous.to_vec();
    };
    let Some(quota) = quota else {
        return previous.to_vec();
    };
    let Some((bucket, _date)) = local_hour_bucket(at) else {
        return previous.to_vec();
    };
    let mut history: Vec<UsagePoint> = previous.to_vec();
    if let Some(point) = history.iter_mut().find(|point| point.at == bucket) {
        point.quota_remaining_percent = Some(quota);
    } else {
        history.push(UsagePoint {
            at: bucket,
            quota_remaining_percent: Some(quota),
        });
    }
    history.sort_by_key(|point| point.at);
    if history.len() > 168 {
        history.drain(0..history.len() - 168);
    }
    history
}

fn history_cycle_key(reset_at: Option<i64>) -> Option<i64> {
    reset_at.map(|timestamp| timestamp - timestamp.rem_euclid(3600))
}

fn trim_history(mut history: Vec<UsagePoint>) -> Vec<UsagePoint> {
    history.sort_by_key(|point| point.at);
    if history.len() > 168 {
        history.drain(0..history.len() - 168);
    }
    history
}

fn cycle_start_bucket(cycle_key: Option<i64>) -> Option<i64> {
    cycle_key.and_then(|key| local_hour_bucket(key - 168 * 3600).map(|(bucket, _)| bucket))
}

fn seed_cycle_baseline(mut history: Vec<UsagePoint>, cycle_key: Option<i64>) -> Vec<UsagePoint> {
    let Some(start) = cycle_start_bucket(cycle_key) else {
        return history;
    };
    if let Some(point) = history.iter_mut().find(|point| {
        local_hour_bucket(point.at)
            .map(|(bucket, _)| bucket == start)
            .unwrap_or(false)
    }) {
        point.quota_remaining_percent = Some(100);
    } else {
        history.push(UsagePoint {
            at: start,
            quota_remaining_percent: Some(100),
        });
    }
    trim_history(history)
}

#[derive(Clone)]
pub struct SnapshotStore {
    state: Arc<Mutex<Snapshot>>,
    acknowledged: Arc<Mutex<HashSet<String>>>,
    tx: watch::Sender<Snapshot>,
}

impl SnapshotStore {
    pub fn new(mut initial: Snapshot) -> (Self, watch::Receiver<Snapshot>) {
        if initial.schema_version == "1.0" {
            initial.history.clear();
        }
        initial.schema_version = crate::domain::SNAPSHOT_SCHEMA_VERSION.into();
        initial.history_cycle_key = initial
            .history_cycle_key
            .or_else(|| history_cycle_key(initial.quota_resets_at));
        initial.previous_history = trim_history(initial.previous_history);
        initial.task_counts = crate::domain::TaskCounts::from_tasks(&initial.tasks);
        initial.active_task_count = initial.task_counts.needs_action + initial.task_counts.running;
        if initial.status == crate::domain::DataStatus::Fresh {
            let history = merge_hourly_history(
                &initial.history,
                initial.fetched_at.or(initial.changed_at),
                initial.quota_remaining_percent,
            );
            initial.history = seed_cycle_baseline(history, initial.history_cycle_key);
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

    fn normalize(&self, mut snapshot: Snapshot, previous_snapshot: &Snapshot) -> Snapshot {
        let legacy_schema = snapshot.schema_version == "1.0";
        if legacy_schema {
            snapshot.history.clear();
            snapshot.history_cycle_key = None;
            snapshot.previous_history.clear();
            snapshot.previous_history_cycle_key = None;
        }
        let incoming_history = snapshot.history.clone();
        let incoming_history_cycle_key = snapshot.history_cycle_key;
        let incoming_previous_history = snapshot.previous_history.clone();
        let incoming_previous_history_cycle_key = snapshot.previous_history_cycle_key;
        let from_cache = snapshot.source.as_deref() == Some("cache-unconfirmed");
        let previous_history = previous_snapshot.history.clone();
        let previous_cycle_key = previous_snapshot
            .history_cycle_key
            .or_else(|| history_cycle_key(previous_snapshot.quota_resets_at));
        let detected_cycle_key =
            incoming_history_cycle_key.or_else(|| history_cycle_key(snapshot.quota_resets_at));
        let mut current_history = if from_cache && previous_history.is_empty() {
            incoming_history
        } else {
            previous_history
        };
        let mut current_cycle_key = if from_cache && previous_cycle_key.is_none() {
            incoming_history_cycle_key.or_else(|| history_cycle_key(snapshot.quota_resets_at))
        } else {
            previous_cycle_key
        };
        let mut previous_history = if from_cache && previous_snapshot.previous_history.is_empty() {
            incoming_previous_history
        } else {
            previous_snapshot.previous_history.clone()
        };
        let mut previous_history_cycle_key =
            if from_cache && previous_snapshot.previous_history_cycle_key.is_none() {
                incoming_previous_history_cycle_key
            } else {
                previous_snapshot.previous_history_cycle_key
            };
        snapshot.schema_version = crate::domain::SNAPSHOT_SCHEMA_VERSION.into();
        if matches!(
            snapshot.source.as_deref(),
            Some(
                "full-poll"
                    | "task-watch"
                    | "metrics-poll"
                    | "manual-refresh"
                    | "desktop-task-watch"
            )
        ) {
            for task in &mut snapshot.tasks {
                let Some(previous_task) = previous_snapshot
                    .tasks
                    .iter()
                    .find(|item| item.id == task.id)
                else {
                    continue;
                };
                let authoritative = matches!(
                    previous_task.source.as_deref(),
                    Some(
                        "hook"
                            | "desktop-accessibility"
                            | "desktop-state"
                            | "app-server-event"
                            | "local-session",
                    )
                );
                let same_turn = previous_task.turn_id.is_none()
                    || task.turn_id.is_none()
                    || previous_task.turn_id == task.turn_id;
                if authoritative
                    && same_turn
                    && previous_task.status != crate::domain::TaskStatus::None
                    && task.source != previous_task.source
                    && !(task.source.as_deref() == Some("local-session")
                        && task.updated_at > previous_task.updated_at)
                {
                    task.status = previous_task.status;
                    task.waiting_reason = previous_task.waiting_reason.clone();
                    task.approval_request_id = previous_task.approval_request_id.clone();
                    task.source = previous_task.source.clone();
                    task.turn_id = previous_task.turn_id.clone();
                    task.received_at = previous_task.received_at;
                    task.updated_at = task.updated_at.max(previous_task.updated_at);
                }
            }
        }
        let mut acknowledged = self.acknowledged.lock().expect("ack lock");
        for task in &snapshot.tasks {
            if previous_snapshot.tasks.iter().any(|previous| {
                previous.id == task.id
                    && (task.turn_id.is_some() && previous.turn_id != task.turn_id
                        || previous.status == crate::domain::TaskStatus::Completed
                            && matches!(
                                task.status,
                                crate::domain::TaskStatus::Running
                                    | crate::domain::TaskStatus::NeedsAction
                            ))
            }) {
                acknowledged.remove(&task.id);
            }
        }
        for task in &mut snapshot.tasks {
            task.acknowledged = acknowledged.contains(&task.id);
        }
        drop(acknowledged);
        let active = snapshot
            .tasks
            .iter()
            .filter(|task| {
                !(task.acknowledged && task.status == crate::domain::TaskStatus::Completed)
                    && !matches!(
                        task.status,
                        crate::domain::TaskStatus::Completed | crate::domain::TaskStatus::None
                    )
            })
            .count();
        snapshot.active_task_count = active as u32;
        snapshot.task_counts = crate::domain::TaskCounts::from_tasks(&snapshot.tasks);
        let history = if snapshot.status == crate::domain::DataStatus::Fresh {
            if let (Some(old_cycle), Some(new_cycle)) = (current_cycle_key, detected_cycle_key) {
                if old_cycle != new_cycle {
                    previous_history = trim_history(current_history.clone());
                    previous_history_cycle_key = Some(old_cycle);
                    current_history.clear();
                }
            }
            current_cycle_key = detected_cycle_key.or(current_cycle_key);
            let history = merge_hourly_history(
                &current_history,
                snapshot.fetched_at.or(snapshot.changed_at),
                snapshot.quota_remaining_percent,
            );
            seed_cycle_baseline(history, current_cycle_key)
        } else {
            current_history
        };
        snapshot.history = history;
        snapshot.history_cycle_key = current_cycle_key;
        snapshot.previous_history = trim_history(previous_history);
        snapshot.previous_history_cycle_key = previous_history_cycle_key;
        snapshot
    }

    pub fn publish(&self, snapshot: Snapshot) {
        let mut current = self.state.lock().expect("snapshot lock");
        let snapshot = self.normalize(snapshot, &current);
        *current = snapshot.clone();
        let _ = self.tx.send(snapshot);
    }

    pub fn publish_if_changed(&self, snapshot: Snapshot) -> bool {
        self.update(|current| *current = snapshot)
    }

    /// Modify only the owning subsystem's fields while holding the snapshot
    /// lock. Slow metrics reads must never publish an old task list.
    pub fn update(&self, apply: impl FnOnce(&mut Snapshot)) -> bool {
        let mut current = self.state.lock().expect("snapshot lock");
        let mut snapshot = current.clone();
        apply(&mut snapshot);
        let snapshot = self.normalize(snapshot, &current);
        let mut current_content = current.clone();
        current_content.changed_at = None;
        current_content.source = None;
        let mut next_content = snapshot.clone();
        next_content.changed_at = None;
        next_content.source = None;
        // Observation heartbeats are not visible changes. Still retain them
        // internally for the short read-failure grace period.
        for task in &mut current_content.tasks {
            task.received_at = 0;
        }
        for task in &mut next_content.tasks {
            task.received_at = 0;
        }
        if current_content == next_content {
            *current = snapshot;
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
        state.task_counts = crate::domain::TaskCounts::from_tasks(&state.tasks);
        state.active_task_count = state.task_counts.needs_action + state.task_counts.running;
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
        task.source = Some("hook".into());
        task.received_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or_default();
        task.waiting_reason = None;
        task.approval_request_id = None;
        state.task_counts = crate::domain::TaskCounts::from_tasks(&state.tasks);
        state.active_task_count = state.task_counts.needs_action + state.task_counts.running;
        let _ = self.tx.send(state.clone());
        true
    }

    pub fn subscribe(&self) -> watch::Receiver<Snapshot> {
        self.tx.subscribe()
    }

    pub fn record_hook_diagnostic(&self, mut diagnostic: crate::domain::HookDiagnostics) {
        let mut state = self.state.lock().expect("snapshot lock");
        diagnostic.received_count = state.hook_diagnostics.received_count.saturating_add(1);
        state.hook_diagnostics = diagnostic;
        state.changed_at = state
            .hook_diagnostics
            .last
            .as_ref()
            .map(|item| item.received_at)
            .or(state.changed_at);
        state.source = Some("hook-diagnostic".into());
        if let Err(error) = self.tx.send(state.clone()) {
            eprintln!("hook diagnostic snapshot publish failed: {error}");
        }
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
        five_hour_remaining_percent: None,
        five_hour_resets_at: None,
        plan: None,
        reset_credits: None,
        today_tokens: None,
        usage_date: None,
        active_task_count: 0,
        task_counts: crate::domain::TaskCounts::default(),
        tasks: Vec::<TaskSummary>::new(),
        error: Some("等待首次连接".into()),
        history: Vec::new(),
        history_cycle_key: None,
        previous_history: Vec::new(),
        previous_history_cycle_key: None,
        hook_diagnostics: crate::domain::HookDiagnostics::default(),
        schema_version: crate::domain::SNAPSHOT_SCHEMA_VERSION.into(),
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
            five_hour_remaining_percent: None,
            five_hour_resets_at: None,
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
                source: None,
                turn_id: None,
                received_at: 1,
            }],
            error: None,
            history: Vec::new(),
            history_cycle_key: None,
            previous_history: Vec::new(),
            previous_history_cycle_key: None,
            hook_diagnostics: crate::domain::HookDiagnostics::default(),
            schema_version: crate::domain::SNAPSHOT_SCHEMA_VERSION.into(),
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
    fn newer_local_lifecycle_replaces_hook_without_reverting_the_task() {
        let mut initial = snapshot();
        initial.tasks[0].source = Some("hook".into());
        initial.tasks[0].status = TaskStatus::Running;
        initial.tasks[0].turn_id = Some("turn".into());
        let (store, _) = SnapshotStore::new(initial);
        store.update(|snapshot| {
            snapshot.source = Some("task-watch".into());
            snapshot.tasks[0].source = Some("local-session".into());
            snapshot.tasks[0].status = TaskStatus::Completed;
            snapshot.tasks[0].updated_at = 2;
        });
        assert_eq!(store.current().tasks[0].status, TaskStatus::Completed);
    }

    #[test]
    fn clears_legacy_history_when_upgrading_to_weekly_history() {
        let mut initial = snapshot();
        initial.schema_version = "1.0".into();
        initial.status = DataStatus::Stale;
        initial.quota_remaining_percent = None;
        initial.history = vec![UsagePoint {
            at: 1,
            quota_remaining_percent: Some(5),
        }];
        let (store, _) = SnapshotStore::new(initial);
        assert!(store.current().history.is_empty());
        assert_eq!(
            store.current().schema_version,
            crate::domain::SNAPSHOT_SCHEMA_VERSION
        );
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
    fn metrics_update_preserves_new_tasks_and_next_turn_can_be_reviewed() {
        let (store, _) = SnapshotStore::new(snapshot());
        assert!(store.acknowledge("t"));
        store.update(|next| {
            next.source = Some("local-session".into());
            next.tasks[0].status = TaskStatus::Running;
            next.tasks[0].turn_id = Some("next-turn".into());
            next.tasks[0].source = Some("local-session".into());
        });
        store.update(|next| {
            next.source = Some("metrics-poll".into());
            next.quota_remaining_percent = Some(60);
        });
        assert_eq!(store.current().tasks[0].status, TaskStatus::Running);
        assert!(!store.current().tasks[0].acknowledged);
        store.update(|next| {
            next.tasks[0].status = TaskStatus::Completed;
        });
        assert_eq!(store.current().tasks[0].status, TaskStatus::Completed);
        assert_eq!(store.current().task_counts.completed, 1);
        assert_eq!(store.current().quota_remaining_percent, Some(60));
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

    #[test]
    fn poll_snapshot_cannot_replace_live_hook_status() {
        let mut initial = snapshot();
        initial.tasks[0].status = TaskStatus::Running;
        initial.tasks[0].source = Some("hook".into());
        initial.tasks[0].turn_id = Some("turn-1".into());
        let (store, _) = SnapshotStore::new(initial.clone());
        let mut polled = initial;
        polled.source = Some("task-watch".into());
        polled.tasks[0].status = TaskStatus::Completed;
        polled.tasks[0].source = Some("poll".into());
        store.publish(polled);
        assert_eq!(store.current().tasks[0].status, TaskStatus::Running);
        assert_eq!(store.current().tasks[0].source.as_deref(), Some("hook"));
    }

    #[test]
    fn hourly_history_replaces_the_same_hour_with_latest_quota() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let hour = now - now.rem_euclid(3600);
        let first = merge_hourly_history(&[], Some(hour + 10), Some(80));
        let second = merge_hourly_history(&first, Some(hour + 3500), Some(72));
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].quota_remaining_percent, Some(72));
        assert_eq!(second[0].at, local_hour_bucket(hour + 10).unwrap().0);
    }

    #[test]
    fn hourly_history_keeps_a_rolling_seven_day_window() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let hour = now - now.rem_euclid(3600);
        let mut history = Vec::new();
        for index in 0..200 {
            history = merge_hourly_history(
                &history,
                Some(hour - (199 - index) * 3600),
                Some(index as u8),
            );
        }
        assert_eq!(history.len(), 168);
        assert!(history.windows(2).all(|points| points[0].at < points[1].at));
        assert_eq!(
            history.last().map(|point| point.at),
            Some(local_hour_bucket(hour).unwrap().0)
        );
        assert_eq!(
            history.first().map(|point| point.at),
            Some(local_hour_bucket(hour - 167 * 3600).unwrap().0)
        );
        let replaced = merge_hourly_history(&history, Some(hour - 10), Some(99));
        assert_eq!(replaced.len(), 168);
        assert_eq!(
            replaced
                .iter()
                .find(|point| point.at == local_hour_bucket(hour - 10).unwrap().0)
                .and_then(|point| point.quota_remaining_percent),
            Some(99)
        );
    }

    #[test]
    fn cache_snapshot_restores_history_across_restart() {
        let (store, _) = SnapshotStore::new(empty_snapshot());
        let mut cached = snapshot();
        cached.status = DataStatus::Stale;
        cached.source = Some("cache-unconfirmed".into());
        cached.history = vec![UsagePoint {
            at: 10,
            quota_remaining_percent: Some(64),
        }];
        cached.history_cycle_key = Some(0);
        store.publish(cached);
        assert_eq!(store.current().history.len(), 1);
        assert_eq!(store.current().history[0].quota_remaining_percent, Some(64));
        assert_eq!(store.current().history_cycle_key, Some(0));
    }

    #[test]
    fn weekly_history_always_starts_at_one_hundred_percent() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut initial = snapshot();
        initial.fetched_at = Some(now);
        initial.changed_at = Some(now);
        initial.quota_remaining_percent = Some(94);
        initial.quota_resets_at = Some(now + 168 * 3600);
        let (store, _) = SnapshotStore::new(initial);
        let current = store.current();
        assert_eq!(
            current
                .history
                .first()
                .and_then(|point| point.quota_remaining_percent),
            Some(100)
        );
        assert_eq!(
            current
                .history
                .last()
                .and_then(|point| point.quota_remaining_percent),
            Some(100)
        );
    }

    #[test]
    fn new_weekly_cycle_archives_previous_history() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let old_cycle = now - now.rem_euclid(3600) - 168 * 3600;
        let new_cycle = old_cycle + 168 * 3600;
        let mut initial = snapshot();
        initial.status = DataStatus::Stale;
        initial.fetched_at = Some(now - 3600);
        initial.quota_resets_at = Some(old_cycle);
        initial.history_cycle_key = Some(old_cycle);
        initial.history = vec![UsagePoint {
            at: now - 3600,
            quota_remaining_percent: Some(72),
        }];
        let (store, _) = SnapshotStore::new(initial);

        let mut next = snapshot();
        next.fetched_at = Some(now);
        next.quota_remaining_percent = Some(100);
        next.quota_resets_at = Some(new_cycle);
        store.publish(next);

        let current = store.current();
        assert_eq!(current.history_cycle_key, Some(new_cycle));
        assert_eq!(current.previous_history_cycle_key, Some(old_cycle));
        assert_eq!(current.previous_history.len(), 1);
        assert_eq!(
            current.previous_history[0].quota_remaining_percent,
            Some(72)
        );
        assert!(current
            .history
            .iter()
            .any(|point| point.quota_remaining_percent == Some(100)));
    }
}
