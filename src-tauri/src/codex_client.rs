use crate::codex_protocol::{parse_rate_limits, parse_usage, NormalizedTaskEvent, RateLimitResponse, UsageResponse, ProtocolError};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::{atomic::{AtomicU64, Ordering}, Arc}};
use thiserror::Error;
use tokio::{io::{AsyncBufReadExt, AsyncWriteExt, BufReader}, process::{Child, ChildStdin, ChildStdout, Command}, sync::Mutex, time::{timeout, Duration}};
use tokio_stream::{wrappers::ReceiverStream, Stream};

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
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    child: Arc<Mutex<Child>>,
    next_id: AtomicU64,
    timeout: Duration,
}

impl CodexClient {
    pub async fn spawn(codex_binary: &Path) -> Result<Self, CodexError> {
        let mut child = Command::new(codex_binary)
            .args(["app-server", "--stdio"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().ok_or(CodexError::ProcessExited)?;
        let stdout = child.stdout.take().ok_or(CodexError::ProcessExited)?;
        Ok(Self { binary: codex_binary.to_path_buf(), stdin: Arc::new(Mutex::new(stdin)), stdout: Arc::new(Mutex::new(BufReader::new(stdout))), child: Arc::new(Mutex::new(child)), next_id: AtomicU64::new(1), timeout: Duration::from_secs(10) })
    }

    async fn request(&self, method: &str, params: Value) -> Result<Value, CodexError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let line = serde_json::to_string(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(line.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }
        let response = timeout(self.timeout, async {
            let mut stdout = self.stdout.lock().await;
            let mut line = String::new();
            loop {
                line.clear();
                if stdout.read_line(&mut line).await? == 0 { return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "EOF")); }
                let value: Value = serde_json::from_str(&line).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                if value.get("id").and_then(Value::as_u64) == Some(id) { return Ok::<Value, std::io::Error>(value); }
            }
        }).await.map_err(|_| CodexError::Timeout)??;
        if let Some(error) = response.get("error") { return Err(CodexError::Response(error.to_string())); }
        Ok(response.get("result").cloned().unwrap_or(response))
    }

    pub async fn read_rate_limits(&self) -> Result<RateLimitResponse, CodexError> { Ok(parse_rate_limits(&self.request("account/rateLimits/read", json!({})).await?.to_string())?) }
    pub async fn read_usage(&self) -> Result<UsageResponse, CodexError> { Ok(parse_usage(&self.request("account/usage/read", json!({})).await?.to_string())?) }

    pub async fn subscribe_events(&self) -> Result<impl Stream<Item = Result<NormalizedTaskEvent, CodexError>>, CodexError> {
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let stdout = Arc::clone(&self.stdout);
        tokio::spawn(async move {
            loop {
                let mut line = String::new();
                let result = {
                    let mut reader = stdout.lock().await;
                    reader.read_line(&mut line).await
                };
                match result {
                    Ok(0) => break,
                    Ok(_) => { let _ = tx.send(crate::codex_protocol::parse_event_line(line.trim()).map_err(CodexError::from)).await; }
                    Err(error) => { let _ = tx.send(Err(CodexError::Spawn(error))).await; break; }
                }
            }
        });
        Ok(ReceiverStream::new(rx))
    }

    pub fn binary(&self) -> &Path { &self.binary }
    pub async fn stop(&self) -> Result<(), CodexError> { self.child.lock().await.kill().await.map_err(CodexError::Spawn) }
}
