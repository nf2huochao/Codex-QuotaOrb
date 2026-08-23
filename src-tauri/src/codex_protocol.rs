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
pub struct UsageResponse {
    pub today_tokens: Option<u64>,
    pub usage_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    pub status: String,
    pub updated_at: i64,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedTaskEvent {
    pub id: String,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub request_id: Option<String>,
    pub resolved_request_id: Option<String>,
    pub title: String,
    pub waiting_reason: Option<String>,
    pub approval_request_id: Option<String>,
    pub waiting_for_user: bool,
    pub running: bool,
    pub completed: bool,
    pub token_count: Option<u64>,
    pub updated_at: i64,
}

fn payload(value: Value) -> Value {
    value.get("result").cloned().unwrap_or(value)
}
fn as_i64(value: Option<&Value>) -> Option<i64> {
    value
        .and_then(Value::as_i64)
        .or_else(|| value.and_then(Value::as_u64).map(|v| v as i64))
}
fn as_u64(value: Option<&Value>) -> Option<u64> {
    value
        .and_then(Value::as_u64)
        .or_else(|| value.and_then(Value::as_i64).map(|v| v.max(0) as u64))
}

pub fn parse_rate_limits(input: &str) -> Result<RateLimitResponse, ProtocolError> {
    let root = payload(serde_json::from_str(input)?);
    let snapshot = root
        .get("rateLimits")
        .or_else(|| root.get("rate_limits"))
        .unwrap_or(&root);
    let primary = snapshot
        .get("primary")
        .or_else(|| snapshot.get("individualLimit"))
        .or_else(|| root.get("primary"))
        .or_else(|| root.get("rate_limit"))
        .unwrap_or(snapshot);
    let remaining_percent = as_u64(
        primary
            .get("remaining_percent")
            .or_else(|| primary.get("remainingPercent")),
    )
    .or_else(|| as_u64(primary.get("usedPercent")).map(|used| 100u64.saturating_sub(used)))
    .or_else(|| {
        as_u64(primary.get("limit"))
            .zip(as_u64(primary.get("used")))
            .map(|(limit, used)| limit.saturating_sub(used) * 100 / limit.max(1))
    })
    .map(|v| v.min(100) as u8);
    let resets_at = as_i64(
        primary
            .get("resets_at")
            .or_else(|| primary.get("reset_at"))
            .or_else(|| primary.get("resetsAt")),
    );
    let credits = root
        .get("rateLimitResetCredits")
        .or_else(|| root.get("credits"))
        .or_else(|| root.get("reset_credits"))
        .or_else(|| root.get("resetCredits"));
    let reset_credits = credits.and_then(|v| {
        v.as_u64()
            .or_else(|| as_u64(v.get("remaining")).or_else(|| as_u64(v.get("count"))))
    });
    let plan = snapshot
        .get("planType")
        .or_else(|| snapshot.get("plan"))
        .or_else(|| root.get("plan"))
        .or_else(|| root.get("plan_name"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let reset_credits =
        reset_credits.or_else(|| credits.and_then(|v| as_u64(v.get("availableCount"))));
    if remaining_percent.is_none() && resets_at.is_none() && plan.is_none() {
        return Err(ProtocolError::Missing("primary"));
    }
    Ok(RateLimitResponse {
        remaining_percent,
        resets_at,
        plan,
        reset_credits,
    })
}

pub fn parse_usage(input: &str) -> Result<UsageResponse, ProtocolError> {
    let root = payload(serde_json::from_str(input)?);
    let explicit = as_u64(
        root.get("today_tokens")
            .or_else(|| root.get("todayTokens"))
            .or_else(|| root.get("tokens")),
    );
    let (today_tokens, usage_date) = if explicit.is_some() {
        (explicit, None)
    } else {
        let today = time::OffsetDateTime::now_utc().date().to_string();
        let buckets = root
            .get("dailyUsageBuckets")
            .and_then(Value::as_array)
            .ok_or(ProtocolError::Missing("dailyUsageBuckets"))?;
        let bucket = buckets
            .iter()
            .find(|bucket| {
                bucket
                    .get("startDate")
                    .and_then(Value::as_str)
                    .is_some_and(|date| date == today)
            })
            .or_else(|| {
                buckets
                    .iter()
                    .filter(|bucket| bucket.get("startDate").and_then(Value::as_str).is_some())
                    .max_by_key(|bucket| {
                        bucket
                            .get("startDate")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                    })
            });
        let usage_date = bucket
            .and_then(|bucket| bucket.get("startDate").and_then(Value::as_str))
            .map(str::to_owned);
        (
            bucket.and_then(|bucket| as_u64(bucket.get("tokens"))),
            usage_date,
        )
    };
    if today_tokens.is_none() {
        return Err(ProtocolError::Missing("today_tokens"));
    }
    Ok(UsageResponse {
        today_tokens,
        usage_date,
    })
}

pub fn parse_threads(input: &str) -> Result<Vec<ThreadSummary>, ProtocolError> {
    let root = payload(serde_json::from_str(input)?);
    let data = root
        .get("data")
        .and_then(Value::as_array)
        .ok_or(ProtocolError::Missing("data"))?;
    Ok(data
        .iter()
        .filter_map(|thread| {
            let id = thread.get("id").and_then(Value::as_str)?.to_owned();
            let title = thread
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| {
                    thread.get("preview").and_then(Value::as_str).map(|value| {
                        value
                            .lines()
                            .find(|line| !line.trim().is_empty())
                            .unwrap_or("Codex 任务")
                    })
                })
                .unwrap_or("Codex 任务")
                .trim()
                .chars()
                .take(80)
                .collect::<String>();
            let status = thread
                .get("status")
                .and_then(|value| {
                    value
                        .get("type")
                        .and_then(Value::as_str)
                        .or_else(|| value.as_str())
                })
                .unwrap_or("notLoaded")
                .to_owned();
            let updated_at =
                as_i64(thread.get("updatedAt").or_else(|| thread.get("updated_at"))).unwrap_or(0);
            let path = thread
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_owned);
            Some(ThreadSummary {
                id,
                title,
                status,
                updated_at,
                path,
            })
        })
        .collect())
}

pub fn parse_event_line(input: &str) -> Result<NormalizedTaskEvent, ProtocolError> {
    let root = payload(serde_json::from_str(input)?);
    let params = root.get("params").unwrap_or(&root);
    let thread = params.get("thread");
    let id = params
        .get("thread_id")
        .or_else(|| params.get("threadId"))
        .or_else(|| params.get("id"))
        .or_else(|| params.get("turnId"))
        .or_else(|| params.get("item").and_then(|item| item.get("threadId")))
        .or_else(|| params.get("item").and_then(|item| item.get("thread_id")))
        .and_then(Value::as_str)
        .or_else(|| {
            thread
                .and_then(|value| value.get("id"))
                .and_then(Value::as_str)
        })
        .ok_or(ProtocolError::Missing("id"))?
        .to_owned();
    let title = params
        .get("title")
        .or_else(|| params.get("name"))
        .or_else(|| params.get("threadName"))
        .and_then(Value::as_str)
        .or_else(|| {
            thread
                .and_then(|value| value.get("name"))
                .and_then(Value::as_str)
        })
        .unwrap_or("Codex 任务")
        .to_owned();
    let kind = root
        .get("method")
        .or_else(|| root.get("event"))
        .or_else(|| root.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let request_id = root.get("id").map(|value| {
        value
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| value.to_string())
    });
    let approval_request_id = request_id.clone().filter(|_| {
        kind.contains("approval")
            || kind.contains("requestuserinput")
            || kind.contains("elicitation/request")
    });
    let resolved_request_id = params
        .get("requestId")
        .or_else(|| params.get("request_id"))
        .and_then(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .or_else(|| Some(value.to_string()))
        })
        .filter(|_| kind.contains("serverrequest/resolved") || kind.contains("request/resolved"));
    let turn_id = params
        .get("turnId")
        .or_else(|| params.get("turn_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let item_id = params
        .get("itemId")
        .or_else(|| params.get("item_id"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let waiting_reason = params
        .get("reason")
        .or_else(|| params.get("message"))
        .or_else(|| params.get("command"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let status_type = params
        .get("status")
        .and_then(|value| value.get("type").unwrap_or(value).as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let approval_method = matches!(
        kind.as_str(),
        "approval_required"
            | "item/commandexecution/requestapproval"
            | "item/tool/requestuserinput"
            | "serverrequest/approval"
            | "elicitation/request"
    );
    let completed_method = matches!(
        kind.as_str(),
        "turn/completed" | "turn/complete" | "thread/completed"
    );
    let running_method = matches!(
        kind.as_str(),
        "turn/started" | "turn/progress" | "turn/updated" | "thread/status/changed"
    );
    let waiting_for_user = params
        .get("waiting_for_user")
        .or_else(|| params.get("waitingForUser"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || approval_method;
    let completed = params
        .get("completed")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || completed_method
        || matches!(
            status_type.as_str(),
            "completed" | "complete" | "done" | "idle"
        );
    let running = params
        .get("running")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || running_method
        || matches!(
            status_type.as_str(),
            "active" | "running" | "inprogress" | "in_progress"
        );
    let token_count = as_u64(
        params
            .get("token_count")
            .or_else(|| params.get("tokenCount"))
            .or_else(|| params.get("tokens"))
            .or_else(|| params.get("tokenUsage"))
            .and_then(|value| {
                if value.get("total").is_some() {
                    value.get("total").and_then(|total| {
                        total
                            .get("totalTokens")
                            .or_else(|| total.get("total_tokens"))
                    })
                } else if value.is_object() {
                    value
                        .get("totalTokens")
                        .or_else(|| value.get("total_tokens"))
                } else {
                    Some(value)
                }
            }),
    );
    let updated_at = as_i64(
        params
            .get("updated_at")
            .or_else(|| params.get("updatedAt"))
            .or_else(|| params.get("timestamp")),
    )
    .unwrap_or(0);
    Ok(NormalizedTaskEvent {
        id,
        turn_id,
        item_id,
        request_id,
        resolved_request_id,
        title,
        waiting_reason,
        approval_request_id,
        waiting_for_user,
        running,
        completed,
        token_count,
        updated_at,
    })
}
