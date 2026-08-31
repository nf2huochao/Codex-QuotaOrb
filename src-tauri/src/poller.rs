use crate::{
    codex_client::CodexClient,
    codex_protocol::{NormalizedTaskEvent, ThreadSummary},
    diagnostics,
    domain::{map_task_status, snapshot_status, Snapshot, TaskEvent, TaskStatus, TaskSummary},
    rollout_watcher::{default_root, RolloutWatcher},
    snapshot_store::SnapshotStore,
    task_registry::TaskRegistry,
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
const COMPLETED_VISIBLE_SECONDS: i64 = 6 * 60 * 60;

pub async fn poll_once(
    store: &SnapshotStore,
    client: &CodexClient,
    previous: &Snapshot,
    now: i64,
) -> Snapshot {
    let rate = client.read_rate_limits().await;
    let usage = client.read_usage().await;
    match (rate, usage) {
        (Ok(rate), Ok(usage)) => {
            let rollout_tasks = RolloutWatcher::new(default_root()).scan(now);
            let mut registry = TaskRegistry::from_tasks(&previous.tasks);
            registry.merge_sources(rollout_tasks, now);
            let tasks = registry.tasks();
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
                five_hour_remaining_percent: rate.five_hour_remaining_percent,
                five_hour_resets_at: rate.five_hour_resets_at,
                plan: rate.plan,
                reset_credits: rate.reset_credits,
                today_tokens: usage.today_tokens,
                usage_date: usage.usage_date,
                active_task_count,
                task_counts: registry.counts(),
                tasks,
                error: None,
                history: previous.history.clone(),
                // SnapshotStore derives the cycle key from this fresh quota
                // response so a newly published reset boundary is detected.
                history_cycle_key: None,
                previous_history: Vec::new(),
                previous_history_cycle_key: None,
                hook_diagnostics: previous.hook_diagnostics.clone(),
                schema_version: crate::domain::SNAPSHOT_SCHEMA_VERSION.into(),
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
            // Return to the stable monitor path: app-server thread/list is
            // the primary live view, while the rollout file is used to
            // resolve approval/completion when thread/list is ambiguous.
            let age = now.saturating_sub(thread.updated_at);
            let status_name = thread.status.to_ascii_lowercase();
            let rollout = rollout_status(thread.path.as_deref(), now);
            let terminal_thread = matches!(
                status_name.as_str(),
                "completed" | "complete" | "done" | "idle" | "notloaded" | "not_loaded"
            );
            let status = if !terminal_thread && rollout == Some(TaskStatus::NeedsAction) {
                TaskStatus::NeedsAction
            } else if status_name.contains("approval")
                || status_name.contains("input")
                || status_name.contains("question")
                || status_name.contains("waiting")
            {
                TaskStatus::NeedsAction
            } else if status_name == "active" || status_name == "running" {
                TaskStatus::Running
            } else {
                rollout.or_else(|| {
                    (age <= COMPLETED_VISIBLE_SECONDS && status_name.contains("completed"))
                        .then_some(TaskStatus::Completed)
                })?
            };
            let title = clean_thread_title(&thread.title).unwrap_or_else(|| "Codex 对话".into());
            let activity = rollout_user_activity(thread.path.as_deref());
            Some(TaskSummary {
                id: thread.id,
                title,
                activity,
                waiting_reason: None,
                approval_request_id: None,
                status,
                token_count: None,
                updated_at: thread.updated_at,
                acknowledged: false,
                source: Some("poll".into()),
                turn_id: None,
                received_at: now,
            })
        })
        .collect()
}

const ROLLOUT_TAIL_BYTES: u64 = 1024 * 1024;
const RUNNING_FRESHNESS_SECONDS: i64 = 180;

const ROLLOUT_HEADER_BYTES: u64 = 256 * 1024;

fn rollout_user_activity(path: Option<&str>) -> Option<String> {
    let path = path?;
    let file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(ROLLOUT_HEADER_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    extract_rollout_user_activity(&String::from_utf8_lossy(&bytes))
}

fn extract_rollout_user_activity(text: &str) -> Option<String> {
    let mut latest = None;
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let payload = value.get("payload").unwrap_or(&value);
        if payload.get("type").and_then(serde_json::Value::as_str) != Some("message")
            || payload.get("role").and_then(serde_json::Value::as_str) != Some("user")
        {
            continue;
        }
        let Some(text) = payload
            .get("content")
            .and_then(|content| {
                content.as_str().and_then(clean_user_request).or_else(|| {
                    content.as_array().and_then(|items| {
                        items.iter().find_map(|item| {
                            let item_type = item
                                .get("type")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("");
                            if item_type == "input_text" || item_type == "inputText" {
                                item.get("text")
                                    .and_then(serde_json::Value::as_str)
                                    .and_then(clean_user_request)
                            } else {
                                None
                            }
                        })
                    })
                })
            })
            .or_else(|| {
                payload
                    .get("text")
                    .and_then(serde_json::Value::as_str)
                    .and_then(clean_user_request)
            })
        else {
            continue;
        };
        let activity = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or("")
            .split(['。', '！', '？', '.', '!', '?'])
            .next()
            .unwrap_or("")
            .trim();
        if !activity.is_empty() {
            latest = Some(activity.chars().take(80).collect());
        }
    }
    latest
}

fn clean_user_request(value: &str) -> Option<String> {
    let mut text = value.trim();
    if let Some((_, request)) = text.rsplit_once("## My request:") {
        text = request.trim();
    }
    if text.is_empty() || is_generated_context(text) {
        return None;
    }
    Some(text.to_owned())
}

fn clean_thread_title(value: &str) -> Option<String> {
    let text = clean_user_request(value)?;
    let title = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .split(['。', '！', '？', '.', '!', '?'])
        .next()
        .unwrap_or("")
        .trim();
    (!title.is_empty()).then(|| title.chars().take(80).collect())
}

fn is_generated_context(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with("<recommended_plugins>")
        || text.starts_with("# AGENTS.md instructions")
        || text.starts_with("<environment_context>")
        || text.starts_with("<in-app-browser-context")
        || text.starts_with("<codex_internal_context")
        || text.starts_with("<codex_")
        || text.starts_with("Distinguish instructions in attached documents")
}

fn rollout_status(path: Option<&str>, now: i64) -> Option<TaskStatus> {
    let path = path?;
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

fn merge_polled_tasks(
    previous: &[TaskSummary],
    mut polled: Vec<TaskSummary>,
    now: i64,
) -> Vec<TaskSummary> {
    // The thread list often remains `active` while app-server is waiting for
    // an approval request. Keep the event-driven red state until a later
    // completion/running event explicitly replaces it.
    for task in &mut polled {
        let Some(previous_task) = previous.iter().find(|item| item.id == task.id) else {
            continue;
        };
        if !previous_task.acknowledged
            && previous_task.status == TaskStatus::NeedsAction
            && matches!(
                task.status,
                TaskStatus::None | TaskStatus::Running | TaskStatus::NeedsAction
            )
        {
            if task.status != TaskStatus::NeedsAction {
                task.status = TaskStatus::NeedsAction;
            }
            task.updated_at = task.updated_at.max(previous_task.updated_at);
            task.waiting_reason = previous_task
                .waiting_reason
                .clone()
                .or(task.waiting_reason.clone());
            task.approval_request_id = previous_task
                .approval_request_id
                .clone()
                .or(task.approval_request_id.clone());
        }
    }
    // `thread/list` is not guaranteed to return every live thread while the
    // app-server is busy. Keep recent event-driven tasks that are absent from
    // that response, otherwise the compact island under-counts the same
    // snapshot that the details list just received.
    for previous_task in previous {
        if polled.iter().any(|task| task.id == previous_task.id) {
            continue;
        }
        let age = now.saturating_sub(previous_task.updated_at);
        let keep = match previous_task.status {
            TaskStatus::NeedsAction | TaskStatus::Running => age <= STALE_AFTER_SECONDS,
            TaskStatus::Completed => age <= COMPLETED_VISIBLE_SECONDS,
            TaskStatus::None => false,
        };
        if keep {
            polled.push(previous_task.clone());
        }
    }
    polled
}

fn merge_rollout_tasks(
    mut polled: Vec<TaskSummary>,
    rollout: Vec<TaskSummary>,
) -> Vec<TaskSummary> {
    for rollout_task in rollout {
        if let Some(task) = polled.iter_mut().find(|task| task.id == rollout_task.id) {
            task.activity = rollout_task.activity.clone().or(task.activity.clone());
            task.token_count = rollout_task.token_count.or(task.token_count);
            task.updated_at = task.updated_at.max(rollout_task.updated_at);
        }
    }
    polled
}

pub fn spawn_poll_loop(
    store: SnapshotStore,
    client: Arc<CodexClient>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let intervals = SyncIntervals::default();
        let mut previous = store.current();
        let mut notification_rx = client.subscribe_notifications();
        let mut metrics_tick = tokio::time::interval(intervals.metrics);
        let mut full_tick = tokio::time::interval(intervals.full);
        metrics_tick.tick().await;
        full_tick.tick().await;
        loop {
            tokio::select! {
                notification = notification_rx.recv() => {
                    if let Ok(notification) = notification {
                        let now = chrono_like_now();
                        let method = notification.get("method").and_then(serde_json::Value::as_str).unwrap_or("");
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
                                next.five_hour_remaining_percent = rate.five_hour_remaining_percent;
                                next.five_hour_resets_at = rate.five_hour_resets_at;
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
                        next.five_hour_remaining_percent = rate.five_hour_remaining_percent;
                        next.five_hour_resets_at = rate.five_hour_resets_at;
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

/// Watches the same local session records used by Codex Desktop and the HUD
/// reference implementation. This loop is intentionally independent from the
/// floating window's private app-server child process, so a failed or missing
/// app-server cannot make task state disappear.
pub fn spawn_local_task_loop(store: SnapshotStore) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        let mut watcher = RolloutWatcher::new(default_root());
        let mut tick = tokio::time::interval(SyncIntervals::default().task);
        tick.tick().await;
        loop {
            tick.tick().await;
            let now = chrono_like_now();
            let previous = store.current();
            let mut registry = TaskRegistry::from_tasks(&previous.tasks);
            registry.replace_source_tasks("local-session", watcher.scan(now), now);
            let mut next = previous.clone();
            next.tasks = registry.tasks();
            next.task_counts = registry.counts();
            next.active_task_count = next.task_counts.needs_action + next.task_counts.running;
            next.changed_at = Some(now);
            next.source = Some("local-session".into());
            store.publish_if_changed(next);
        }
    })
}

pub fn refresh_local_tasks(store: &SnapshotStore, previous: &Snapshot, now: i64) -> Snapshot {
    let mut watcher = RolloutWatcher::new(default_root());
    let mut registry = TaskRegistry::from_tasks(&previous.tasks);
    registry.replace_source_tasks("local-session", watcher.scan(now), now);
    let mut next = previous.clone();
    next.tasks = registry.tasks();
    next.task_counts = registry.counts();
    next.active_task_count = next.task_counts.needs_action + next.task_counts.running;
    next.changed_at = Some(now);
    next.source = Some("manual-refresh".into());
    store.publish_if_changed(next.clone());
    next
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
        waiting_reason: event.waiting_reason.clone(),
        approval_request_id: event.approval_request_id.clone(),
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
        let keep_approval = task.status == TaskStatus::NeedsAction
            && matches!(status, TaskStatus::None | TaskStatus::Running)
            && task_event.approval_request_id.is_none()
            && !task_event.waiting_for_user
            && !task_event.completed;
        if !keep_approval {
            task.status = status;
        }
        if status == TaskStatus::NeedsAction {
            task.waiting_reason = task_event
                .waiting_reason
                .clone()
                .or(task.waiting_reason.clone());
            task.approval_request_id = task_event
                .approval_request_id
                .clone()
                .or(task.approval_request_id.clone());
        } else if !keep_approval {
            task.waiting_reason = None;
            task.approval_request_id = None;
        }
        task.token_count = task_event.token_count.or(task.token_count);
        task.updated_at = task_event.updated_at.max(task.updated_at);
    } else {
        snapshot.tasks.push(TaskSummary {
            id: task_event.id,
            title: task_event.title,
            activity: None,
            waiting_reason: task_event.waiting_reason,
            approval_request_id: task_event.approval_request_id,
            status,
            token_count: task_event.token_count,
            updated_at: task_event.updated_at,
            acknowledged: false,
            source: Some("app-server-event".into()),
            turn_id: event.turn_id,
            received_at: now,
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
    fn rollout_text_recovers_approval_for_an_active_thread() {
        let path = std::env::temp_dir().join(format!(
            "codex-quota-approval-{}-{}.jsonl",
            std::process::id(),
            chrono_like_now()
        ));
        std::fs::write(
            &path,
            r#"{"type":"event_msg","payload":{"type":"approval_required","reason":"需要确认"}}"#,
        )
        .unwrap();
        let task = map_threads(
            vec![ThreadSummary {
                id: "thread-approval".into(),
                title: "任务".into(),
                status: "active".into(),
                updated_at: chrono_like_now(),
                path: Some(path.to_string_lossy().into_owned()),
            }],
            chrono_like_now(),
        );
        let _ = std::fs::remove_file(path);
        assert_eq!(task[0].status, TaskStatus::NeedsAction);
    }

    #[test]
    fn rollout_activity_skips_injected_context_and_uses_actual_request() {
        let input = r###"
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<recommended_plugins>\n- Cloudflare\n</recommended_plugins>"},{"type":"input_text","text":"# AGENTS.md instructions\n<INSTRUCTIONS>...</INSTRUCTIONS>"},{"type":"input_text","text":"<environment_context>...</environment_context>"},{"type":"input_text","text":"\n# Files mentioned by the user:\n\n## My request:\n请修复任务标题识别问题，并重新打包。"}]}}
"###;
        assert_eq!(
            extract_rollout_user_activity(input).as_deref(),
            Some("请修复任务标题识别问题，并重新打包")
        );
    }

    #[test]
    fn rollout_activity_uses_the_latest_user_request() {
        let input = r#"
{"type":"response_item","payload":{"type":"message","role":"user","content":"先检查项目"}}
{"type":"response_item","payload":{"type":"message","role":"user","content":"现在修复任务显示"}}
"#;
        assert_eq!(
            extract_rollout_user_activity(input).as_deref(),
            Some("现在修复任务显示")
        );
    }

    #[test]
    fn thread_title_does_not_expose_injected_plugin_context() {
        let threads = map_threads(
            vec![ThreadSummary {
                id: "thread-plugin-context".into(),
                title: "<recommended_plugins>\n- Cloudflare\n</recommended_plugins>".into(),
                status: "active".into(),
                updated_at: chrono_like_now(),
                path: None,
            }],
            chrono_like_now(),
        );
        assert_eq!(threads[0].title, "Codex 对话");

        let threads = map_threads(
            vec![ThreadSummary {
                id: "thread-delegation-context".into(),
                title: "<codex_delegation>\n任务上下文\n</codex_delegation>".into(),
                status: "active".into(),
                updated_at: chrono_like_now(),
                path: None,
            }],
            chrono_like_now(),
        );
        assert_eq!(threads[0].title, "Codex 对话");
    }

    #[test]
    fn recent_idle_thread_is_metadata_only_until_a_stop_event() {
        let now = chrono_like_now();
        let tasks = map_threads(
            vec![ThreadSummary {
                id: "thread-completed".into(),
                title: "已完成任务".into(),
                status: "idle".into(),
                updated_at: now,
                path: None,
            }],
            now,
        );
        assert!(tasks.is_empty());
    }

    #[test]
    fn not_loaded_thread_is_not_falsely_marked_completed() {
        let now = chrono_like_now();
        let tasks = map_threads(
            vec![ThreadSummary {
                id: "thread-not-loaded".into(),
                title: "未加载任务".into(),
                status: "notLoaded".into(),
                updated_at: now,
                path: None,
            }],
            now,
        );
        assert!(tasks.is_empty());
    }

    #[test]
    fn approval_event_marks_the_thread_red_with_reason_and_request_id() {
        let mut snapshot = crate::snapshot_store::empty_snapshot();
        apply_task_event(
            &mut snapshot,
            NormalizedTaskEvent {
                id: "thread-approval".into(),
                turn_id: Some("turn-approval".into()),
                item_id: Some("item-approval".into()),
                request_id: Some("41".into()),
                resolved_request_id: None,
                title: "Codex 任务".into(),
                waiting_reason: Some("需要批准命令".into()),
                approval_request_id: Some("41".into()),
                waiting_for_user: true,
                running: false,
                completed: false,
                token_count: None,
                updated_at: 100,
            },
            100,
        );
        assert_eq!(snapshot.tasks[0].status, TaskStatus::NeedsAction);
        assert_eq!(
            snapshot.tasks[0].waiting_reason.as_deref(),
            Some("需要批准命令")
        );
        assert_eq!(snapshot.tasks[0].approval_request_id.as_deref(), Some("41"));
    }

    #[test]
    fn keeps_approval_state_when_thread_list_still_says_active() {
        let previous = vec![TaskSummary {
            id: "thread-1".into(),
            title: "需要确认".into(),
            activity: None,
            waiting_reason: Some("需要批准".into()),
            approval_request_id: Some("req-1".into()),
            status: TaskStatus::NeedsAction,
            token_count: None,
            updated_at: 100,
            acknowledged: false,
            source: None,
            turn_id: None,
            received_at: 100,
        }];
        let polled = vec![TaskSummary {
            id: "thread-1".into(),
            title: "需要确认".into(),
            activity: None,
            waiting_reason: None,
            approval_request_id: None,
            status: TaskStatus::Running,
            token_count: None,
            updated_at: 101,
            acknowledged: false,
            source: None,
            turn_id: None,
            received_at: 101,
        }];
        let merged = merge_polled_tasks(&previous, polled, 101);
        assert_eq!(merged[0].status, TaskStatus::NeedsAction);
    }

    #[test]
    fn retains_recent_event_tasks_missing_from_thread_list() {
        let previous = vec![
            TaskSummary {
                id: "thread-1".into(),
                title: "任务一".into(),
                activity: None,
                waiting_reason: None,
                approval_request_id: None,
                status: TaskStatus::Running,
                token_count: None,
                updated_at: 100,
                acknowledged: false,
                source: None,
                turn_id: None,
                received_at: 100,
            },
            TaskSummary {
                id: "thread-2".into(),
                title: "任务二".into(),
                activity: None,
                waiting_reason: None,
                approval_request_id: None,
                status: TaskStatus::Running,
                token_count: None,
                updated_at: 100,
                acknowledged: false,
                source: None,
                turn_id: None,
                received_at: 100,
            },
        ];
        let polled = vec![TaskSummary {
            id: "thread-1".into(),
            title: "任务一".into(),
            activity: None,
            waiting_reason: None,
            approval_request_id: None,
            status: TaskStatus::Running,
            token_count: None,
            updated_at: 101,
            acknowledged: false,
            source: None,
            turn_id: None,
            received_at: 101,
        }];
        let merged = merge_polled_tasks(&previous, polled, 101);
        assert_eq!(merged.len(), 2);
        assert!(merged.iter().any(|task| task.id == "thread-2"));
    }

    #[test]
    fn rollout_tasks_are_merged_into_the_same_compact_source() {
        let polled = vec![TaskSummary {
            id: "thread-1".into(),
            title: "旧标题".into(),
            activity: None,
            waiting_reason: None,
            approval_request_id: None,
            status: TaskStatus::Running,
            token_count: None,
            updated_at: 100,
            acknowledged: false,
            source: None,
            turn_id: None,
            received_at: 100,
        }];
        let rollout = vec![
            TaskSummary {
                id: "thread-1".into(),
                title: "Codex 对话".into(),
                activity: Some("用户第一句话".into()),
                waiting_reason: Some("等待确认".into()),
                approval_request_id: None,
                status: TaskStatus::NeedsAction,
                token_count: Some(42),
                updated_at: 101,
                acknowledged: false,
                source: Some("history".into()),
                turn_id: None,
                received_at: 101,
            },
            TaskSummary {
                id: "thread-2".into(),
                title: "并行任务".into(),
                activity: Some("并行任务内容".into()),
                waiting_reason: None,
                approval_request_id: None,
                status: TaskStatus::Running,
                token_count: Some(7),
                updated_at: 102,
                acknowledged: false,
                source: Some("history".into()),
                turn_id: None,
                received_at: 102,
            },
        ];
        let merged = merge_rollout_tasks(polled, rollout);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].status, TaskStatus::Running);
        assert_eq!(merged[0].title, "旧标题");
        assert_eq!(merged[0].activity.as_deref(), Some("用户第一句话"));
    }

    #[test]
    fn keeps_approval_state_when_a_running_event_arrives_before_decision() {
        let mut snapshot = crate::snapshot_store::empty_snapshot();
        apply_task_event(
            &mut snapshot,
            NormalizedTaskEvent {
                id: "thread-approval".into(),
                turn_id: Some("turn-approval".into()),
                item_id: Some("item-approval".into()),
                request_id: Some("41".into()),
                resolved_request_id: None,
                title: "需要确认".into(),
                waiting_reason: Some("需要批准命令".into()),
                approval_request_id: Some("41".into()),
                waiting_for_user: true,
                running: false,
                completed: false,
                token_count: None,
                updated_at: 100,
            },
            100,
        );
        apply_task_event(
            &mut snapshot,
            NormalizedTaskEvent {
                id: "thread-approval".into(),
                turn_id: Some("turn-approval".into()),
                item_id: Some("item-approval".into()),
                request_id: None,
                resolved_request_id: None,
                title: "需要确认".into(),
                waiting_reason: None,
                approval_request_id: None,
                waiting_for_user: false,
                running: true,
                completed: false,
                token_count: None,
                updated_at: 101,
            },
            101,
        );
        assert_eq!(snapshot.tasks[0].status, TaskStatus::NeedsAction);
        assert_eq!(snapshot.tasks[0].approval_request_id.as_deref(), Some("41"));
    }
}
