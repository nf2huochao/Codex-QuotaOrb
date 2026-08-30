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
    pub waiting_reason: Option<String>,
    pub approval_request_id: Option<String>,
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
    #[serde(default)]
    pub activity: Option<String>,
    #[serde(default)]
    pub waiting_reason: Option<String>,
    #[serde(default)]
    pub approval_request_id: Option<String>,
    pub status: TaskStatus,
    pub token_count: Option<u64>,
    pub updated_at: i64,
    pub acknowledged: bool,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub received_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookDiagnostic {
    pub event: String,
    pub session_id: Option<String>,
    pub turn_id: Option<String>,
    pub received_at: i64,
    pub http_status: Option<u16>,
    pub delivered: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct HookDiagnostics {
    pub last: Option<HookDiagnostic>,
    pub received_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsagePoint {
    pub at: i64,
    pub quota_remaining_percent: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskCounts {
    pub none: u32,
    pub needs_action: u32,
    pub running: u32,
    pub completed: u32,
}

impl TaskCounts {
    pub fn from_tasks(tasks: &[TaskSummary]) -> Self {
        let mut counts = Self::default();
        for task in tasks
            .iter()
            .filter(|task| !(task.acknowledged && task.status == TaskStatus::Completed))
        {
            match task.status {
                TaskStatus::None => counts.none += 1,
                TaskStatus::NeedsAction => counts.needs_action += 1,
                TaskStatus::Running => counts.running += 1,
                TaskStatus::Completed => counts.completed += 1,
            }
        }
        counts
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub status: DataStatus,
    pub changed_at: Option<i64>,
    pub source: Option<String>,
    pub fetched_at: Option<i64>,
    pub quota_remaining_percent: Option<u8>,
    pub quota_resets_at: Option<i64>,
    #[serde(default)]
    pub five_hour_remaining_percent: Option<u8>,
    #[serde(default)]
    pub five_hour_resets_at: Option<i64>,
    pub plan: Option<String>,
    pub reset_credits: Option<u64>,
    pub today_tokens: Option<u64>,
    pub usage_date: Option<String>,
    pub active_task_count: u32,
    #[serde(default)]
    pub task_counts: TaskCounts,
    pub tasks: Vec<TaskSummary>,
    pub error: Option<String>,
    #[serde(default)]
    pub history: Vec<UsagePoint>,
    #[serde(default)]
    pub hook_diagnostics: HookDiagnostics,
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
            waiting_reason: None,
            approval_request_id: None,
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
    fn task_counts_ignore_acknowledged_rows() {
        let tasks = vec![
            TaskSummary {
                id: "none".into(),
                title: "无".into(),
                activity: None,
                waiting_reason: None,
                approval_request_id: None,
                status: TaskStatus::None,
                token_count: None,
                updated_at: 0,
                acknowledged: false,
                source: None,
                turn_id: None,
                received_at: 0,
            },
            TaskSummary {
                id: "red".into(),
                title: "红".into(),
                activity: None,
                waiting_reason: None,
                approval_request_id: Some("r".into()),
                status: TaskStatus::NeedsAction,
                token_count: None,
                updated_at: 0,
                acknowledged: false,
                source: None,
                turn_id: None,
                received_at: 0,
            },
            TaskSummary {
                id: "yellow".into(),
                title: "黄".into(),
                activity: None,
                waiting_reason: None,
                approval_request_id: None,
                status: TaskStatus::Running,
                token_count: None,
                updated_at: 0,
                acknowledged: false,
                source: None,
                turn_id: None,
                received_at: 0,
            },
            TaskSummary {
                id: "green".into(),
                title: "绿".into(),
                activity: None,
                waiting_reason: None,
                approval_request_id: None,
                status: TaskStatus::Completed,
                token_count: None,
                updated_at: 0,
                acknowledged: false,
                source: None,
                turn_id: None,
                received_at: 0,
            },
            TaskSummary {
                id: "acked".into(),
                title: "已验收".into(),
                activity: None,
                waiting_reason: None,
                approval_request_id: None,
                status: TaskStatus::Completed,
                token_count: None,
                updated_at: 0,
                acknowledged: true,
                source: None,
                turn_id: None,
                received_at: 0,
            },
        ];
        assert_eq!(
            TaskCounts::from_tasks(&tasks),
            TaskCounts {
                none: 1,
                needs_action: 1,
                running: 1,
                completed: 1
            }
        );
    }

    #[test]
    fn task_counts_include_acknowledged_running_rows() {
        let tasks = vec![TaskSummary {
            id: "run".into(),
            title: "重新运行".into(),
            activity: None,
            waiting_reason: None,
            approval_request_id: None,
            status: TaskStatus::Running,
            token_count: None,
            updated_at: 0,
            acknowledged: true,
            source: None,
            turn_id: None,
            received_at: 0,
        }];
        assert_eq!(TaskCounts::from_tasks(&tasks).running, 1);
    }
    #[test]
    fn snapshot_is_stale_after_fifteen_minutes() {
        assert_eq!(
            snapshot_status(Some(0), 901, false, true),
            DataStatus::Stale
        );
    }
}
