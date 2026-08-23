use crate::codex_protocol::{
    parse_rate_limits, parse_threads, parse_usage, ProtocolError, RateLimitResponse, ThreadSummary,
    UsageResponse,
};
use futures_util::future::join_all;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::{broadcast, oneshot, Mutex},
    time::{timeout, Duration},
};

#[derive(Debug, Error)]
pub enum CodexError {
    #[error("无法启动 Codex app-server: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("Codex 请求超时")]
    Timeout,
    #[error("Codex 进程已退出")]
    ProcessExited,
    #[error("Codex 协议错误: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("Codex JSON 编码错误: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Codex 响应无效: {0}")]
    Response(String),
}

pub struct CodexClient {
    binary: PathBuf,
    stdin: Arc<Mutex<ChildStdin>>,
    child: Arc<Mutex<Child>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    pending_approvals: Arc<Mutex<HashMap<String, Value>>>,
    thread_status_cache: Arc<Mutex<HashMap<String, (i64, i64, String)>>>,
    notifications: broadcast::Sender<Value>,
    connected: Arc<AtomicBool>,
    next_id: AtomicU64,
    timeout: Duration,
}

impl CodexClient {
    pub async fn spawn(codex_binary: &Path) -> Result<Self, CodexError> {
        let mut command = Command::new(codex_binary);
        command
            .args(["app-server", "--stdio"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn()?;
        let stdin = child.stdin.take().ok_or(CodexError::ProcessExited)?;
        let stdout = child.stdout.take().ok_or(CodexError::ProcessExited)?;
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let pending_approvals = Arc::new(Mutex::new(HashMap::new()));
        let thread_status_cache = Arc::new(Mutex::new(HashMap::new()));
        let (notifications, _) = broadcast::channel(128);
        let connected = Arc::new(AtomicBool::new(true));
        let client = Self {
            binary: codex_binary.to_path_buf(),
            stdin: Arc::new(Mutex::new(stdin)),
            child: Arc::new(Mutex::new(child)),
            pending: Arc::clone(&pending),
            pending_approvals: Arc::clone(&pending_approvals),
            thread_status_cache,
            notifications: notifications.clone(),
            connected: Arc::clone(&connected),
            next_id: AtomicU64::new(1),
            timeout: Duration::from_secs(10),
        };
        tokio::spawn(read_loop(
            BufReader::new(stdout),
            pending,
            pending_approvals,
            notifications,
            connected,
        ));
        if let Err(error) = client.initialize().await {
            let _ = client.stop().await;
            return Err(error);
        }
        Ok(client)
    }

    async fn initialize(&self) -> Result<(), CodexError> {
        let _ = self.request("initialize", json!({ "clientInfo": { "name": "codex-quota-floating-window", "title": "Codex 额度悬浮窗", "version": "0.1.0" }, "capabilities": null })).await?;
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"initialized\",\"params\":null}\n")
            .await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, CodexError> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(CodexError::ProcessExited);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let line = serde_json::to_string(
            &json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}),
        )?;
        let (response_tx, response_rx) = oneshot::channel();
        self.pending.lock().await.insert(id, response_tx);
        {
            let mut stdin = self.stdin.lock().await;
            if let Err(error) = async {
                stdin.write_all(line.as_bytes()).await?;
                stdin.write_all(b"\n").await?;
                stdin.flush().await
            }
            .await
            {
                self.pending.lock().await.remove(&id);
                self.connected.store(false, Ordering::Release);
                return Err(CodexError::Spawn(error));
            }
        }
        let response = match timeout(self.timeout, response_rx).await {
            Err(_) => {
                self.pending.lock().await.remove(&id);
                // A slow optional request (for example, probing one external
                // thread) must not tear down the whole app-server session.
                // The read loop is the source of truth for process-level
                // disconnects; it flips `connected` when stdout closes or an
                // actual I/O error occurs. Keeping the session alive here lets
                // the next poll retry metrics and task reads normally.
                return Err(CodexError::Timeout);
            }
            Ok(Err(_)) => return Err(CodexError::ProcessExited),
            Ok(Ok(Err(error))) => return Err(CodexError::Response(error)),
            Ok(Ok(Ok(value))) => value,
        };
        if let Some(error) = response.get("error") {
            return Err(CodexError::Response(error.to_string()));
        }
        Ok(response.get("result").cloned().unwrap_or(response))
    }

    pub async fn read_rate_limits(&self) -> Result<RateLimitResponse, CodexError> {
        Ok(parse_rate_limits(
            &self
                .request("account/rateLimits/read", Value::Null)
                .await?
                .to_string(),
        )?)
    }
    pub async fn read_usage(&self) -> Result<UsageResponse, CodexError> {
        Ok(parse_usage(
            &self
                .request("account/usage/read", Value::Null)
                .await?
                .to_string(),
        )?)
    }

    pub async fn read_threads(&self) -> Result<Vec<ThreadSummary>, CodexError> {
        // Ask app-server for the complete thread view. `useStateDbOnly` can
        // omit sessions owned by another Codex app-server process, which is
        // exactly how an active project could disappear from the monitor.
        let response = self
            .request(
                "thread/list",
                json!({ "limit": 50, "sortKey": "updated_at", "archived": false }),
            )
            .await?;
        let mut threads = parse_threads(&response.to_string())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        let mut probes = Vec::new();
        {
            let cache = self.thread_status_cache.lock().await;
            for (index, thread) in threads.iter_mut().enumerate() {
                let status = thread.status.to_ascii_lowercase();
                if matches!(status.as_str(), "active" | "running")
                    || now.saturating_sub(thread.updated_at) > THREAD_STATUS_PROBE_MAX_AGE
                {
                    continue;
                }
                if let Some((cached_at, checked_at, cached_status)) = cache.get(&thread.id) {
                    if *cached_at == thread.updated_at
                        && now.saturating_sub(*checked_at) < THREAD_STATUS_CACHE_SECONDS
                    {
                        thread.status = cached_status.clone();
                        continue;
                    }
                }
                if probes.len() >= THREAD_STATUS_PROBE_LIMIT {
                    break;
                }
                probes.push((index, thread.id.clone(), thread.updated_at));
            }
        }
        let results = join_all(
            probes
                .into_iter()
                .map(|(index, id, updated_at)| async move {
                    let result = self
                        .request(
                            "thread/read",
                            json!({ "threadId": id, "includeTurns": true }),
                        )
                        .await;
                    (index, id, updated_at, result)
                }),
        )
        .await;
        let mut cache = self.thread_status_cache.lock().await;
        for (index, id, updated_at, result) in results {
            let status = result
                .ok()
                .and_then(|value| thread_read_status(&value))
                .unwrap_or_else(|| "notLoaded".into());
            threads[index].status = status.clone();
            cache.insert(id, (updated_at, now, status));
        }
        Ok(threads)
    }

    pub fn subscribe_notifications(&self) -> broadcast::Receiver<Value> {
        self.notifications.subscribe()
    }
    pub async fn respond_to_approval(
        &self,
        approval_request_id: &str,
        decision: &str,
    ) -> Result<(), CodexError> {
        if !matches!(decision, "accept" | "decline") {
            return Err(CodexError::Response("批准决定无效".into()));
        }
        let request_id = self
            .pending_approvals
            .lock()
            .await
            .remove(approval_request_id)
            .ok_or_else(|| CodexError::Response("批准请求已失效或已处理".into()))?;
        let line = serde_json::to_string(&json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "result": { "decision": decision }
        }))?;
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(line.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        Ok(())
    }
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    pub fn binary(&self) -> &Path {
        &self.binary
    }
    pub async fn stop(&self) -> Result<(), CodexError> {
        self.connected.store(false, Ordering::Release);
        self.child
            .lock()
            .await
            .kill()
            .await
            .map_err(CodexError::Spawn)
    }
}

const THREAD_STATUS_PROBE_LIMIT: usize = 12;
const THREAD_STATUS_PROBE_MAX_AGE: i64 = 6 * 60 * 60;
const THREAD_STATUS_CACHE_SECONDS: i64 = 5;

fn thread_read_status(value: &Value) -> Option<String> {
    let thread = value.get("thread")?;
    let direct = status_text(thread.get("status"));
    if direct
        .as_deref()
        .is_some_and(|status| !status.eq_ignore_ascii_case("notloaded"))
    {
        return direct;
    }
    let latest_turn = thread.get("turns")?.as_array()?.last()?;
    let latest = status_text(latest_turn.get("status"))?.to_ascii_lowercase();
    if matches!(
        latest.as_str(),
        "inprogress" | "in_progress" | "running" | "started"
    ) {
        return Some("active".into());
    }
    if matches!(latest.as_str(), "completed" | "complete" | "done" | "idle") {
        return Some("completed".into());
    }
    Some("notLoaded".into())
}

fn status_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|value| {
            value
                .as_str()
                .or_else(|| value.get("type").and_then(Value::as_str))
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::thread_read_status;
    use serde_json::json;

    #[test]
    fn thread_read_uses_latest_in_progress_turn_for_external_sessions() {
        let value = json!({
            "thread": {
                "status": { "type": "notLoaded" },
                "turns": [{ "status": "inProgress" }]
            }
        });
        assert_eq!(thread_read_status(&value).as_deref(), Some("active"));
    }

    #[test]
    fn thread_read_does_not_turn_an_interrupted_turn_into_running() {
        let value = json!({
            "thread": {
                "status": { "type": "notLoaded" },
                "turns": [{ "status": "interrupted" }]
            }
        });
        assert_eq!(thread_read_status(&value).as_deref(), Some("notLoaded"));
    }
}

async fn read_loop(
    mut stdout: BufReader<tokio::process::ChildStdout>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>,
    pending_approvals: Arc<Mutex<HashMap<String, Value>>>,
    notifications: broadcast::Sender<Value>,
    connected: Arc<AtomicBool>,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match stdout.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let value: Value = match serde_json::from_str(&line) {
                    Ok(value) => value,
                    Err(error) => {
                        log::warn!("Codex app-server returned malformed JSON: {error}");
                        continue;
                    }
                };
                // app-server approval prompts are JSON-RPC *requests*: they carry
                // both `id` and `method`. Route every method-bearing message to
                // the notification stream first, otherwise an approval prompt
                // with a numeric id is mistaken for a response and dropped.
                if value.get("method").is_some() {
                    let method = value.get("method").and_then(Value::as_str).unwrap_or("");
                    let method_name = method.to_ascii_lowercase();
                    if method_name.contains("requestapproval")
                        || method_name.contains("requestuserinput")
                        || method_name.contains("elicitation/request")
                    {
                        if let Some(request_id) = value.get("id") {
                            let key = request_id
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| request_id.to_string());
                            pending_approvals
                                .lock()
                                .await
                                .insert(key, request_id.clone());
                        }
                    }
                    let _ = notifications.send(value);
                } else if let Some(id) = value.get("id").and_then(Value::as_u64) {
                    if let Some(sender) = pending.lock().await.remove(&id) {
                        let _ = sender.send(Ok(value));
                    }
                }
            }
            Err(error) => {
                log::warn!("Codex app-server read failed: {error}");
                break;
            }
        }
    }
    connected.store(false, Ordering::Release);
    let mut waiting = pending.lock().await;
    for (_, sender) in waiting.drain() {
        let _ = sender.send(Err("Codex app-server 连接已断开".into()));
    }
}
