use crate::domain::{HookDiagnostic, HookDiagnostics};
use serde_json::json;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

const MAX_LOG_BYTES: u64 = 1024 * 1024;

fn log_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn log_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.codex.quota-floating-window")
        .join("hook-events.jsonl")
}

fn trim_log(path: &PathBuf) -> Result<(), std::io::Error> {
    let metadata = fs::metadata(path)?;
    if metadata.len() <= MAX_LOG_BYTES {
        return Ok(());
    }
    let bytes = fs::read(path)?;
    let keep = bytes
        .split_at(bytes.len().saturating_sub(MAX_LOG_BYTES as usize))
        .1;
    let start = keep
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    fs::write(path, &keep[start..])
}

pub fn record(
    event: &str,
    session_id: Option<&str>,
    turn_id: Option<&str>,
    at: i64,
    http_status: u16,
    delivered: bool,
    error: Option<&str>,
) -> HookDiagnostics {
    let diagnostic = HookDiagnostic {
        event: event.to_owned(),
        session_id: session_id.map(str::to_owned),
        turn_id: turn_id.map(str::to_owned),
        received_at: at,
        http_status: Some(http_status),
        delivered,
        error: error.map(|value| value.chars().take(160).collect()),
    };
    let path = log_path();
    if let Some(parent) = path.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            eprintln!("hook diagnostic directory write failed: {error}");
        }
    }
    match log_lock().lock() {
        Ok(_guard) => match OpenOptions::new().create(true).append(true).open(&path) {
            Ok(mut file) => {
                let line = json!({
                    "event": diagnostic.event,
                    "session_id": diagnostic.session_id,
                    "turn_id": diagnostic.turn_id,
                    "received_at": diagnostic.received_at,
                    "http_status": diagnostic.http_status,
                    "delivered": diagnostic.delivered,
                    "error": diagnostic.error,
                });
                if let Err(error) = writeln!(file, "{}", line) {
                    eprintln!("hook diagnostic append failed: {error}");
                } else if let Err(error) = file.flush() {
                    eprintln!("hook diagnostic flush failed: {error}");
                } else if let Err(error) = trim_log(&path) {
                    eprintln!("hook diagnostic trim failed: {error}");
                }
            }
            Err(error) => eprintln!("hook diagnostic open failed: {error}"),
        },
        Err(error) => eprintln!("hook diagnostic lock failed: {error}"),
    }
    HookDiagnostics {
        last: Some(diagnostic),
        received_count: 1,
    }
}
