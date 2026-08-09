use crate::{codex_client::CodexClient, domain::{map_task_status, snapshot_status, Snapshot, TaskSummary}, snapshot_store::SnapshotStore};
use std::{sync::Arc, time::Duration};
use tokio_stream::StreamExt;

pub const POLL_INTERVAL: Duration = Duration::from_secs(120);
#[allow(dead_code)]
pub const STALE_AFTER_SECONDS: i64 = 900;

pub async fn poll_once(store: &SnapshotStore, client: &CodexClient, previous: &Snapshot, now: i64) -> Snapshot {
    let rate = client.read_rate_limits().await;
    let usage = client.read_usage().await;
    match (rate, usage) {
        (Ok(rate), Ok(usage)) => {
            let snapshot = Snapshot { status: snapshot_status(Some(now), now, false, true), fetched_at: Some(now), quota_remaining_percent: rate.remaining_percent, quota_resets_at: rate.resets_at, plan: rate.plan, reset_credits: rate.reset_credits, today_tokens: usage.today_tokens, active_task_count: previous.active_task_count, tasks: previous.tasks.clone(), error: None, schema_version: "1.0".into() };
            store.publish(snapshot.clone());
            snapshot
        }
        (rate_result, usage_result) => {
            let mut stale = previous.clone();
            stale.status = snapshot_status(previous.fetched_at, now, true, true);
            stale.error = Some(format!("额度读取失败{}{}", if rate_result.is_err() { "（额度）" } else { "" }, if usage_result.is_err() { "（Token）" } else { "" }));
            store.publish(stale.clone());
            stale
        }
    }
}

pub fn spawn_poll_loop(store: SnapshotStore, client: Arc<CodexClient>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut previous = store.current();
        loop {
            let now = chrono_like_now();
            previous = poll_once(&store, &client, &previous, now).await;
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    })
}

pub fn spawn_event_loop(store: SnapshotStore, client: Arc<CodexClient>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let Ok(mut events) = client.subscribe_events().await else { return; };
        while let Some(Ok(event)) = events.next().await {
            let mut snapshot = store.current();
            let status = map_task_status(&crate::domain::TaskEvent { id: event.id.clone(), title: event.title.clone(), waiting_for_user: event.waiting_for_user, running: event.running, completed: event.completed, token_count: event.token_count, updated_at: event.updated_at });
            if let Some(task) = snapshot.tasks.iter_mut().find(|task| task.id == event.id) {
                task.title = event.title;
                task.status = status;
                task.token_count = event.token_count;
                task.updated_at = event.updated_at;
            } else {
                snapshot.tasks.push(TaskSummary { id: event.id, title: event.title, status, token_count: event.token_count, updated_at: event.updated_at, acknowledged: false });
            }
            store.publish(snapshot);
        }
    })
}

fn chrono_like_now() -> i64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|duration| duration.as_secs() as i64).unwrap_or(0) }
