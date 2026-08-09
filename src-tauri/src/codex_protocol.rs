use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("JSON 格式无效: {0}")]
    Json(#[from] serde_json::Error),
    #[error("缺少必要字段: {0}")]
    Missing(&'static str),
    #[error("字段类型无效: {0}")]
    Invalid(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateLimitResponse {
    pub remaining_percent: Option<u8>,
    pub resets_at: Option<i64>,
    pub plan: Option<String>,
    pub reset_credits: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageResponse { pub today_tokens: Option<u64> }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedTaskEvent {
    pub id: String,
    pub title: String,
    pub waiting_for_user: bool,
    pub running: bool,
    pub completed: bool,
    pub token_count: Option<u64>,
    pub updated_at: i64,
}

fn payload(value: Value) -> Value { value.get("result").cloned().unwrap_or(value) }
fn as_i64(value: Option<&Value>) -> Option<i64> { value.and_then(Value::as_i64).or_else(|| value.and_then(Value::as_u64).map(|v| v as i64)) }
fn as_u64(value: Option<&Value>) -> Option<u64> { value.and_then(Value::as_u64).or_else(|| value.and_then(Value::as_i64).map(|v| v.max(0) as u64)) }

pub fn parse_rate_limits(input: &str) -> Result<RateLimitResponse, ProtocolError> {
    let root = payload(serde_json::from_str(input)?);
    let primary = root.get("primary").or_else(|| root.get("rate_limit")).unwrap_or(&root);
    let remaining_percent = as_u64(primary.get("remaining_percent").or_else(|| primary.get("remainingPercent")))
        .or_else(|| as_u64(primary.get("limit")).zip(as_u64(primary.get("used"))).map(|(limit, used)| limit.saturating_sub(used) * 100 / limit.max(1)))
        .map(|v| v.min(100) as u8);
    let resets_at = as_i64(primary.get("resets_at").or_else(|| primary.get("reset_at")).or_else(|| primary.get("resetsAt")));
    let credits = root.get("credits").or_else(|| root.get("reset_credits")).or_else(|| root.get("resetCredits"));
    let reset_credits = credits.and_then(|v| v.as_u64().or_else(|| as_u64(v.get("remaining")).or_else(|| as_u64(v.get("count")))));
    let plan = root.get("plan").or_else(|| root.get("plan_name")).and_then(Value::as_str).map(str::to_owned);
    if remaining_percent.is_none() && resets_at.is_none() && plan.is_none() { return Err(ProtocolError::Missing("primary")); }
    Ok(RateLimitResponse { remaining_percent, resets_at, plan, reset_credits })
}

pub fn parse_usage(input: &str) -> Result<UsageResponse, ProtocolError> {
    let root = payload(serde_json::from_str(input)?);
    let today_tokens = as_u64(root.get("today_tokens").or_else(|| root.get("todayTokens")).or_else(|| root.get("tokens")));
    if today_tokens.is_none() { return Err(ProtocolError::Missing("today_tokens")); }
    Ok(UsageResponse { today_tokens })
}

pub fn parse_event_line(input: &str) -> Result<NormalizedTaskEvent, ProtocolError> {
    let root = payload(serde_json::from_str(input)?);
    let id = root.get("id").or_else(|| root.get("thread_id")).or_else(|| root.get("threadId")).and_then(Value::as_str).ok_or(ProtocolError::Missing("id"))?.to_owned();
    let title = root.get("title").or_else(|| root.get("name")).and_then(Value::as_str).unwrap_or("Codex 任务").to_owned();
    let kind = root.get("event").or_else(|| root.get("type")).and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    let waiting_for_user = root.get("waiting_for_user").or_else(|| root.get("waitingForUser")).and_then(Value::as_bool).unwrap_or(false)
        || kind.contains("approval") || kind.contains("question") || kind.contains("input");
    let completed = root.get("completed").and_then(Value::as_bool).unwrap_or(false) || kind.contains("completed") || kind.contains("done");
    let running = root.get("running").and_then(Value::as_bool).unwrap_or(false) || kind.contains("started") || kind.contains("progress") || kind.contains("running");
    let token_count = as_u64(root.get("token_count").or_else(|| root.get("tokenCount")).or_else(|| root.get("tokens")));
    let updated_at = as_i64(root.get("updated_at").or_else(|| root.get("updatedAt")).or_else(|| root.get("timestamp"))).unwrap_or(0);
    Ok(NormalizedTaskEvent { id, title, waiting_for_user, running, completed, token_count, updated_at })
}
