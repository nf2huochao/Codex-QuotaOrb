use crate::domain::{DataStatus, Snapshot, SNAPSHOT_SCHEMA_VERSION};
use serde::Serialize;
use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use time::format_description::well_known::Rfc3339;

#[derive(Serialize)]
pub struct SavedReport {
    pub text: String,
    pub saved_path: String,
}

pub enum CodexBinaryMode {
    Automatic,
    UserSelectedAvailable,
    UserSelectedMissing,
}

impl CodexBinaryMode {
    fn label(&self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::UserSelectedAvailable => "user-selected-available",
            Self::UserSelectedMissing => "user-selected-missing",
        }
    }
}

fn timestamp(value: Option<i64>) -> String {
    value
        .and_then(|seconds| time::OffsetDateTime::from_unix_timestamp(seconds).ok())
        .and_then(|date| date.format(&Rfc3339).ok())
        .unwrap_or_else(|| "unavailable".into())
}

fn data_status(status: DataStatus) -> &'static str {
    match status {
        DataStatus::Fresh => "fresh",
        DataStatus::Stale => "stale",
        DataStatus::Error => "error",
        DataStatus::Unauthenticated => "unauthenticated",
    }
}

fn source(value: Option<&str>) -> &'static str {
    match value {
        Some("metrics-poll") => "metrics-poll",
        Some("task-watch") | Some("local-session") => "local-session",
        Some("manual-refresh") => "manual-refresh",
        Some("permission-hook") | Some("lifecycle-hook") | Some("hook-diagnostic") => "hook",
        Some("cache-unconfirmed") | Some("local-cache") => "local-cache",
        Some("app-server-event") => "app-server-event",
        _ => "other-or-unavailable",
    }
}

fn error_category(value: Option<&str>) -> &'static str {
    match value {
        None => "none",
        Some("usage_unavailable") => "usage-unavailable",
        Some("Codex 未登录，请先在 Codex 中登录") => "not-logged-in",
        Some("Codex app-server 暂时不可用") | Some("Codex app-server 已断开，正在重连…") => {
            "app-server-unavailable"
        }
        Some("Codex 协议版本不兼容") => "protocol-mismatch",
        Some("无法连接 Codex 服务，请检查网络") => "network-unavailable",
        Some("读取 Codex 数据超时") => "timeout",
        Some("Codex 返回的数据格式异常") => "malformed-response",
        Some("Codex 服务暂时无法读取数据，请稍后重试") => "service-unavailable",
        Some(_) => "other-or-unavailable",
    }
}

pub fn build_report(
    snapshot: &Snapshot,
    connected: bool,
    binary_mode: CodexBinaryMode,
    now: i64,
) -> String {
    let hook = &snapshot.hook_diagnostics;
    let hook_delivery = hook
        .last
        .as_ref()
        .map(|last| if last.delivered { "yes" } else { "no" })
        .unwrap_or("unavailable");
    let hook_status = hook
        .last
        .as_ref()
        .and_then(|last| last.http_status)
        .map(|status| status.to_string())
        .unwrap_or_else(|| "unavailable".into());
    let hook_at = timestamp(hook.last.as_ref().map(|last| last.received_at));
    format!(
        "Codex QuotaOrb diagnostic report\n\
         Generated (UTC): {}\n\
         App version: {}\n\
         Platform: {}-{}\n\
         Snapshot schema: {}\n\
         Codex connection: {}\n\
         Codex executable mode: {}\n\
         Data status: {}\n\
         Last successful quota update (UTC): {}\n\
         Last state change (UTC): {}\n\
         State source: {}\n\
         Error category: {}\n\
         Tasks needing action: {}\n\
         Tasks running: {}\n\
         Tasks completed: {}\n\
         Hook events received: {}\n\
         Last Hook received (UTC): {}\n\
         Last Hook delivered: {}\n\
         Last Hook HTTP status: {}\n\n\
         Privacy: no prompts, task text, IDs, credentials, paths, raw logs or raw errors are included.\n",
        timestamp(Some(now)),
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        SNAPSHOT_SCHEMA_VERSION,
        if connected { "connected" } else { "disconnected" },
        binary_mode.label(),
        data_status(snapshot.status),
        timestamp(snapshot.fetched_at),
        timestamp(snapshot.changed_at),
        source(snapshot.source.as_deref()),
        error_category(snapshot.error.as_deref()),
        snapshot.task_counts.needs_action,
        snapshot.task_counts.running,
        snapshot.task_counts.completed,
        hook.received_count,
        hook_at,
        hook_delivery,
        hook_status,
    )
}

pub fn save_report(directory: &Path, text: String, now: i64) -> Result<SavedReport, String> {
    std::fs::create_dir_all(directory).map_err(|_| "无法创建下载文件夹".to_owned())?;
    for attempt in 0..100 {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let path: PathBuf = directory.join(format!("codex-quotaorb-diagnostic-{now}{suffix}.txt"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if file.write_all(text.as_bytes()).is_err() {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err("无法写入诊断报告".into());
                }
                return Ok(SavedReport {
                    text,
                    saved_path: path.to_string_lossy().into_owned(),
                });
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(_) => return Err("无法保存诊断报告，请检查下载文件夹权限".into()),
        }
    }
    Err("同一时间生成的诊断报告过多，请稍后重试".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_excludes_untrusted_text_and_can_be_saved_without_overwriting() {
        let mut snapshot = crate::snapshot_store::empty_snapshot();
        snapshot.error = Some("private prompt C:\\Users\\Alice\\secret.txt token=abc".into());
        snapshot.source = Some("private source".into());
        snapshot.tasks.push(crate::domain::TaskSummary {
            id: "private-session-id".into(),
            title: "private task title".into(),
            activity: Some("private activity".into()),
            waiting_reason: None,
            approval_request_id: Some("private-approval-id".into()),
            status: crate::domain::TaskStatus::Running,
            token_count: None,
            updated_at: 1,
            acknowledged: false,
            source: None,
            turn_id: None,
            received_at: 1,
        });
        snapshot.hook_diagnostics.last = Some(crate::domain::HookDiagnostic {
            event: "private event".into(),
            session_id: Some("private hook session".into()),
            turn_id: Some("private hook turn".into()),
            received_at: 1,
            http_status: Some(200),
            delivered: true,
            error: Some("private hook error".into()),
        });
        let report = build_report(&snapshot, false, CodexBinaryMode::UserSelectedMissing, 1);
        assert!(report.contains("App version:"));
        assert!(report.contains("\nCodex connection: disconnected\n"));
        assert!(report.contains("Codex executable mode: user-selected-missing"));
        assert!(report.contains("Error category: other-or-unavailable"));
        assert!(!report.contains("private"));
        assert!(!report.contains("Alice"));
        assert!(!report.contains("token=abc"));

        let directory = tempfile::tempdir().unwrap();
        let first = save_report(directory.path(), report.clone(), 1).unwrap();
        let second = save_report(directory.path(), report.clone(), 1).unwrap();
        assert_ne!(first.saved_path, second.saved_path);
        assert_eq!(std::fs::read_to_string(first.saved_path).unwrap(), report);
    }
}
