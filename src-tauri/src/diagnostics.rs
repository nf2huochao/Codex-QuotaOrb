use crate::codex_client::CodexError;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCategory {
    NotLoggedIn,
    AppServerUnavailable,
    ProtocolMismatch,
    NetworkUnavailable,
    Timeout,
    MalformedResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub timestamp: i64,
    pub category: DiagnosticCategory,
    pub schema_version: String,
}

impl DiagnosticCategory {
    pub fn message(self) -> &'static str {
        match self {
            Self::NotLoggedIn => "Codex 未登录，请先在 Codex 中登录",
            Self::AppServerUnavailable => "Codex app-server 暂时不可用",
            Self::ProtocolMismatch => "Codex 协议版本不兼容",
            Self::NetworkUnavailable => "局域网连接不可用",
            Self::Timeout => "读取 Codex 数据超时",
            Self::MalformedResponse => "Codex 返回的数据格式异常",
        }
    }
}

pub fn classify(error: &CodexError) -> DiagnosticCategory {
    match error {
        CodexError::Timeout => DiagnosticCategory::Timeout,
        CodexError::Spawn(_) | CodexError::ProcessExited => {
            DiagnosticCategory::AppServerUnavailable
        }
        CodexError::Protocol(_) => DiagnosticCategory::MalformedResponse,
        CodexError::Json(_) => DiagnosticCategory::ProtocolMismatch,
        CodexError::Response(text)
            if text.to_ascii_lowercase().contains("unauthenticated")
                || text.to_ascii_lowercase().contains("login") =>
        {
            DiagnosticCategory::NotLoggedIn
        }
        CodexError::Response(_) => DiagnosticCategory::ProtocolMismatch,
    }
}

pub fn diagnostic(category: DiagnosticCategory, schema_version: &str) -> Diagnostic {
    Diagnostic {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0),
        category,
        schema_version: schema_version.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn messages_are_short_and_redacted() {
        let diagnostic = diagnostic(DiagnosticCategory::ProtocolMismatch, "1.0");
        let text = serde_json::to_string(&diagnostic).unwrap();
        assert!(!text.contains("token"));
        assert_eq!(DiagnosticCategory::Timeout.message(), "读取 Codex 数据超时");
    }
}
