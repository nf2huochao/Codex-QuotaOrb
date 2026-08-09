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
    fn parses_usage_tokens() { assert_eq!(parse_usage(include_str!("../fixtures/usage.json")).unwrap().today_tokens, Some(128400)); }
    #[test]
    fn parses_event_and_prioritises_waiting_state() {
        let event = parse_event_line(r#"{"id":"x","event":"approval_required","title":"授权","updated_at":12}"#).unwrap();
        assert!(event.waiting_for_user);
        assert!(!event.running);
    }
    #[test]
    fn rejects_malformed_and_missing_payloads() {
        assert!(parse_rate_limits("not-json").is_err());
        assert!(parse_usage("{}").is_err());
    }
}
