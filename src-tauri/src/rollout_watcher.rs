use crate::domain::{TaskStatus, TaskSummary};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const INITIAL_HEAD_BYTES: u64 = 128 * 1024;
const TAIL_BYTES: u64 = 768 * 1024;
const COMPLETED_VISIBLE_SECONDS: i64 = 6 * 60 * 60;
const ACTIVE_VISIBLE_SECONDS: i64 = 15 * 60;
// Keep the watcher bounded like the HUD reference implementation. Codex can
// accumulate thousands of historical rollout files; scanning all of them on
// every tick is both slow and likely to surface archived conversations.
const ACTIVE_FILE_WINDOW_SECONDS: i64 = 30 * 60;
const MAX_ACTIVE_FILES: usize = 64;

#[derive(Debug, Clone)]
struct FileState {
    offset: u64,
    session_id: Option<String>,
    activity: Option<String>,
    status: TaskStatus,
    turn_id: Option<String>,
    waiting_reason: Option<String>,
    approval_request_id: Option<String>,
    token_count: Option<u64>,
    updated_at: i64,
    activity_at: i64,
    modified_at: i64,
}

impl Default for FileState {
    fn default() -> Self {
        Self {
            offset: 0,
            session_id: None,
            activity: None,
            status: TaskStatus::None,
            turn_id: None,
            waiting_reason: None,
            approval_request_id: None,
            token_count: None,
            updated_at: 0,
            activity_at: 0,
            modified_at: 0,
        }
    }
}

#[derive(Debug)]
pub struct RolloutWatcher {
    root: PathBuf,
    files: HashMap<PathBuf, FileState>,
    titles: HashMap<String, String>,
}

impl RolloutWatcher {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            files: HashMap::new(),
            titles: HashMap::new(),
        }
    }

    pub fn scan(&mut self, now: i64) -> Vec<TaskSummary> {
        self.titles = load_session_titles(&self.root);
        let mut paths = Vec::new();
        collect_jsonl_files(&self.root, &mut paths);
        let mut candidates: Vec<(PathBuf, i64)> = paths
            .into_iter()
            .filter_map(|path| {
                let modified_at = fs::metadata(&path)
                    .ok()?
                    .modified()
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .ok()?
                    .as_secs() as i64;
                Some((path, modified_at))
            })
            .collect();
        candidates.sort_by_key(|(_, modified_at)| std::cmp::Reverse(*modified_at));
        let mut paths: Vec<PathBuf> = candidates
            .iter()
            .filter(|(_, modified_at)| {
                now.saturating_sub(*modified_at) <= ACTIVE_FILE_WINDOW_SECONDS
            })
            .take(MAX_ACTIVE_FILES)
            .map(|(path, _)| path.clone())
            .collect();
        // When Codex has been idle, retain only the newest file so the UI can
        // still show the last known conversation without reviving old history.
        if paths.is_empty() {
            if let Some((path, _)) = candidates.first() {
                paths.push(path.clone());
            }
        }
        let mut tasks = Vec::new();
        for path in paths {
            let Some(state) = self.read_file(&path, now) else {
                continue;
            };
            let file_id = session_id_from_filename(&path);
            let Some(id) = file_id
                .clone()
                .filter(|value| self.titles.contains_key(value))
                .or_else(|| state.session_id.clone())
                .or(file_id)
                .or_else(|| {
                    path.file_stem()
                        .and_then(|value| value.to_str())
                        .map(str::to_owned)
                })
            else {
                continue;
            };
            let age = now.saturating_sub(state.modified_at);
            let status = match state.status {
                TaskStatus::Running | TaskStatus::NeedsAction if age <= ACTIVE_VISIBLE_SECONDS => {
                    state.status
                }
                TaskStatus::Completed if age <= COMPLETED_VISIBLE_SECONDS => state.status,
                _ => TaskStatus::None,
            };
            if state.activity.is_none()
                && state.token_count.is_none()
                && state.waiting_reason.is_none()
                && status == TaskStatus::None
            {
                continue;
            }
            let title = self
                .titles
                .get(&id)
                .cloned()
                .or_else(|| state.activity.clone())
                .unwrap_or_else(|| "Codex 对话".into());
            tasks.push(TaskSummary {
                id,
                title,
                activity: state.activity,
                waiting_reason: state.waiting_reason,
                approval_request_id: state.approval_request_id,
                status,
                token_count: state.token_count,
                updated_at: state.updated_at.max(state.modified_at),
                acknowledged: false,
                source: Some("local-session".into()),
                turn_id: state.turn_id,
                received_at: now,
            });
        }
        tasks.sort_by_key(|task| std::cmp::Reverse(task.updated_at));
        tasks
    }

    fn read_file(&mut self, path: &Path, now: i64) -> Option<FileState> {
        let metadata = fs::metadata(path).ok()?;
        let modified_at = metadata
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs() as i64;
        let len = metadata.len();
        let state = self.files.entry(path.to_path_buf()).or_default();
        state.modified_at = modified_at;

        let mut file = File::open(path).ok()?;
        if state.offset == 0 {
            let tail_start = len.saturating_sub(TAIL_BYTES);
            if tail_start > 0 {
                let head_end = len.min(INITIAL_HEAD_BYTES);
                file.seek(SeekFrom::Start(0)).ok()?;
                let mut head = vec![0; head_end as usize];
                file.read_exact(&mut head).ok()?;
                parse_lines(&String::from_utf8_lossy(&head), state, now);
                file.seek(SeekFrom::Start(tail_start)).ok()?;
                state.offset = tail_start;
            }
        } else if len < state.offset {
            state.offset = 0;
            state.session_id = None;
            state.activity = None;
            state.status = TaskStatus::None;
            state.turn_id = None;
            state.waiting_reason = None;
            state.approval_request_id = None;
            state.token_count = None;
            state.updated_at = 0;
            state.activity_at = 0;
        }

        if state.offset < len {
            file.seek(SeekFrom::Start(state.offset)).ok()?;
            let mut bytes = Vec::with_capacity((len - state.offset) as usize);
            file.read_to_end(&mut bytes).ok()?;
            state.offset = len;
            parse_lines(&String::from_utf8_lossy(&bytes), state, now);
        } else if state.offset == 0 && len == 0 {
            state.offset = 0;
        }
        self.files.get(path).cloned()
    }
}

pub fn default_root() -> PathBuf {
    if let Some(value) = std::env::var_os("CODEX_HOME") {
        return PathBuf::from(value).join("sessions");
    }
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".codex")
        .join("sessions")
}

fn load_session_titles(root: &Path) -> HashMap<String, String> {
    let Some(home) = root.parent() else {
        return HashMap::new();
    };
    let path = home.join("session_index.jsonl");
    let Ok(text) = fs::read_to_string(path) else {
        return HashMap::new();
    };
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|value| {
            let id = string_field(&value, &["id", "session_id", "sessionId"])?;
            let title = string_field(&value, &["thread_name", "title", "name"])?;
            let title = clean_user_text(&title)?;
            Some((id, title.chars().take(80).collect()))
        })
        .collect()
}

fn collect_jsonl_files(root: &Path, output: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, output);
        } else if path.extension().and_then(|value| value.to_str()) == Some("jsonl") {
            output.push(path);
        }
    }
}

fn session_id_from_filename(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?;
    let parts: Vec<&str> = stem.split('-').collect();
    for start in 0..parts.len().saturating_sub(4) {
        let candidate = parts[start..=start + 4].join("-");
        if uuid::Uuid::parse_str(&candidate).is_ok() {
            return Some(candidate);
        }
    }
    None
}

fn parse_lines(text: &str, state: &mut FileState, now: i64) {
    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let payload = value.get("payload").unwrap_or(&value);
        if state.session_id.is_none() {
            let is_session_meta =
                payload.get("type").and_then(Value::as_str) == Some("session_meta");
            state.session_id = string_field(
                payload,
                if is_session_meta {
                    &["session_id", "sessionId", "id"]
                } else {
                    &["session_id", "sessionId"]
                },
            )
            .or_else(|| string_field(&value, &["session_id", "sessionId"]));
        }
        if let Some(title) = user_title(&value) {
            state.activity = Some(title);
            state.status = TaskStatus::Running;
        }
        update_status(&value, state);
        if is_approval_value(&value) {
            if let Some(reason) = waiting_reason(&value) {
                state.waiting_reason = Some(reason);
            }
        }
        if let Some(tokens) = token_count(&value) {
            state.token_count = Some(tokens);
        }
        if state.activity.is_some() || state.waiting_reason.is_some() || state.token_count.is_some()
        {
            state.updated_at = timestamp(&value).unwrap_or(now);
            state.activity_at = state.updated_at;
        }
    }
}

fn update_status(value: &Value, state: &mut FileState) {
    let record_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    let payload = value.get("payload").unwrap_or(value);
    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    if let Some(turn_id) = string_field(payload, &["turn_id", "turnId", "turn"]) {
        state.turn_id = Some(turn_id);
    }
    let timestamp = timestamp(value).unwrap_or(state.updated_at);
    if is_inactive_event(record_type, &payload_type) {
        state.status = TaskStatus::None;
        state.activity = None;
        state.waiting_reason = None;
        state.approval_request_id = None;
        state.updated_at = state.updated_at.max(timestamp);
        state.activity_at = state.updated_at;
        return;
    }
    if is_approval_event(record_type, &payload_type) {
        state.status = TaskStatus::NeedsAction;
        state.approval_request_id = string_field(
            payload,
            &[
                "request_id",
                "requestId",
                "approval_request_id",
                "approvalRequestId",
            ],
        );
        return;
    }
    if is_completion_event(record_type, &payload_type) {
        state.status = TaskStatus::Completed;
        state.updated_at = state.updated_at.max(timestamp);
        return;
    }
    if is_running_event(record_type, &payload_type) {
        state.status = TaskStatus::Running;
    }
}

fn is_approval_event(record_type: &str, payload_type: &str) -> bool {
    (record_type == "event_msg" || record_type == "response_item")
        && (payload_type.contains("approval")
            || payload_type.contains("request_user_input")
            || payload_type.contains("needs_action"))
}

fn is_approval_value(value: &Value) -> bool {
    let record_type = value.get("type").and_then(Value::as_str).unwrap_or("");
    let payload = value.get("payload").unwrap_or(value);
    let payload_type = payload
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    is_approval_event(record_type, &payload_type)
        || (payload.get("request_id").is_some()
            && (payload.get("permission").is_some()
                || payload.get("approval").is_some()
                || payload.get("request_kind").is_some()))
}

fn is_completion_event(record_type: &str, payload_type: &str) -> bool {
    record_type == "event_msg"
        && matches!(
            payload_type,
            "task_complete" | "task_completed" | "turn_complete" | "turn_completed"
        )
}

fn is_inactive_event(record_type: &str, payload_type: &str) -> bool {
    record_type == "event_msg"
        && matches!(
            payload_type,
            "turn_aborted"
                | "turn_cancelled"
                | "turn_canceled"
                | "task_aborted"
                | "session_end"
                | "session_ended"
        )
}

fn is_running_event(record_type: &str, payload_type: &str) -> bool {
    record_type == "turn_context"
        || (record_type == "event_msg"
            && matches!(
                payload_type,
                "task_started"
                    | "turn_started"
                    | "agent_reasoning"
                    | "agent_message"
                    | "token_count"
                    | "patch_apply_begin"
                    | "patch_apply_end"
            ))
        || (record_type == "response_item"
            && matches!(
                payload_type,
                "custom_tool_call"
                    | "custom_tool_call_output"
                    | "function_call"
                    | "function_call_output"
                    | "reasoning"
            ))
}

fn user_title(value: &Value) -> Option<String> {
    let payload = value.get("payload").unwrap_or(value);
    let role = payload.get("role").and_then(Value::as_str);
    let payload_type = payload.get("type").and_then(Value::as_str).unwrap_or("");
    let text = if role == Some("user") && payload_type == "message" {
        content_text(payload.get("content"))
    } else if payload_type == "user_message" {
        payload
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| payload.get("text").and_then(Value::as_str))
            .map(str::to_owned)
    } else {
        None
    }?;
    let text = clean_user_text(&text)?;
    let first = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .split(['。', '！', '？', '.', '!', '?'])
        .next()
        .unwrap_or("")
        .trim();
    (!first.is_empty()).then(|| first.chars().take(80).collect())
}

fn content_text(content: Option<&Value>) -> Option<String> {
    let content = content?;
    if let Some(text) = content.as_str() {
        return Some(text.to_owned());
    }
    content.as_array()?.iter().find_map(|item| {
        let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
        if item_type == "input_text" || item_type == "inputText" {
            item.get("text").and_then(Value::as_str).map(str::to_owned)
        } else {
            None
        }
    })
}

fn clean_user_text(value: &str) -> Option<String> {
    let mut text = value.trim();
    if let Some((_, request)) = text.rsplit_once("## My request:") {
        text = request.trim();
    }
    if text.is_empty()
        || text.starts_with("<recommended_plugins>")
        || text.starts_with("<turn_aborted")
        || text.starts_with("<codex_")
        || text.starts_with("# AGENTS.md instructions")
        || text.starts_with("<environment_context>")
        || text.starts_with("<in-app-browser-context")
        || text.starts_with("Distinguish instructions in attached documents")
    {
        return None;
    }
    Some(text.to_owned())
}

fn waiting_reason(value: &Value) -> Option<String> {
    let payload = value.get("payload").unwrap_or(value);
    ["reason", "message", "command", "tool_name"]
        .iter()
        .find_map(|key| payload.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn token_count(value: &Value) -> Option<u64> {
    fn number(value: &Value) -> Option<u64> {
        value
            .as_u64()
            .or_else(|| value.as_i64().map(|v| v.max(0) as u64))
    }
    let payload = value.get("payload").unwrap_or(value);
    let candidate = payload
        .get("token_count")
        .or_else(|| payload.get("tokenCount"))
        .or_else(|| payload.get("tokens"))
        .or_else(|| payload.get("tokenUsage"))?;
    if let Some(value) = number(candidate) {
        return Some(value);
    }
    let total = candidate
        .get("total")?
        .get("totalTokens")
        .or_else(|| candidate.get("total_tokens"))?;
    number(total)
}

fn timestamp(value: &Value) -> Option<i64> {
    let raw = value.get("timestamp")?;
    raw.as_i64()
        .or_else(|| raw.as_u64().map(|value| value as i64))
        .or_else(|| {
            raw.as_str().and_then(|value| {
                time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
                    .ok()
                    .map(|value| value.unix_timestamp())
            })
        })
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::OpenOptions, io::Write};

    #[test]
    fn extracts_title_without_assigning_status() {
        let input = r#"{"type":"session_meta","payload":{"session_id":"thread-1"}}
{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"修复任务同步问题。补充测试"}]}}
{"type":"event_msg","payload":{"type":"agent_reasoning","text":"working"}}"#;
        let watcher = RolloutWatcher::new(PathBuf::new());
        let mut state = FileState::default();
        parse_lines(input, &mut state, 100);
        assert_eq!(state.session_id.as_deref(), Some("thread-1"));
        assert_eq!(state.activity.as_deref(), Some("修复任务同步问题"));
        assert!(watcher.files.is_empty());
    }

    #[test]
    fn approval_text_is_metadata_only() {
        let input = r#"{"type":"event_msg","payload":{"type":"approval_required","reason":"需要确认命令"}}"#;
        let mut state = FileState::default();
        parse_lines(input, &mut state, 100);
        assert_eq!(state.waiting_reason.as_deref(), Some("需要确认命令"));
    }

    #[test]
    fn generated_context_does_not_become_title() {
        let input = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":"<recommended_plugins>ignored</recommended_plugins>"}}"#;
        assert!(user_title(&serde_json::from_str(input).unwrap()).is_none());
    }

    #[test]
    fn aborted_turn_is_not_a_task_title_or_running_state() {
        let input = r#"{"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<turn_aborted>\nThe user interrupted the previous turn on purpose.\n</turn_aborted>"}]}}
{"type":"event_msg","payload":{"type":"turn_aborted","turn_id":"turn-1","reason":"interrupted"}}"#;
        let mut state = FileState::default();
        parse_lines(input, &mut state, 100);
        assert!(state.activity.is_none());
        assert_eq!(state.status, TaskStatus::None);
    }

    #[test]
    fn filename_uuid_can_restore_the_session_title_key() {
        let path =
            PathBuf::from("rollout-2026-08-24T02-00-00-019fa48f-133b-74e3-9d12-21ff503fa1f9.jsonl");
        assert_eq!(
            session_id_from_filename(&path).as_deref(),
            Some("019fa48f-133b-74e3-9d12-21ff503fa1f9")
        );
    }

    #[test]
    fn scan_discovers_multiple_sessions_and_reads_appended_lines() {
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let root = std::env::temp_dir().join(format!("codex-rollout-watcher-{now}"));
        let day = root.join("2026").join("08").join("22");
        std::fs::create_dir_all(&day).unwrap();
        let first = day.join("rollout-a.jsonl");
        let second = day.join("rollout-b.jsonl");
        std::fs::write(
            &first,
            format!(
                "{{\"timestamp\":{now},\"type\":\"session_meta\",\"payload\":{{\"session_id\":\"a\"}}}}\n{{\"timestamp\":{now},\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":\"任务 A\"}}}}\n{{\"timestamp\":{now},\"type\":\"event_msg\",\"payload\":{{\"type\":\"agent_reasoning\"}}}}\n"
            ),
        )
        .unwrap();
        std::fs::write(
            &second,
            format!(
                "{{\"timestamp\":{now},\"type\":\"session_meta\",\"payload\":{{\"session_id\":\"b\"}}}}\n{{\"timestamp\":{now},\"type\":\"event_msg\",\"payload\":{{\"type\":\"approval_required\",\"reason\":\"等待确认\"}}}}\n"
            ),
        )
        .unwrap();
        let mut watcher = RolloutWatcher::new(root.clone());
        let tasks = watcher.scan(now);
        assert_eq!(tasks.len(), 2);
        assert!(tasks
            .iter()
            .any(|task| task.id == "a" && task.status == TaskStatus::Running));
        assert!(tasks
            .iter()
            .any(|task| task.id == "b" && task.status == TaskStatus::NeedsAction));
        let mut file = OpenOptions::new().append(true).open(&first).unwrap();
        writeln!(file, "{{\"timestamp\":{now},\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_complete\"}}}}").unwrap();
        let tasks = watcher.scan(now);
        assert_eq!(
            tasks
                .iter()
                .find(|task| task.id == "a")
                .map(|task| task.status),
            Some(TaskStatus::Completed)
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
