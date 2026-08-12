#[cfg(test)]
mod tests {
    use super::super::codex_protocol::*;

    #[test]
    fn parses_remaining_percent_and_reset_credit_count() {
        let response = parse_rate_limits(include_str!("../fixtures/rate_limits.json")).unwrap();
        assert_eq!(response.remaining_percent, Some(72));
        assert_eq!(response.reset_credits, Some(1));
        assert_eq!(response.plan.as_deref(), Some("Plus"));
    }
    #[test]
    fn parses_usage_tokens() {
        let usage = parse_usage(include_str!("../fixtures/usage.json")).unwrap();
        assert_eq!(usage.today_tokens, Some(128400));
        assert_eq!(usage.usage_date, None);
    }
    #[test]
    fn labels_the_actual_usage_bucket_date() {
        let usage = parse_usage(r#"{"dailyUsageBuckets":[{"startDate":"2026-08-10","tokens":10},{"startDate":"2026-08-11","tokens":42}]}"#).unwrap();
        assert_eq!(usage.today_tokens, Some(42));
        assert_eq!(usage.usage_date.as_deref(), Some("2026-08-11"));
    }
    #[test]
    fn parses_thread_list_for_task_status() {
        let threads = parse_threads(r#"{"data":[{"id":"t1","name":"当前任务","status":{"type":"active"},"updatedAt":1786478441,"path":"C:\\codex\\task.jsonl"}]}"#).unwrap();
        assert_eq!(threads[0].id, "t1");
        assert_eq!(threads[0].status, "active");
    }
    #[test]
    fn parses_event_and_prioritises_waiting_state() {
        let event = parse_event_line(
            r#"{"id":"x","event":"approval_required","title":"授权","updated_at":12}"#,
        )
        .unwrap();
        assert!(event.waiting_for_user);
        assert!(!event.running);
    }
    #[test]
    fn parses_current_app_server_thread_notification_shape() {
        let event = parse_event_line(
            r#"{"method":"turn/started","params":{"threadId":"thread-1","turn":{"id":"turn-1"}}}"#,
        )
        .unwrap();
        assert_eq!(event.id, "thread-1");
        assert!(event.running);
    }
    #[test]
    fn parses_thread_status_notification_shape() {
        let event = parse_event_line(r#"{"method":"thread/status/changed","params":{"threadId":"thread-1","status":{"type":"active"}}}"#).unwrap();
        assert_eq!(event.id, "thread-1");
        assert!(event.running);
        assert!(!event.completed);
    }
    #[test]
    fn rejects_malformed_and_missing_payloads() {
        assert!(parse_rate_limits("not-json").is_err());
        assert!(parse_usage("{}").is_err());
    }
}
