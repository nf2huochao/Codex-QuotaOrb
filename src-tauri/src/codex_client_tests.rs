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
    fn parses_string_thread_status_without_downgrading_to_not_loaded() {
        let threads = parse_threads(
            r#"{"data":[{"id":"t2","name":"外部任务","status":"active","updatedAt":1786478441}]}"#,
        )
        .unwrap();
        assert_eq!(threads[0].status, "active");
    }
    #[test]
    fn parses_event_and_prioritises_waiting_state() {
        let event = parse_event_line(
            r#"{"id":"x","event":"approval_required","title":"授权","updated_at":12}"#,
        )
        .unwrap();
        assert!(event.waiting_for_user);
        assert_eq!(event.approval_request_id.as_deref(), Some("x"));
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
    fn parses_command_approval_request_shape_as_needs_action() {
        let event = parse_event_line(
            r#"{"jsonrpc":"2.0","id":7,"method":"item/commandExecution/requestApproval","params":{"threadId":"thread-1","turnId":"turn-1","itemId":"item-1","command":"git push","cwd":"C:\\work","reason":"需要确认"}}"#,
        )
        .unwrap();
        assert_eq!(event.id, "thread-1");
        assert_eq!(event.turn_id.as_deref(), Some("turn-1"));
        assert_eq!(event.item_id.as_deref(), Some("item-1"));
        assert_eq!(event.request_id.as_deref(), Some("7"));
        assert_eq!(event.approval_request_id.as_deref(), Some("7"));
        assert_eq!(event.waiting_reason.as_deref(), Some("需要确认"));
        assert!(event.waiting_for_user);
        assert!(!event.running);
        assert!(!event.completed);
    }
    #[test]
    fn parses_user_input_and_resolution_requests() {
        let request = parse_event_line(r#"{"jsonrpc":"2.0","id":8,"method":"item/tool/requestUserInput","params":{"threadId":"thread-2","turnId":"turn-2","itemId":"item-2","questions":[]}}"#).unwrap();
        assert_eq!(request.id, "thread-2");
        assert_eq!(request.approval_request_id.as_deref(), Some("8"));
        assert!(request.waiting_for_user);
        let resolved = parse_event_line(
            r#"{"method":"serverRequest/resolved","params":{"threadId":"thread-2","requestId":8}}"#,
        )
        .unwrap();
        assert_eq!(resolved.id, "thread-2");
        assert_eq!(resolved.resolved_request_id.as_deref(), Some("8"));
    }
    #[test]
    fn rejects_malformed_and_missing_payloads() {
        assert!(parse_rate_limits("not-json").is_err());
        assert!(parse_usage("{}").is_err());
    }
}
