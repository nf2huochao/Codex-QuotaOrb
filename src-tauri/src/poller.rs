use crate::{
    codex_client::CodexClient,
    codex_protocol::{NormalizedTaskEvent, ThreadSummary},
    diagnostics,
    domain::{map_task_status, snapshot_status, Snapshot, TaskEvent, TaskStatus, TaskSummary},
    snapshot_store::SnapshotStore,
};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

pub const POLL_INTERVAL: Duration = Duration::from_secs(120);
#[allow(dead_code)]
pub const STALE_AFTER_SECONDS: i64 = 900;

pub async fn poll_once(
    store: &SnapshotStore,
    client: &CodexClient,
    previous: &Snapshot,
    now: i64,
) -> Snapshot {
    let rate = client.read_rate_limits().await;
    let usage = client.read_usage().await;
    let threads = client.read_threads().await.unwrap_or_default();
    match (rate, usage) {
        (Ok(rate), Ok(usage)) => {
            let tasks = if threads.is_empty() {
                previous.tasks.clone()
            } else {
                map_threads(threads, now)
            };
            let active_task_count = tasks
                .iter()
                .filter(|task| matches!(task.status, TaskStatus::Running | TaskStatus::NeedsAction))
                .count() as u32;
            let snapshot = Snapshot {
                status: snapshot_status(Some(now), now, false, true),
                changed_at: Some(now),
                source: Some("full-poll".into()),
                fetched_at: Some(now),
                quota_remaining_percent: rate.remaining_percent,
                quota_resets_at: rate.resets_at,
                plan: rate.plan,
                reset_credits: rate.reset_credits,
                today_tokens: usage.today_tokens,
                usage_date: usage.usage_date,
                active_task_count,
                tasks,
                error: None,
                schema_version: "1.0".into(),
            };
            store.publish_if_changed(snapshot.clone());
            snapshot
        }
        (rate_result, usage_result) => {
            let mut stale = previous.clone();
            stale.changed_at = Some(now);
            stale.source = Some("full-poll".into());
            stale.status = snapshot_status(previous.fetched_at, now, true, true);
            let error = rate_result.err().or_else(|| usage_result.err());
            stale.error = error
                .as_ref()
                .map(|error| diagnostics::classify(error).message().to_owned())
                .or_else(|| Some("读取 Codex 数据失败".into()));
            store.publish_if_changed(stale.clone());
            stale
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncIntervals {
    pub task: Duration,
    pub metrics: Duration,
    pub full: Duration,
}

impl Default for SyncIntervals {
    fn default() -> Self {
        Self {
            task: Duration::from_secs(1),
            metrics: Duration::from_secs(15),
            full: POLL_INTERVAL,
        }
    }
}

fn map_threads(threads: Vec<ThreadSummary>, now: i64) -> Vec<TaskSummary> {
    threads
        .into_iter()
        .filter_map(|thread| {
            let age = now.saturating_sub(thread.updated_at);
            let status_name = thread.status.to_ascii_lowercase();
            let status = if status_name.contains("approval")
                || status_name.contains("input")
                || status_name.contains("question")
                || status_name.contains("waiting")
            {
                TaskStatus::NeedsAction
            } else if status_name == "active" || status_name == "running" {
                TaskStatus::Running
            } else {
                rollout_status(thread.path.as_deref(), now).or_else(|| {
                    (age <= 6 * 60 * 60 && status_name.contains("completed"))
                        .then_some(TaskStatus::Completed)
                })?
            };
            Some(TaskSummary {
                id: thread.id,
                title: thread.title,
                status,
                token_count: None,
                updated_at: thread.updated_at,
                acknowledged: false,
            })
        })
        .collect()
}

const ROLLOUT_TAIL_BYTES: u64 = 1024 * 1024;
const RUNNING_FRESHNESS_SECONDS: i64 = 180;
const COMPLETED_VISIBLE_SECONDS: i64 = 6 * 60 * 60;

fn rollout_status(path: Option<&str>, now: i64) -> Option<TaskStatus> {
    let Some(path) = path else {
        return None;
    };
    let mut file = File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let modified_at = metadata
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    let start = metadata.len().saturating_sub(ROLLOUT_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity((metadata.len() - start) as usize);
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let text = if start > 0 {
        text.split_once('\n').map(|(_, tail)| tail).unwrap_or("")
    } else {
        &text
    };

    let mut status = None;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        status = rollout_line_status(&value).or(status);
    }

    match status {
        Some(TaskStatus::Completed)
            if now.saturating_sub(modified_at) <= COMPLETED_VISIBLE_SECONDS =>
        {
            Some(TaskStatus::Completed)
        }
        Some(TaskStatus::Running | TaskStatus::NeedsAction)
            if now.saturating_sub(modified_at) <= RUNNING_FRESHNESS_SECONDS =>
        {
            status
        }
        _ => None,
    }
}

fn rollout_line_status(value: &serde_json::Value) -> Option<TaskStatus> {
    let record_type = value.get("type").and_then(serde_json::Value::as_str)?;
    let payload = value.get("payload").unwrap_or(value);
    let payload_type = payload
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();

    if record_type == "event_msg" && payload_type == "task_complete" {
        return Some(TaskStatus::Completed);
    }
    if record_type == "event_msg"
        && (payload_type.contains("approval")
            || payload_type.contains("request_user_input")
            || payload_type.contains("needs_action"))
    {
        return Some(TaskStatus::NeedsAction);
    }
    if record_type == "turn_context" {
        return Some(TaskStatus::Running);
    }
    if record_type == "response_item" {
        if payload_type == "message"
            && payload.get("role").and_then(serde_json::Value::as_str) == Some("user")
        {
            return Some(TaskStatus::Running);
        }
        if payload_type == "custom_tool_call"
            && payload
                .get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|name| name.contains("request_user_input"))
        {
            return Some(TaskStatus::NeedsAction);
        }
        if matches!(
            payload_type.as_str(),
            "custom_tool_call"
                | "custom_tool_call_output"
                | "function_call"
                | "function_call_output"
                | "reasoning"
        ) {
            return Some(TaskStatus::Running);
        }
    }
    if record_type == "event_msg"
        && matches!(
            payload_type.as_str(),
            "agent_message"
                | "agent_reasoning"
                | "token_count"
                | "patch_apply_begin"
                | "patch_apply_end"
        )
    {
        return Some(TaskStatus::Running);
    }
    None
}

pub fn spawn_poll_loop(
    store: SnapshotStore,
    client: Arc<CodexClient>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let intervals = SyncIntervals::default();
        let mut previous = store.current();
        let mut notification_rx = client.subscribe_notifications();
        let mut task_tick = tokio::time::interval(intervals.task);
        let mut metrics_tick = tokio::time::interval(intervals.metrics);
        let mut full_tick = tokio::time::interval(intervals.full);
        task_tick.tick().await;
        metrics_tick.tick().await;
        full_tick.tick().await;
        loop {
            tokio::select! {
                notification = notification_rx.recv() => {
                    if let Ok(notification) = notification {
                        let now = chrono_like_now();
                        let method = notification.get("method").and_then(serde_json::Value::as_str).unwrap_or("");
                        if let Ok(event) = crate::codex_protocol::parse_event_line(&notification.to_string()) {
                            let mut next = previous.clone();
                            apply_task_event(&mut next, event, now);
                            next.changed_at = Some(now);
                            next.source = Some("app-server-event".into());
                            next.status = snapshot_status(next.fetched_at, now, false, client.is_connected());
                            store.publish_if_changed(next);
                            previous = store.current();
                        }
                        if is_metrics_notification(method) {
                            let rate = client.read_rate_limits().await;
                            let usage = client.read_usage().await;
                            if let (Ok(rate), Ok(usage)) = (rate, usage) {
                                let mut next = previous.clone();
                                next.changed_at = Some(now);
                                next.source = Some("app-server-event".into());
                                next.status = snapshot_status(Some(now), now, false, client.is_connected());
                                next.fetched_at = Some(now);
                                next.quota_remaining_percent = rate.remaining_percent;
                                next.quota_resets_at = rate.resets_at;
                                next.plan = rate.plan;
                                next.reset_credits = rate.reset_credits;
                                next.today_tokens = usage.today_tokens;
                                next.usage_date = usage.usage_date;
                                next.error = None;
                                store.publish_if_changed(next);
                                previous = store.current();
                            }
                        }
                    } else if matches!(notification, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                        break;
                    }
                }
                _ = task_tick.tick() => {
                    if !client.is_connected() { break; }
                    let now = chrono_like_now();
                    if let Ok(threads) = client.read_threads().await {
                        let mut next = previous.clone();
                        next.changed_at = Some(now);
                        next.source = Some("task-watch".into());
                        next.tasks = map_threads(threads, now);
                        next.active_task_count = next.tasks.iter().filter(|task| matches!(task.status, TaskStatus::Running | TaskStatus::NeedsAction)).count() as u32;
                        store.publish_if_changed(next.clone());
                        previous = store.current();
                    }
                }
                _ = metrics_tick.tick() => {
                    if !client.is_connected() { break; }
                    let now = chrono_like_now();
                    let rate = client.read_rate_limits().await;
                    let usage = client.read_usage().await;
                    if let (Ok(rate), Ok(usage)) = (rate, usage) {
                        let mut next = previous.clone();
                        next.changed_at = Some(now);
                        next.source = Some("metrics-poll".into());
                        next.status = snapshot_status(Some(now), now, false, client.is_connected());
                        next.fetched_at = Some(now);
                        next.quota_remaining_percent = rate.remaining_percent;
                        next.quota_resets_at = rate.resets_at;
                        next.plan = rate.plan;
                        next.reset_credits = rate.reset_credits;
                        next.today_tokens = usage.today_tokens;
                        next.usage_date = usage.usage_date;
                        next.error = None;
                        store.publish_if_changed(next.clone());
                        previous = store.current();
                    }
                }
                _ = full_tick.tick() => {
                    if !client.is_connected() { break; }
                    let now = chrono_like_now();
                    previous = poll_once(&store, &client, &previous, now).await;
                }
            }
        }
    })
}

fn is_metrics_notification(method: &str) -> bool {
    let method = method.to_ascii_lowercase();
    method.contains("tokenusage")
        || method.contains("ratelimit")
        || method.contains("usage/updated")
}

fn apply_task_event(snapshot: &mut Snapshot, event: NormalizedTaskEvent, now: i64) {
    let task_event = TaskEvent {
        id: event.id.clone(),
        title: event.title.clone(),
        waiting_for_user: event.waiting_for_user,
        running: event.running,
        completed: event.completed,
        token_count: event.token_count,
        updated_at: if event.updated_at == 0 {
            now
        } else {
            event.updated_at
        },
    };
    let status = map_task_status(&task_event);
    if let Some(task) = snapshot
        .tasks
        .iter_mut()
        .find(|task| task.id == task_event.id)
    {
        if task_event.title != "Codex 任务" {
            task.title = task_event.title;
        }
        task.status = status;
        task.token_count = task_event.token_count.or(task.token_count);
        task.updated_at = task_event.updated_at.max(task.updated_at);
    } else {
        snapshot.tasks.push(TaskSummary {
            id: task_event.id,
            title: task_event.title,
            status,
            token_count: task_event.token_count,
            updated_at: task_event.updated_at,
            acknowledged: false,
        });
    }
    snapshot.active_task_count = snapshot
        .tasks
        .iter()
        .filter(|task| matches!(task.status, TaskStatus::Running | TaskStatus::NeedsAction))
        .count() as u32;
}

fn chrono_like_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_intervals_match_product_contract() {
        let intervals = SyncIntervals::default();
        assert_eq!(intervals.task, Duration::from_secs(1));
        assert_eq!(intervals.metrics, Duration::from_secs(15));
        assert_eq!(intervals.full, Duration::from_secs(120));
    }

    #[test]
    fn token_and_rate_limit_notifications_trigger_metrics_refresh() {
        assert!(is_metrics_notification("thread/tokenUsage/updated"));
        assert!(is_metrics_notification("account/rateLimits/updated"));
        assert!(!is_metrics_notification("turn/started"));
    }

    #[test]
    fn rollout_completion_overrides_running_activity() {
        let running: serde_json::Value =
            serde_json::from_str(r#"{"type":"event_msg","payload":{"type":"token_count"}}"#)
                .unwrap();
        let completed: serde_json::Value =
            serde_json::from_str(r#"{"type":"event_msg","payload":{"type":"task_complete"}}"#)
                .unwrap();
        assert_eq!(rollout_line_status(&running), Some(TaskStatus::Running));
        assert_eq!(rollout_line_status(&completed), Some(TaskStatus::Completed));
    }

    #[test]
    fn rollout_user_input_is_needs_action() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"type":"response_item","payload":{"type":"custom_tool_call","name":"request_user_input"}}"#,
        )
        .unwrap();
        assert_eq!(rollout_line_status(&value), Some(TaskStatus::NeedsAction));
    }
}
