use crate::domain::{DataStatus, Snapshot};
use std::path::Path;

pub fn load(path: &Path) -> Option<Snapshot> {
    let bytes = std::fs::read(path).ok()?;
    let mut snapshot: Snapshot = serde_json::from_slice(&bytes).ok()?;
    if snapshot.schema_version != "1.0" || snapshot.fetched_at.is_none() {
        return None;
    }
    snapshot.status = DataStatus::Stale;
    snapshot.changed_at = None;
    snapshot.source = Some("local-cache".into());
    snapshot.error = Some("正在连接 Codex，当前显示上次成功数据".into());
    Some(snapshot)
}

pub fn save(path: &Path, snapshot: &Snapshot) -> std::io::Result<()> {
    if snapshot.status != DataStatus::Fresh || snapshot.fetched_at.is_none() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec(snapshot)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{domain::TaskSummary, snapshot_store::empty_snapshot};

    #[test]
    fn cache_load_is_always_marked_stale() {
        let path =
            std::env::temp_dir().join(format!("codex-quota-cache-{}.json", uuid::Uuid::new_v4()));
        let mut snapshot = empty_snapshot();
        snapshot.status = DataStatus::Fresh;
        snapshot.fetched_at = Some(100);
        snapshot.quota_remaining_percent = Some(72);
        snapshot.tasks = Vec::<TaskSummary>::new();
        save(&path, &snapshot).unwrap();
        let cached = load(&path).unwrap();
        assert_eq!(cached.status, DataStatus::Stale);
        assert_eq!(cached.quota_remaining_percent, Some(72));
        assert!(cached.error.as_deref().unwrap().contains("上次成功数据"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stale_snapshot_never_overwrites_cache() {
        let path =
            std::env::temp_dir().join(format!("codex-quota-cache-{}.json", uuid::Uuid::new_v4()));
        let mut fresh = empty_snapshot();
        fresh.status = DataStatus::Fresh;
        fresh.fetched_at = Some(100);
        fresh.today_tokens = Some(10);
        save(&path, &fresh).unwrap();
        let mut stale = fresh.clone();
        stale.status = DataStatus::Stale;
        stale.today_tokens = Some(999);
        save(&path, &stale).unwrap();
        assert_eq!(load(&path).unwrap().today_tokens, Some(10));
        let _ = std::fs::remove_file(path);
    }
}
