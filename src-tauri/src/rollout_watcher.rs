use crate::domain::{TaskStatus, TaskSummary};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const INITIAL_HEAD_BYTES: u64 = 128 * 1024;
const TAIL_BYTES: u64 = 768 * 1024;
const COMPLETED_VISIBLE_SECONDS: i64 = 6 * 60 * 60;
const ACTIVE_VISIBLE_SECONDS: i64 = 15 * 60;
const DISCOVERY_INTERVAL_SECONDS: i64 = 5;
const READ_FAILURE_GRACE_SECONDS: i64 = 5;

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
    last_read_at: i64,
    waiting_call_id: Option<String>,
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
            last_read_at: 0,
            waiting_call_id: None,
        }
    }
}

#[derive(Debug)]
pub struct RolloutWatcher {
    root: PathBuf,
    files: HashMap<PathBuf, FileState>,
    titles: HashMap<String, String>,
    paths: Vec<PathBuf>,
    next_discovery: i64,
}

impl RolloutWatcher {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            files: HashMap::new(),
            titles: HashMap::new(),
            paths: Vec::new(),
            next_discovery: 0,
        }
    }

    pub fn scan(&mut self, now: i64) -> Vec<TaskSummary> {
        if now >= self.next_discovery {
            self.titles = load_session_titles(&self.root);
            let mut paths = Vec::new();
            collect_jsonl_files(&self.root, &mut paths);
            // A failed directory read is not evidence that all tasks ended.
            if !paths.is_empty() {
                self.paths = paths;
                let known_paths: HashSet<_> = self.paths.iter().collect();
                self.files.retain(|path, _| known_paths.contains(path));
            }
            self.next_discovery = now + DISCOVERY_INTERVAL_SECONDS;
        }
        let mut tasks = Vec::new();
        // Windows may keep mtime unchanged while an open file grows. Inspect
        // length for every discovered file; only parse the appended bytes.
        for path in self.paths.clone() {
            let Some(state) = self.read_file(&path, now).or_else(|| {
                self.files
                    .get(&path)
                    .filter(|state| {
                        now.saturating_sub(state.last_read_at) <= READ_FAILURE_GRACE_SECONDS
                    })
                    .cloned()
            }) else {
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
            let age = now.saturating_sub(state.updated_at);
            if state.updated_at == 0 || age > COMPLETED_VISIBLE_SECONDS {
                continue;
            }
            let status = match state.status {
                TaskStatus::Running if age <= ACTIVE_VISIBLE_SECONDS => state.status,
                TaskStatus::NeedsAction => state.status,
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
                // A rollout request belongs to the owning Codex process and
                // cannot be approved through our separate app-server channel.
                approval_request_id: None,
                status,
                token_count: state.token_count,
                updated_at: state.updated_at,
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
        if state.offset > 0 && state.offset == len {
            state.last_read_at = now;
            return Some(state.clone());
        }

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
            state.waiting_call_id = None;
        }

        if state.offset < len {
            if len - state.offset > TAIL_BYTES {
                state.offset = len.saturating_sub(TAIL_BYTES);
            }
            file.seek(SeekFrom::Start(state.offset)).ok()?;
            let mut bytes = Vec::with_capacity((len - state.offset) as usize);
            file.take(len - state.offset).read_to_end(&mut bytes).ok()?;
            // Never discard a partly written JSON record. Reread its bytes
            // when Codex appends the terminating newline on a later tick.
            if let Some(end) = bytes.iter().rposition(|byte| *byte == b'\n') {
                state.offset += (end + 1) as u64;
                parse_lines(&String::from_utf8_lossy(&bytes[..=end]), state, now);
            }
        } else if state.offset == 0 && len == 0 {
            state.offset = 0;
        }
        state.last_read_at = now;
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
        let previous_status = state.status;
        let previous_activity_at = state.activity_at;
        let has_request = user_title(&value);
        if let Some(title) = has_request.clone() {
            state.activity = Some(title);
            state.status = TaskStatus::Running;
            state.waiting_reason = None;
            state.approval_request_id = None;
            state.waiting_call_id = None;
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
        let record_type = value.get("type").and_then(Value::as_str).unwrap_or("");
        let payload_type = payload.get("type").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
        // Usage and metadata are not lifecycle evidence. Activity must work
        // even when the bounded read skipped the original user request.
        let new_start = matches!(payload_type.as_str(), "task_started" | "turn_started") || record_type == "turn_context";
        let valid_activity = is_running_event(record_type, &payload_type)
            && (new_start || has_request.is_some()
                || !(previous_status == TaskStatus::Completed || (previous_status == TaskStatus::None && previous_activity_at > 0)));
        if has_request.is_some() || valid_activity
            || is_completion_event(record_type, &payload_type)
            || is_inactive_event(record_type, &payload_type)
            || is_approval_value(&value) {
            state.updated_at = state
                .updated_at
                .max(timestamp(&value).unwrap_or(state.modified_at.min(now)));
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
    let turn_id = string_field(payload, &["turn_id", "turnId", "turn"]);
    if (is_completion_event(record_type, &payload_type)
        || is_inactive_event(record_type, &payload_type))
        && turn_id.is_some()
        && state.turn_id.is_some()
        && turn_id != state.turn_id
    {
        return;
    }
    state.turn_id = turn_id.or(state.turn_id.clone());
    let timestamp = timestamp(value).unwrap_or(state.updated_at);
    if is_inactive_event(record_type, &payload_type) {
        state.status = TaskStatus::None;
        state.activity = None;
        state.waiting_reason = None;
        state.approval_request_id = None;
        state.waiting_call_id = None;
        state.updated_at = state.updated_at.max(timestamp);
        state.activity_at = state.updated_at;
        return;
    }
    let tool_wait = record_type == "response_item"
        && matches!(payload_type.as_str(), "function_call" | "custom_tool_call")
        && payload
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|name| {
                !name.contains("request_user_input_async")
                    && (name.contains("request_user_input") || name.contains("requestUserInput"))
            });
    if is_approval_event(record_type, &payload_type) || tool_wait {
        state.status = TaskStatus::NeedsAction;
        state.waiting_call_id = string_field(payload, &["call_id", "callId"]);
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
        state.waiting_reason = None;
        state.approval_request_id = None;
        state.waiting_call_id = None;
        state.updated_at = state.updated_at.max(timestamp);
        return;
    }
    let resolves_wait = state.waiting_call_id.is_some()
        && string_field(payload, &["call_id", "callId"]) == state.waiting_call_id
        && matches!(
            payload_type.as_str(),
            "function_call_output" | "custom_tool_call_output"
        );
    if resolves_wait || is_running_event(record_type, &payload_type) {
        let explicit_start = matches!(payload_type.as_str(), "task_started" | "turn_started")
            || record_type == "turn_context";
        if (state.status == TaskStatus::Completed || (state.status == TaskStatus::None && state.activity_at > 0))
            && !explicit_start && !resolves_wait {
            return;
        }
        if state.status == TaskStatus::NeedsAction
            && !resolves_wait
            && !matches!(payload_type.as_str(), "task_started" | "turn_started")
        {
            return;
        }
        state.status = TaskStatus::Running;
        state.waiting_reason = None;
        state.approval_request_id = None;
        state.waiting_call_id = None;
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
    for pointer in ["/info/total_token_usage/total_tokens", "/thread_token_usage/total_tokens", "/token_count/total_tokens", "/tokenUsage/total/totalTokens"] {
        if let Some(total) = payload.pointer(pointer).and_then(number) { return Some(total); }
    }
    let candidate = payload
        .get("token_count")
        .or_else(|| payload.get("tokenCount"))
        .or_else(|| payload.get("tokens"))
        .or_else(|| payload.get("tokenUsage"))?;
    if let Some(value) = number(candidate) {
        return Some(value);
    }
    let total = candidate
        .get("total").and_then(|v| v.get("totalTokens"))
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
    fn bounded_tail_activity_survives_missing_request_and_registry_filter() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("large.jsonl");
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{{\"type\":\"session_meta\",\"payload\":{{\"session_id\":\"large\"}}}}").unwrap();
        let padding = format!("{{\"type\":\"padding\",\"payload\":\"{}\"}}\n", "x".repeat(4096));
        for _ in 0..40 { file.write_all(padding.as_bytes()).unwrap(); }
        writeln!(file, "{{\"timestamp\":1,\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_started\"}}}}").unwrap();
        writeln!(file, "{{\"timestamp\":1,\"type\":\"event_msg\",\"payload\":{{\"type\":\"user_message\",\"message\":\"hidden request\"}}}}").unwrap();
        for _ in 0..220 { file.write_all(padding.as_bytes()).unwrap(); }
        writeln!(file, "{{\"timestamp\":100000,\"type\":\"response_item\",\"payload\":{{\"type\":\"reasoning\"}}}}").unwrap();
        let mut watcher = RolloutWatcher::new(root.path().into());
        let tasks = watcher.scan(100000);
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].status, TaskStatus::Running);
        assert!(tasks[0].activity.is_none());
        assert!(tasks[0].token_count.is_none());
        let mut registry = crate::task_registry::TaskRegistry::default();
        registry.replace_source_tasks("local-session", tasks, 100000);
        assert_eq!(registry.tasks().len(), 1);
        let mut append = OpenOptions::new().append(true).open(&path).unwrap();
        let old = UNIX_EPOCH + std::time::Duration::from_secs(1);
        let line = "{\"timestamp\":101000,\"type\":\"response_item\",\"payload\":{\"type\":\"function_call\"}}\n";
        append.write_all(&line.as_bytes()[..30]).unwrap();
        append.set_modified(old).unwrap();
        assert!(watcher.scan(101000).iter().all(|task| task.status != TaskStatus::Running));
        append.write_all(&line.as_bytes()[30..]).unwrap();
        append.set_modified(old).unwrap();
        let tasks = watcher.scan(101001);
        assert_eq!(tasks[0].status, TaskStatus::Running);
        assert_eq!(tasks[0].updated_at, 101000);
    }

    #[test]
    fn usage_does_not_refresh_or_revive_terminal_lifecycle() {
        let mut state = FileState::default();
        parse_lines("{\"timestamp\":100,\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\"}}\n", &mut state, 100);
        parse_lines("{\"timestamp\":200,\"type\":\"token_usage_record\",\"payload\":{\"thread_token_usage\":{\"total_tokens\":42}}}\n", &mut state, 200);
        assert_eq!(state.status, TaskStatus::Completed);
        assert_eq!(state.updated_at, 100);
        parse_lines("{\"timestamp\":202,\"type\":\"response_item\",\"payload\":{\"type\":\"reasoning\"}}\n", &mut state, 202);
        assert_eq!(state.status, TaskStatus::Completed);
        assert_eq!(state.updated_at, 100);
        parse_lines("{\"timestamp\":203,\"type\":\"event_msg\",\"payload\":{\"type\":\"turn_started\",\"turn_id\":\"new\"}}\n", &mut state, 203);
        assert_eq!(state.status, TaskStatus::Running);
        assert_eq!(state.updated_at, 203);
        assert_eq!(state.token_count, Some(42));
        parse_lines("{\"timestamp\":201,\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"total_tokens\":43}}}}\n", &mut state, 201);
        assert_eq!(state.token_count, Some(43));
        assert_eq!(state.updated_at, 203);
    }

    #[test]
    #[ignore = "read-only real session validation"]
    fn actual_missing_task_enters_registry() {
        let id = "01a0a2a2-6b1e-70b0-859b-276fce495c4e";
        let now = std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
        let mut watcher = RolloutWatcher::new(default_root());
        let start = std::time::Instant::now();
        let tasks = watcher.scan(now);
        let cold = start.elapsed();
        let task = tasks.iter().find(|t| t.id == id).expect("real task absent from watcher");
        println!("target_status={:?} age={} token_present={} cold_ms={}", task.status, now-task.updated_at, task.token_count.is_some(), cold.as_millis());
        let mut registry = crate::task_registry::TaskRegistry::default();
        registry.replace_source_tasks("local-session", tasks, now);
        assert!(registry.tasks().iter().any(|t| t.id == id));
        let start = std::time::Instant::now();
        watcher.scan(now+1);
        println!("warm_ms={} files={}", start.elapsed().as_millis(), watcher.files.len());
    }

    #[test]
    #[ignore = "read-only local session timing; reports counts only"]
    fn local_watcher_timing() {
        let now = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut watcher = RolloutWatcher::new(default_root());
        let start = std::time::Instant::now();
        let tasks = watcher.scan(now);
        let cold = start.elapsed();
        let start = std::time::Instant::now();
        watcher.scan(now + 1);
        println!(
            "files={} visible_tasks={} cold_ms={} warm_ms={}",
            watcher.files.len(),
            tasks.len(),
            cold.as_millis(),
            start.elapsed().as_millis()
        );
    }

    #[test]
    fn lifecycle_survives_frozen_mtime_and_split_records() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("rollout-live.jsonl");
        let now = 1_790_000_000;
        fs::write(&path, format!("{{\"timestamp\":{now},\"type\":\"session_meta\",\"payload\":{{\"session_id\":\"live\"}}}}\n{{\"timestamp\":{now},\"type\":\"response_item\",\"payload\":{{\"type\":\"message\",\"role\":\"user\",\"content\":\"核心任务\"}}}}\n")).unwrap();
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        let old = UNIX_EPOCH + std::time::Duration::from_secs((now - 7200) as u64);
        file.set_modified(old).unwrap();
        let mut watcher = RolloutWatcher::new(root.path().to_path_buf());
        assert_eq!(watcher.scan(now)[0].status, TaskStatus::Running);
        let wait = format!("{{\"timestamp\":{},\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call\",\"name\":\"functions.request_user_input\",\"call_id\":\"wait-1\"}}}}\n", now + 1);
        file.write_all(&wait.as_bytes()[..40]).unwrap();
        file.set_modified(old).unwrap();
        assert_eq!(watcher.scan(now + 1)[0].status, TaskStatus::Running);
        file.write_all(&wait.as_bytes()[40..]).unwrap();
        file.set_modified(old).unwrap();
        assert_eq!(watcher.scan(now + 2)[0].status, TaskStatus::NeedsAction);
        writeln!(
            file,
            "{{\"timestamp\":{},\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\"}}}}",
            now + 3
        )
        .unwrap();
        assert_eq!(watcher.scan(now + 3)[0].status, TaskStatus::NeedsAction);
        writeln!(file, "{{\"timestamp\":{},\"type\":\"response_item\",\"payload\":{{\"type\":\"function_call_output\",\"call_id\":\"wait-1\"}}}}", now + 4).unwrap();
        assert_eq!(watcher.scan(now + 4)[0].status, TaskStatus::Running);
        writeln!(file, "{{\"timestamp\":{},\"type\":\"event_msg\",\"payload\":{{\"type\":\"task_complete\"}}}}", now + 5).unwrap();
        writeln!(
            file,
            "{{\"timestamp\":{},\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\"}}}}",
            now + 6
        )
        .unwrap();
        file.set_modified(old).unwrap();
        assert_eq!(watcher.scan(now + 6)[0].status, TaskStatus::Completed);
        // Restarting the monitor must reconstruct the same final state.
        assert_eq!(
            RolloutWatcher::new(root.path().to_path_buf()).scan(now + 7)[0].status,
            TaskStatus::Completed
        );
    }

    #[test]
    fn silent_work_becomes_unconfirmed_not_completed() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("live.jsonl"), "{\"timestamp\":1000,\"type\":\"response_item\",\"payload\":{\"session_id\":\"live\",\"type\":\"message\",\"role\":\"user\",\"content\":\"long task\"}}\n").unwrap();
        let mut watcher = RolloutWatcher::new(root.path().to_path_buf());
        assert_eq!(watcher.scan(1000)[0].status, TaskStatus::Running);
        assert_eq!(watcher.scan(2000)[0].status, TaskStatus::None);
    }

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
        let root = tempfile::tempdir().unwrap();
        let day = root.path().join("2026").join("08").join("22");
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
        let mut watcher = RolloutWatcher::new(root.path().to_path_buf());
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
    }
}
