use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    None,
    NeedsAction,
    Running,
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataStatus {
    Fresh,
    Stale,
    Error,
    Unauthenticated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskEvent {
    pub id: String,
    pub title: String,
    pub waiting_for_user: bool,
    pub running: bool,
    pub completed: bool,
    pub token_count: Option<u64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub token_count: Option<u64>,
    pub updated_at: i64,
    pub acknowledged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub status: DataStatus,
    pub changed_at: Option<i64>,
    pub source: Option<String>,
    pub fetched_at: Option<i64>,
    pub quota_remaining_percent: Option<u8>,
    pub quota_resets_at: Option<i64>,
    pub plan: Option<String>,
    pub reset_credits: Option<u64>,
    pub today_tokens: Option<u64>,
    pub usage_date: Option<String>,
    pub active_task_count: u32,
    pub tasks: Vec<TaskSummary>,
    pub error: Option<String>,
    pub schema_version: String,
}

pub fn map_task_status(event: &TaskEvent) -> TaskStatus {
    if event.waiting_for_user {
        TaskStatus::NeedsAction
    } else if event.running {
        TaskStatus::Running
    } else if event.completed {
        TaskStatus::Completed
    } else {
        TaskStatus::None
    }
}

pub fn snapshot_status(
    last_success: Option<i64>,
    now: i64,
    has_error: bool,
    authenticated: bool,
) -> DataStatus {
    if !authenticated {
        return DataStatus::Unauthenticated;
    }
    let Some(last_success) = last_success else {
        return if has_error {
            DataStatus::Error
        } else {
            DataStatus::Stale
        };
    };
    if now - last_success > 900 {
        DataStatus::Stale
    } else if has_error {
        DataStatus::Error
    } else {
        DataStatus::Fresh
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(waiting_for_user: bool, running: bool, completed: bool) -> TaskEvent {
        TaskEvent {
            id: "t".into(),
            title: "任务".into(),
            waiting_for_user,
            running,
            completed,
            token_count: None,
            updated_at: 0,
        }
    }
    #[test]
    fn needs_action_has_red_priority() {
        assert_eq!(
            map_task_status(&event(true, true, false)),
            TaskStatus::NeedsAction
        );
    }
    #[test]
    fn running_and_completed_are_mapped() {
        assert_eq!(
            map_task_status(&event(false, true, false)),
            TaskStatus::Running
        );
        assert_eq!(
            map_task_status(&event(false, false, true)),
            TaskStatus::Completed
        );
        assert_eq!(
            map_task_status(&event(false, false, false)),
            TaskStatus::None
        );
    }
    #[test]
    fn snapshot_is_stale_after_fifteen_minutes() {
        assert_eq!(
            snapshot_status(Some(0), 901, false, true),
            DataStatus::Stale
        );
    }
}
