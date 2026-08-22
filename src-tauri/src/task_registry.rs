use crate::{
    codex_protocol::NormalizedTaskEvent,
    domain::{map_task_status, TaskCounts, TaskEvent, TaskStatus, TaskSummary},
};

const RUNNING_KEEP_SECONDS: i64 = 15 * 60;
const COMPLETED_KEEP_SECONDS: i64 = 6 * 60 * 60;

#[derive(Debug, Clone, Default)]
pub struct TaskRegistry {
    tasks: Vec<TaskSummary>,
}

impl TaskRegistry {
    pub fn from_tasks(tasks: &[TaskSummary]) -> Self {
        Self { tasks: tasks.to_vec() }
    }

    pub fn apply_event(&mut self, event: NormalizedTaskEvent, now: i64) {
        let updated_at = if event.updated_at == 0 { now } else { event.updated_at };
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
            if let Some(resolved_id) = event.resolved_request_id.as_deref() {
                if task.approval_request_id.as_deref() == Some(resolved_id) {
                    task.approval_request_id = None;
                    task.waiting_reason = None;
                    task.status = if event.completed { TaskStatus::Completed } else { TaskStatus::Running };
                    task.updated_at = task.updated_at.max(updated_at);
                    return;
                }
            }
            let keep_approval = task.status == TaskStatus::NeedsAction
                && matches!(status, TaskStatus::None | TaskStatus::Running)
                && event.approval_request_id.is_none()
                && !event.waiting_for_user
                && !event.completed;
            if !keep_approval {
                task.status = status;
            }
            if status == TaskStatus::NeedsAction {
                task.waiting_reason = event.waiting_reason.clone().or(task.waiting_reason.clone());
                task.approval_request_id = event.approval_request_id.clone().or(task.approval_request_id.clone());
            } else if !keep_approval {
                task.waiting_reason = None;
                task.approval_request_id = None;
            }
            if !event.title.is_empty() && event.title != "Codex 任务" {
                task.title = event.title;
            }
            task.token_count = event.token_count.or(task.token_count);
            task.updated_at = task.updated_at.max(updated_at);
            return;
        }
        self.tasks.push(TaskSummary {
            id: event.id,
            title: if event.title.is_empty() { "Codex 对话".into() } else { event.title },
            activity: None,
            waiting_reason: event.waiting_reason,
            approval_request_id: event.approval_request_id,
            status,
            token_count: event.token_count,
            updated_at,
            acknowledged: false,
        });
    }

    pub fn merge_sources(&mut self, incoming: Vec<TaskSummary>, now: i64) {
        for candidate in incoming {
            if let Some(task) = self.tasks.iter_mut().find(|task| task.id == candidate.id) {
                let keep_approval = task.status == TaskStatus::NeedsAction
                    && candidate.status != TaskStatus::NeedsAction
                    && task.approval_request_id.is_some();
                if !keep_approval && (candidate.updated_at >= task.updated_at || candidate.status == TaskStatus::NeedsAction) {
                    task.status = candidate.status;
                }
                if candidate.title != "Codex 对话" && candidate.title != "Codex 任务" {
                    task.title = candidate.title;
                }
                task.activity = candidate.activity.or(task.activity.clone());
                task.waiting_reason = candidate.waiting_reason.or(task.waiting_reason.clone());
                task.approval_request_id = candidate.approval_request_id.or(task.approval_request_id.clone());
                task.token_count = candidate.token_count.or(task.token_count);
                task.updated_at = task.updated_at.max(candidate.updated_at);
            } else {
                self.tasks.push(candidate);
            }
        }
        self.tasks.retain(|task| {
            let age = now.saturating_sub(task.updated_at);
            match task.status {
                TaskStatus::NeedsAction | TaskStatus::Running => age <= RUNNING_KEEP_SECONDS || task.approval_request_id.is_some(),
                TaskStatus::Completed => age <= COMPLETED_KEEP_SECONDS,
                TaskStatus::None => task.approval_request_id.is_some(),
            }
        });
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

    fn event(id: &str, waiting: bool, running: bool, completed: bool, updated_at: i64) -> NormalizedTaskEvent {
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
        TaskSummary { id: id.into(), title: format!("任务 {id}"), activity: None, waiting_reason: None, approval_request_id: None, status, token_count: None, updated_at, acknowledged: false }
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
    fn same_id_updates_in_place() {
        let mut registry = TaskRegistry::default();
        registry.apply_event(event("a", false, true, false, 100), 100);
        registry.apply_event(event("a", false, false, true, 101), 101);
        assert_eq!(registry.tasks().len(), 1);
        assert_eq!(registry.tasks()[0].status, TaskStatus::Completed);
    }
}
