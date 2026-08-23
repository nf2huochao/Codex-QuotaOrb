use crate::{
    codex_protocol::NormalizedTaskEvent,
    domain::{map_task_status, TaskCounts, TaskEvent, TaskStatus, TaskSummary},
};
use std::collections::HashSet;

const RUNNING_KEEP_SECONDS: i64 = 15 * 60;
const COMPLETED_KEEP_SECONDS: i64 = 6 * 60 * 60;

fn source_rank(source: Option<&str>) -> u8 {
    match source.unwrap_or("") {
        "hook" => 5,
        "desktop-accessibility" => 4,
        "desktop-state" => 3,
        "local-session" => 4,
        "app-server-event" => 2,
        "poll" | "history" => 1,
        _ => 0,
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskRegistry {
    tasks: Vec<TaskSummary>,
}

impl TaskRegistry {
    pub fn from_tasks(tasks: &[TaskSummary]) -> Self {
        Self {
            tasks: tasks.to_vec(),
        }
    }

    pub fn apply_event(&mut self, event: NormalizedTaskEvent, now: i64) {
        let updated_at = if event.updated_at == 0 {
            now
        } else {
            event.updated_at
        };
        let task_event = TaskEvent {
            id: event.id.clone(),
            title: event.title.clone(),
            waiting_reason: event.waiting_reason.clone(),
            approval_request_id: event.approval_request_id.clone(),
            waiting_for_user: event.waiting_for_user,
            running: event.running,
            completed: event.completed,
            token_count: event.token_count,
            updated_at,
        };
        let status = map_task_status(&task_event);
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == event.id) {
            let stale_turn = event.completed
                && task.turn_id.is_some()
                && event.turn_id.is_some()
                && task.turn_id != event.turn_id;
            let hook_authority = task.source.as_deref() == Some("hook");
            if let Some(resolved_id) = event.resolved_request_id.as_deref() {
                if task.approval_request_id.as_deref() == Some(resolved_id) {
                    task.approval_request_id = None;
                    task.waiting_reason = None;
                    task.status = if event.completed {
                        TaskStatus::Completed
                    } else {
                        TaskStatus::Running
                    };
                    task.updated_at = task.updated_at.max(updated_at);
                    return;
                }
            }
            let keep_approval = task.status == TaskStatus::NeedsAction
                && matches!(status, TaskStatus::None | TaskStatus::Running)
                && event.approval_request_id.is_none()
                && !event.waiting_for_user
                && !event.completed;
            if !keep_approval && !stale_turn && !hook_authority {
                task.status = status;
            }
            if status == TaskStatus::NeedsAction && !hook_authority {
                task.waiting_reason = event.waiting_reason.clone().or(task.waiting_reason.clone());
                task.approval_request_id = event
                    .approval_request_id
                    .clone()
                    .or(task.approval_request_id.clone());
            } else if !keep_approval && !hook_authority {
                task.waiting_reason = None;
                task.approval_request_id = None;
            }
            if !event.title.is_empty() && event.title != "Codex 任务" {
                task.title = event.title;
            }
            task.token_count = event.token_count.or(task.token_count);
            task.updated_at = task.updated_at.max(updated_at);
            if !hook_authority && !stale_turn {
                task.source = Some("app-server-event".into());
                task.turn_id = event.turn_id.or(task.turn_id.clone());
            }
            task.received_at = task.received_at.max(updated_at);
            return;
        }
        self.tasks.push(TaskSummary {
            id: event.id,
            title: if event.title.is_empty() {
                "Codex 对话".into()
            } else {
                event.title
            },
            activity: None,
            waiting_reason: event.waiting_reason,
            approval_request_id: event.approval_request_id,
            status,
            token_count: event.token_count,
            updated_at,
            acknowledged: false,
            source: Some("app-server-event".into()),
            turn_id: event.turn_id,
            received_at: now,
        });
    }

    pub fn merge_sources(&mut self, incoming: Vec<TaskSummary>, now: i64) {
        for candidate in incoming {
            if let Some(task) = self.tasks.iter_mut().find(|task| task.id == candidate.id) {
                let keep_approval = task.status == TaskStatus::NeedsAction
                    && candidate.status != TaskStatus::NeedsAction
                    && task.approval_request_id.is_some();
                let candidate_is_newer = candidate.updated_at >= task.updated_at;
                let candidate_is_new_turn = candidate.turn_id.is_some()
                    && task.turn_id.is_some()
                    && candidate.turn_id != task.turn_id
                    && candidate.updated_at > task.updated_at;
                let candidate_rank = source_rank(candidate.source.as_deref());
                let task_rank = source_rank(task.source.as_deref());
                let candidate_is_authoritative = candidate_rank > task_rank
                    || (candidate_rank == task_rank && candidate_is_newer)
                    || candidate_is_new_turn;
                if !keep_approval
                    && candidate.status != TaskStatus::None
                    && candidate_is_authoritative
                {
                    task.status = candidate.status;
                }
                if candidate.title != "Codex 对话" && candidate.title != "Codex 任务" {
                    task.title = candidate.title;
                }
                task.activity = candidate.activity.or(task.activity.clone());
                task.waiting_reason = candidate.waiting_reason.or(task.waiting_reason.clone());
                task.approval_request_id = candidate
                    .approval_request_id
                    .or(task.approval_request_id.clone());
                task.token_count = candidate.token_count.or(task.token_count);
                task.updated_at = task.updated_at.max(candidate.updated_at);
                if candidate_is_authoritative {
                    task.source = candidate.source.or(task.source.clone());
                    task.turn_id = candidate.turn_id.or(task.turn_id.clone());
                }
                task.received_at = task.received_at.max(candidate.received_at);
            } else {
                self.tasks.push(candidate);
            }
        }
        self.tasks.retain(|task| {
            let age = now.saturating_sub(task.updated_at);
            match task.status {
                TaskStatus::NeedsAction | TaskStatus::Running => {
                    matches!(
                        task.source.as_deref(),
                        Some("desktop-state" | "desktop-accessibility")
                    ) || age <= RUNNING_KEEP_SECONDS
                        || task.approval_request_id.is_some()
                }
                TaskStatus::Completed => age <= COMPLETED_KEEP_SECONDS,
                TaskStatus::None => task.approval_request_id.is_some(),
            }
        });
    }

    /// Replace the live view for a source. Local session scans are bounded to
    /// recent files, so a task that disappears from that authoritative scan
    /// must not remain as a phantom running row from an earlier tick.
    pub fn replace_source_tasks(&mut self, source: &str, incoming: Vec<TaskSummary>, now: i64) {
        let incoming_ids: HashSet<String> = incoming.iter().map(|task| task.id.clone()).collect();
        self.tasks.retain(|task| {
            task.source.as_deref() != Some(source)
                || incoming_ids.contains(&task.id)
                || task.status == TaskStatus::Completed
        });
        self.merge_sources(incoming, now);
    }

    pub fn tasks(&self) -> Vec<TaskSummary> {
        let mut tasks = self.tasks.clone();
        tasks.sort_by_key(|task| std::cmp::Reverse(task.updated_at));
        tasks
    }

    pub fn counts(&self) -> TaskCounts {
        TaskCounts::from_tasks(&self.tasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        id: &str,
        waiting: bool,
        running: bool,
        completed: bool,
        updated_at: i64,
    ) -> NormalizedTaskEvent {
        NormalizedTaskEvent {
            id: id.into(),
            turn_id: Some(format!("turn-{id}")),
            item_id: Some(format!("item-{id}")),
            request_id: waiting.then(|| format!("request-{id}")),
            resolved_request_id: None,
            title: format!("任务 {id}"),
            waiting_reason: waiting.then(|| "需要确认".into()),
            approval_request_id: waiting.then(|| format!("request-{id}")),
            waiting_for_user: waiting,
            running,
            completed,
            token_count: None,
            updated_at,
        }
    }

    fn task(id: &str, status: TaskStatus, updated_at: i64) -> TaskSummary {
        TaskSummary {
            id: id.into(),
            title: format!("任务 {id}"),
            activity: None,
            waiting_reason: None,
            approval_request_id: None,
            status,
            token_count: None,
            updated_at,
            acknowledged: false,
            source: None,
            turn_id: None,
            received_at: updated_at,
        }
    }

    #[test]
    fn partial_thread_poll_does_not_drop_event_tasks() {
        let mut registry = TaskRegistry::default();
        registry.apply_event(event("a", false, true, false, 100), 100);
        registry.apply_event(event("b", false, true, false, 100), 100);
        registry.apply_event(event("c", false, true, false, 100), 100);
        registry.merge_sources(vec![task("a", TaskStatus::Running, 101)], 101);
        assert_eq!(registry.counts().running, 3);
    }

    #[test]
    fn approval_state_survives_active_poll() {
        let mut registry = TaskRegistry::default();
        registry.apply_event(event("a", true, false, false, 100), 100);
        registry.merge_sources(vec![task("a", TaskStatus::Running, 101)], 101);
        let current = registry.tasks();
        assert_eq!(current[0].status, TaskStatus::NeedsAction);
        assert_eq!(registry.counts().needs_action, 1);
    }

    #[test]
    fn unknown_thread_poll_does_not_erase_event_status() {
        let mut registry = TaskRegistry::default();
        registry.apply_event(event("a", false, true, false, 100), 100);
        registry.merge_sources(vec![task("a", TaskStatus::None, 101)], 101);
        assert_eq!(registry.tasks()[0].status, TaskStatus::Running);
    }

    #[test]
    fn local_scan_removes_a_missing_phantom_task() {
        let mut registry = TaskRegistry::default();
        let mut current = task("a", TaskStatus::Running, 100);
        current.source = Some("local-session".into());
        registry.merge_sources(vec![current], 100);
        registry.replace_source_tasks("local-session", Vec::new(), 101);
        assert!(registry.tasks().is_empty());
    }

    #[test]
    fn same_id_updates_in_place() {
        let mut registry = TaskRegistry::default();
        registry.apply_event(event("a", false, true, false, 100), 100);
        registry.apply_event(event("a", false, false, true, 101), 101);
        assert_eq!(registry.tasks().len(), 1);
        assert_eq!(registry.tasks()[0].status, TaskStatus::Completed);
    }

    #[test]
    fn polling_completed_cannot_overwrite_hook_running_state() {
        let mut registry = TaskRegistry::default();
        registry.apply_event(event("a", false, true, false, 100), 100);
        registry.tasks[0].source = Some("hook".into());
        registry.tasks[0].turn_id = Some("turn-a".into());
        registry.merge_sources(vec![task("a", TaskStatus::Completed, 200)], 200);
        assert_eq!(registry.tasks()[0].status, TaskStatus::Running);
        assert_eq!(registry.tasks()[0].source.as_deref(), Some("hook"));
    }

    #[test]
    fn stale_stop_from_an_older_turn_is_ignored() {
        let mut registry = TaskRegistry::default();
        registry.apply_event(event("a", false, true, false, 100), 100);
        registry.tasks[0].source = Some("hook".into());
        registry.tasks[0].turn_id = Some("turn-new".into());
        let mut stale = event("a", false, false, true, 200);
        stale.turn_id = Some("turn-old".into());
        registry.apply_event(stale, 200);
        assert_eq!(registry.tasks()[0].status, TaskStatus::Running);
    }

    #[test]
    fn newer_desktop_turn_replaces_an_older_hook_turn() {
        let mut registry = TaskRegistry::default();
        let mut old = task("a", TaskStatus::Completed, 100);
        old.source = Some("hook".into());
        old.turn_id = Some("turn-old".into());
        registry.merge_sources(vec![old], 100);
        let mut current = task("a", TaskStatus::Running, 200);
        current.source = Some("desktop-state".into());
        current.turn_id = Some("turn-new".into());
        registry.merge_sources(vec![current], 200);
        assert_eq!(registry.tasks()[0].status, TaskStatus::Running);
        assert_eq!(registry.tasks()[0].turn_id.as_deref(), Some("turn-new"));
        assert_eq!(registry.tasks()[0].source.as_deref(), Some("desktop-state"));
    }

    #[test]
    fn desktop_state_overrides_a_newer_metadata_timestamp() {
        let mut registry = TaskRegistry::default();
        let mut metadata = task("a", TaskStatus::None, 500);
        metadata.source = Some("poll".into());
        registry.merge_sources(vec![metadata], 500);
        let mut desktop = task("a", TaskStatus::Running, 100);
        desktop.source = Some("desktop-state".into());
        desktop.turn_id = Some("turn-live".into());
        registry.merge_sources(vec![desktop], 500);
        assert_eq!(registry.tasks()[0].status, TaskStatus::Running);
        assert_eq!(registry.tasks()[0].source.as_deref(), Some("desktop-state"));
    }
}
