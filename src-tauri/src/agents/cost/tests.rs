use std::collections::HashMap;
use crate::agents::cost::*;

#[test]
fn parse_cost_from_claude_result() {
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}
{"type":"result","result":"Hello world","usage":{"input_tokens":1000,"output_tokens":500,"cache_read_input_tokens":200,"cache_creation_input_tokens":100,"total_cost_usd":0.0525},"modelUsage":{"claude-opus-4-6":{"inputTokens":1000,"outputTokens":500,"cacheReadInputTokens":200,"cacheCreationInputTokens":100,"costUSD":0.0525}}}"#;

    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert_eq!(cost.input_tokens, 1000);
    assert_eq!(cost.output_tokens, 500);
    assert_eq!(cost.cache_read_tokens, 200);
    assert_eq!(cost.cache_creation_tokens, 100);
    assert!((cost.total_cost_usd - 0.0525).abs() < 0.0001);
    assert!(!cost.is_estimated);

    // Model name should be normalized from "claude-opus-4-6" -> "opus-4.6"
    let model = cost.model_usage.get("opus-4.6").unwrap();
    assert_eq!(model.input_tokens, 1000);
    assert_eq!(model.output_tokens, 500);
    assert!((model.cost_usd - 0.0525).abs() < 0.0001);
}

#[test]
fn parse_cost_missing_usage_returns_none() {
    let stream_output = r#"{"type":"result","result":"Hello world"}"#;
    let cost = extract_cost_from_stream_json(stream_output);
    assert!(cost.is_none());
}

#[test]
fn parse_cost_zero_tokens_returns_none() {
    let stream_output = r#"{"type":"result","result":"","usage":{"input_tokens":0,"output_tokens":0,"total_cost_usd":0.0}}"#;
    let cost = extract_cost_from_stream_json(stream_output);
    assert!(cost.is_none());
}

#[test]
fn parse_cost_cache_only_tokens_returns_some() {
    let stream_output = r#"{"type":"result","result":"","usage":{"input_tokens":0,"output_tokens":0,"cache_read_input_tokens":500,"cache_creation_input_tokens":0,"total_cost_usd":0.0}}"#;
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert_eq!(cost.input_tokens, 0);
    assert_eq!(cost.output_tokens, 0);
    assert_eq!(cost.cache_read_tokens, 500);
    assert!(!cost.is_estimated);
}

#[test]
fn parse_cost_cache_creation_only_returns_some() {
    let stream_output = r#"{"type":"result","result":"","usage":{"input_tokens":0,"output_tokens":0,"cache_read_input_tokens":0,"cache_creation_input_tokens":300,"total_cost_usd":0.0}}"#;
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert_eq!(cost.cache_creation_tokens, 300);
    assert!(!cost.is_estimated);
}

#[test]
fn parse_cost_no_result_line() {
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"hello"}}}"#;
    let cost = extract_cost_from_stream_json(stream_output);
    assert!(cost.is_none());
}

#[test]
fn parse_cost_partial_usage() {
    let stream_output =
        r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50}}"#;
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert_eq!(cost.input_tokens, 100);
    assert_eq!(cost.output_tokens, 50);
    assert_eq!(cost.cache_read_tokens, 0);
    assert_eq!(cost.total_cost_usd, 0.0);
    assert!(!cost.is_estimated);
}

#[test]
fn estimate_cost_opus() {
    let cost = estimate_cost("claude-opus-4-6", 4000, 10.0);
    assert!(cost.is_estimated);
    assert!(cost.total_cost_usd > 0.0);
    assert_eq!(cost.output_tokens, 1000); // 4000 chars / 4
    assert!(cost.model_usage.contains_key("opus-4.6"));
}

#[test]
fn estimate_cost_sonnet() {
    let cost_opus = estimate_cost("opus-4.6", 4000, 10.0);
    let cost_sonnet = estimate_cost("sonnet-4.5", 4000, 10.0);
    assert!(cost_opus.total_cost_usd > cost_sonnet.total_cost_usd);
}

#[test]
fn estimate_cost_zero_output() {
    let cost = estimate_cost("sonnet-4.5", 0, 5.0);
    assert!(cost.is_estimated);
    assert_eq!(cost.output_tokens, 0);
    assert!(cost.input_tokens > 0);
}

#[test]
fn extract_via_claude_provider_prefers_parsed() {
    use crate::agents::claude::provider::ClaudeProvider;
    use crate::agents::provider::AgentProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.01}}"#;
    let cost = provider.extract_cost(stream_output, "opus-4.6", 10.0).unwrap();
    assert!(!cost.is_estimated);
    assert_eq!(cost.input_tokens, 100);
}

#[test]
fn extract_via_cursor_provider_estimates() {
    use crate::agents::cursor::provider::CursorProvider;
    use crate::agents::provider::AgentProvider;
    let provider = CursorProvider::new();
    let plain_output = "This is plain text from Cursor agent";
    let cost = provider.extract_cost(plain_output, "opus-4.6", 10.0).unwrap();
    assert!(cost.is_estimated);
    assert!(cost.total_cost_usd > 0.0);
}

#[test]
fn aggregated_cost_add() {
    let mut agg = AggregatedCost::default();

    let cost1 = RunCostData {
        input_tokens: 100,
        output_tokens: 50,
        total_cost_usd: 0.01,
        is_estimated: false,
        ..Default::default()
    };
    let cost2 = RunCostData {
        input_tokens: 200,
        output_tokens: 100,
        total_cost_usd: 0.02,
        is_estimated: true,
        ..Default::default()
    };

    agg.add(&cost1);
    agg.add(&cost2);

    assert_eq!(agg.run_count, 2);
    assert_eq!(agg.estimated_count, 1);
    assert_eq!(agg.total_input_tokens, 300);
    assert_eq!(agg.total_output_tokens, 150);
    assert!((agg.total_cost_usd - 0.03).abs() < 0.0001);
    assert!(agg.model_totals.contains_key("other"));
    assert!((agg.model_totals["other"].cost_usd - 0.03).abs() < 0.0001);
}

#[test]
fn aggregated_cost_model_totals_sum_equals_total() {
    let mut agg = AggregatedCost::default();

    let mut usage1 = HashMap::new();
    usage1.insert("opus-4.6".to_string(), ModelCostData {
        input_tokens: 100,
        output_tokens: 50,
        cost_usd: 0.05,
        ..Default::default()
    });
    agg.add(&RunCostData {
        input_tokens: 100,
        output_tokens: 50,
        total_cost_usd: 0.05,
        model_usage: usage1,
        ..Default::default()
    });

    agg.add(&RunCostData {
        input_tokens: 200,
        output_tokens: 100,
        total_cost_usd: 0.03,
        model_usage: HashMap::new(),
        ..Default::default()
    });

    let model_sum: f64 = agg.model_totals.values().map(|d| d.cost_usd).sum();
    assert!(
        (model_sum - agg.total_cost_usd).abs() < 0.0001,
        "model_totals sum ({}) must equal total_cost_usd ({})",
        model_sum, agg.total_cost_usd
    );
    assert_eq!(agg.model_totals.len(), 2);
    assert!(agg.model_totals.contains_key("other"));
}

#[test]
fn cost_data_serialization_roundtrip() {
    let cost = RunCostData {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: 200,
        cache_creation_tokens: 100,
        total_cost_usd: 0.05,
        model_usage: HashMap::new(),
        is_estimated: false,
    };
    let json = serde_json::to_string(&cost).unwrap();
    let parsed: RunCostData = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.input_tokens, 1000);
    assert_eq!(parsed.output_tokens, 500);
    assert!((parsed.total_cost_usd - 0.05).abs() < 0.001);
}

#[test]
fn extract_cost_empty_input() {
    assert!(extract_cost_from_stream_json("").is_none());
    assert!(extract_cost_from_stream_json("   \n\n  ").is_none());
}

#[test]
fn extract_cost_skips_malformed_json_lines() {
    let stream_output = "not json at all\n{broken json\n{\"type\":\"result\",\"result\":\"ok\",\"usage\":{\"input_tokens\":10,\"output_tokens\":5}}";
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert_eq!(cost.input_tokens, 10);
}

#[test]
fn extract_cost_multi_model_usage() {
    let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.05},"modelUsage":{"claude-opus-4-6":{"inputTokens":80,"outputTokens":40,"costUSD":0.04},"claude-sonnet-4-5":{"inputTokens":20,"outputTokens":10,"costUSD":0.01}}}"#;
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert_eq!(cost.model_usage.len(), 2);
    assert!(cost.model_usage.contains_key("opus-4.6"));
    assert!(cost.model_usage.contains_key("sonnet-4.5"));
    let opus = &cost.model_usage["opus-4.6"];
    let sonnet = &cost.model_usage["sonnet-4.5"];
    assert_eq!(opus.input_tokens + sonnet.input_tokens, 100);
    assert!((cost.total_cost_usd - 0.05).abs() < 0.0001);
}

#[test]
fn extract_cost_result_without_model_usage() {
    let stream_output =
        r#"{"type":"result","result":"text","usage":{"input_tokens":50,"output_tokens":25,"total_cost_usd":0.01}}"#;
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert!(cost.model_usage.is_empty());
    assert_eq!(cost.input_tokens, 50);
}

#[test]
fn estimate_cost_haiku() {
    let cost = estimate_cost("claude-haiku-3", 4000, 10.0);
    assert!(cost.is_estimated);
    let sonnet_cost = estimate_cost("sonnet-4.5", 4000, 10.0);
    assert!(cost.total_cost_usd < sonnet_cost.total_cost_usd);
}

#[test]
fn estimate_cost_unknown_model_uses_sonnet_pricing() {
    let unknown = estimate_cost("gpt-5", 4000, 10.0);
    let sonnet = estimate_cost("sonnet-4.5", 4000, 10.0);
    assert!((unknown.total_cost_usd - sonnet.total_cost_usd).abs() < 0.0001);
}

#[test]
fn compute_cost_from_tokens_includes_cache_pricing() {
    let cost = compute_cost_from_tokens("opus-4.6", 1_000_000, 1_000_000, 1_000_000, 1_000_000);
    let expected = 5.0 + 25.0 + 0.50 + 6.25;
    assert!((cost - expected).abs() < 0.001, "Opus full breakdown should be ${expected}, got ${cost}");

    let cost_s = compute_cost_from_tokens("sonnet-4.5", 1_000_000, 1_000_000, 1_000_000, 1_000_000);
    let expected_s = 3.0 + 15.0 + 0.30 + 3.75;
    assert!((cost_s - expected_s).abs() < 0.001, "Sonnet full breakdown should be ${expected_s}, got ${cost_s}");

    let cost_h = compute_cost_from_tokens("haiku-4.5", 1_000_000, 1_000_000, 1_000_000, 1_000_000);
    let expected_h = 1.0 + 5.0 + 0.10 + 1.25;
    assert!((cost_h - expected_h).abs() < 0.001, "Haiku full breakdown should be ${expected_h}, got ${cost_h}");
}

#[test]
fn estimate_cost_matches_anthropic_pricing() {
    let chars_for_1m_tokens = 4_000_000;
    let secs_for_1m_tokens = 2000.0;

    let opus = estimate_cost("opus-4.6", chars_for_1m_tokens, secs_for_1m_tokens);
    assert!((opus.total_cost_usd - 30.0).abs() < 0.01, "Opus 1M+1M should be ~$30, got {}", opus.total_cost_usd);

    let sonnet = estimate_cost("sonnet-4.5", chars_for_1m_tokens, secs_for_1m_tokens);
    assert!((sonnet.total_cost_usd - 18.0).abs() < 0.01, "Sonnet 1M+1M should be ~$18, got {}", sonnet.total_cost_usd);

    let haiku = estimate_cost("haiku-4.5", chars_for_1m_tokens, secs_for_1m_tokens);
    assert!((haiku.total_cost_usd - 6.0).abs() < 0.01, "Haiku 1M+1M should be ~$6, got {}", haiku.total_cost_usd);
}

#[test]
fn extract_via_claude_provider_no_result_falls_back() {
    use crate::agents::claude::provider::ClaudeProvider;
    use crate::agents::provider::AgentProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"hello"}}}"#;
    let cost = provider.extract_cost(stream_output, "opus-4.6", 5.0).unwrap();
    assert!(cost.is_estimated);
}

#[test]
fn extract_via_cursor_provider_empty_returns_none() {
    use crate::agents::cursor::provider::CursorProvider;
    use crate::agents::provider::AgentProvider;
    let provider = CursorProvider::new();
    let cost = provider.extract_cost("", "opus-4.6", 0.0);
    assert!(cost.is_none());
}

#[test]
fn extract_via_cursor_provider_empty_positive_duration() {
    use crate::agents::cursor::provider::CursorProvider;
    use crate::agents::provider::AgentProvider;
    let provider = CursorProvider::new();
    let cost = provider.extract_cost("", "opus-4.6", 5.0).unwrap();
    assert!(cost.is_estimated);
    assert!(cost.input_tokens > 0);
    assert_eq!(cost.output_tokens, 0);
}

#[test]
fn normalize_model_name_maps_claude_variants() {
    assert_eq!(normalize_model_name("claude-opus-4-6"), "opus-4.6");
    assert_eq!(normalize_model_name("claude-opus-4-5"), "opus-4.5");
    assert_eq!(normalize_model_name("claude-sonnet-4-5"), "sonnet-4.5");
    assert_eq!(normalize_model_name("claude-haiku-3"), "haiku-3.5");
}

#[test]
fn normalize_model_name_preserves_short_form() {
    assert_eq!(normalize_model_name("opus-4.6"), "opus-4.6");
    assert_eq!(normalize_model_name("sonnet-4.5"), "sonnet-4.5");
}

#[test]
fn normalize_model_name_strips_claude_prefix_for_unknown() {
    assert_eq!(normalize_model_name("claude-future-model"), "future-model");
}

#[test]
fn normalize_model_name_is_case_insensitive() {
    assert_eq!(normalize_model_name("Claude-Opus-4-6"), "opus-4.6");
    assert_eq!(normalize_model_name("CLAUDE-SONNET-4-5"), "sonnet-4.5");
}

#[test]
fn total_always_equals_model_sum() {
    let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.04},"modelUsage":{"claude-opus-4-6":{"inputTokens":80,"outputTokens":40,"costUSD":0.04},"claude-sonnet-4-5":{"inputTokens":20,"outputTokens":10,"costUSD":0.01}}}"#;
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert!((cost.total_cost_usd - 0.05).abs() < 0.0001, "total_cost_usd should be model sum 0.05, got {}", cost.total_cost_usd);
}

#[test]
fn zero_total_with_model_costs_derives_total_from_models() {
    let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0},"modelUsage":{"claude-opus-4-6":{"inputTokens":80,"outputTokens":40,"costUSD":0.04},"claude-sonnet-4-5":{"inputTokens":20,"outputTokens":10,"costUSD":0.01}}}"#;
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert!((cost.total_cost_usd - 0.05).abs() < 0.0001, "total should be model sum 0.05, got {}", cost.total_cost_usd);
}

#[test]
fn missing_total_with_model_costs_derives_total_from_models() {
    let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50},"modelUsage":{"claude-opus-4-6":{"inputTokens":80,"outputTokens":40,"costUSD":0.04},"claude-sonnet-4-5":{"inputTokens":20,"outputTokens":10,"costUSD":0.01}}}"#;
    let cost = extract_cost_from_stream_json(stream_output).unwrap();
    assert!((cost.total_cost_usd - 0.05).abs() < 0.0001, "total should be model sum 0.05, got {}", cost.total_cost_usd);
}

#[test]
fn extract_via_provider_backfills_empty_model_usage() {
    use crate::agents::claude::provider::ClaudeProvider;
    use crate::agents::provider::AgentProvider;
    let provider = ClaudeProvider::new();
    let stream_output =
        r#"{"type":"result","result":"text","usage":{"input_tokens":50,"output_tokens":25,"total_cost_usd":0.01}}"#;
    let cost = provider.extract_cost(stream_output, "opus-4.6", 10.0).unwrap();
    assert!(!cost.is_estimated);
    assert!(!cost.model_usage.is_empty(), "model_usage should have a fallback entry");
    assert!(cost.model_usage.contains_key("opus-4.6"));
    assert!((cost.model_usage["opus-4.6"].cost_usd - 0.01).abs() < 0.0001);
    assert!((cost.total_cost_usd - 0.01).abs() < 0.0001);
}

#[test]
fn extract_via_provider_preserves_existing_model_usage() {
    use crate::agents::claude::provider::ClaudeProvider;
    use crate::agents::provider::AgentProvider;
    let provider = ClaudeProvider::new();
    let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.05},"modelUsage":{"claude-opus-4-6":{"inputTokens":100,"outputTokens":50,"costUSD":0.05}}}"#;
    let cost = provider.extract_cost(stream_output, "opus-4.6", 10.0).unwrap();
    assert_eq!(cost.model_usage.len(), 1);
    assert!(cost.model_usage.contains_key("opus-4.6"));
    assert!((cost.model_usage["opus-4.6"].cost_usd - 0.05).abs() < 0.0001);
}

#[test]
fn estimate_cost_normalizes_model_name() {
    let cost = estimate_cost("opus-4.6", 4000, 10.0);
    assert!(cost.model_usage.contains_key("opus-4.6"));

    let cost2 = estimate_cost("claude-opus-4-6", 4000, 10.0);
    assert!(cost2.model_usage.contains_key("opus-4.6"));
    assert!((cost.total_cost_usd - cost2.total_cost_usd).abs() < 0.0001);
}

#[test]
fn aggregated_cost_merges_different_name_variants() {
    let mut agg = AggregatedCost::default();

    let mut usage1 = HashMap::new();
    usage1.insert("claude-opus-4-6".to_string(), ModelCostData {
        input_tokens: 100, output_tokens: 50, cost_usd: 0.01, ..Default::default()
    });
    agg.add(&RunCostData { model_usage: usage1, total_cost_usd: 0.01, ..Default::default() });

    let mut usage2 = HashMap::new();
    usage2.insert("opus-4.6".to_string(), ModelCostData {
        input_tokens: 200, output_tokens: 100, cost_usd: 0.02, ..Default::default()
    });
    agg.add(&RunCostData { model_usage: usage2, total_cost_usd: 0.02, ..Default::default() });

    assert_eq!(agg.model_totals.len(), 1, "Should have 1 model entry, not 2");
    assert!(agg.model_totals.contains_key("opus-4.6"));
    assert_eq!(agg.model_totals["opus-4.6"].input_tokens, 300);
    assert!((agg.model_totals["opus-4.6"].cost_usd - 0.03).abs() < 0.0001);
    assert!((agg.total_cost_usd - 0.03).abs() < 0.0001);
}

#[test]
fn aggregated_cost_model_usage_merges() {
    let mut agg = AggregatedCost::default();

    let mut usage1 = HashMap::new();
    usage1.insert("opus".to_string(), ModelCostData {
        input_tokens: 100, output_tokens: 50, cost_usd: 0.01, ..Default::default()
    });
    agg.add(&RunCostData { model_usage: usage1, total_cost_usd: 0.01, ..Default::default() });

    let mut usage2 = HashMap::new();
    usage2.insert("opus".to_string(), ModelCostData {
        input_tokens: 200, output_tokens: 100, cost_usd: 0.02, ..Default::default()
    });
    usage2.insert("sonnet".to_string(), ModelCostData {
        input_tokens: 50, output_tokens: 25, cost_usd: 0.005, ..Default::default()
    });
    agg.add(&RunCostData { model_usage: usage2, total_cost_usd: 0.025, ..Default::default() });

    assert_eq!(agg.model_totals.len(), 2);
    assert_eq!(agg.model_totals["opus"].input_tokens, 300);
    assert_eq!(agg.model_totals["sonnet"].input_tokens, 50);
    assert!((agg.model_totals["opus"].cost_usd - 0.03).abs() < 0.0001);
}

#[test]
fn aggregated_cost_default_is_zero() {
    let agg = AggregatedCost::default();
    assert_eq!(agg.run_count, 0);
    assert_eq!(agg.estimated_count, 0);
    assert_eq!(agg.total_cost_usd, 0.0);
    assert!(agg.model_totals.is_empty());
}

#[test]
fn cost_data_deserializes_without_model_usage() {
    let json = r#"{"inputTokens":100,"outputTokens":50,"cacheReadTokens":0,"cacheCreationTokens":0,"totalCostUsd":0.01,"isEstimated":false}"#;
    let cost: RunCostData = serde_json::from_str(json).unwrap();
    assert_eq!(cost.input_tokens, 100);
    assert!(cost.model_usage.is_empty());
}

#[test]
fn aggregated_cost_serialization_roundtrip() {
    let mut agg = AggregatedCost::default();
    agg.add(&RunCostData {
        input_tokens: 500, output_tokens: 250, total_cost_usd: 0.05,
        is_estimated: true, ..Default::default()
    });
    let json = serde_json::to_string(&agg).unwrap();
    let parsed: AggregatedCost = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.run_count, 1);
    assert_eq!(parsed.estimated_count, 1);
    assert_eq!(parsed.total_input_tokens, 500);
}

// ── extract_or_estimate_cost_by_agent (registry dispatch) ─────

fn make_test_registry() -> crate::agents::registry::AgentRegistry {
    use crate::agents::claude::provider::ClaudeProvider;
    use crate::agents::cursor::provider::CursorProvider;
    use std::sync::Arc;

    let mut registry = crate::agents::registry::AgentRegistry::new();
    registry.register(Arc::new(ClaudeProvider::new()));
    registry.register(Arc::new(CursorProvider::new()));
    registry
}

#[test]
fn cost_by_agent_claude_parses_stream_json() {
    let registry = make_test_registry();
    let stream = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.01}}"#;
    let cost = extract_or_estimate_cost_by_agent(&registry, "claude", stream, "opus-4.6", 5.0).unwrap();
    assert!(!cost.is_estimated);
    assert_eq!(cost.input_tokens, 100);
}

#[test]
fn cost_by_agent_cursor_estimates() {
    let registry = make_test_registry();
    let cost = extract_or_estimate_cost_by_agent(&registry, "cursor", "some output", "opus-4.6", 5.0).unwrap();
    assert!(cost.is_estimated);
}

#[test]
fn cost_by_agent_unknown_falls_back_to_estimation() {
    let registry = make_test_registry();
    let cost = extract_or_estimate_cost_by_agent(&registry, "windsurf", "output", "opus-4.6", 5.0).unwrap();
    assert!(cost.is_estimated);
    assert!(cost.total_cost_usd > 0.0);
}

#[test]
fn cost_by_agent_unknown_empty_returns_none() {
    let registry = make_test_registry();
    let cost = extract_or_estimate_cost_by_agent(&registry, "windsurf", "", "opus-4.6", 0.0);
    assert!(cost.is_none());
}

#[test]
fn cost_by_agent_claude_no_result_falls_back_to_estimation() {
    let registry = make_test_registry();
    // Claude stream without a result line -- provider.extract_cost returns estimated fallback
    let no_result_stream = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"hello"}}}"#;
    let cost = extract_or_estimate_cost_by_agent(&registry, "claude", no_result_stream, "opus-4.6", 5.0).unwrap();
    assert!(cost.is_estimated);
}

#[test]
fn cost_by_agent_empty_registry_falls_back() {
    let registry = crate::agents::registry::AgentRegistry::new();
    // Unknown agent with non-empty output should still estimate
    let cost = extract_or_estimate_cost_by_agent(&registry, "claude", "output", "opus-4.6", 5.0).unwrap();
    assert!(cost.is_estimated);
    assert!(cost.total_cost_usd > 0.0);
}

#[test]
fn cost_by_agent_empty_registry_empty_output_returns_none() {
    let registry = crate::agents::registry::AgentRegistry::new();
    let cost = extract_or_estimate_cost_by_agent(&registry, "claude", "", "opus-4.6", 0.0);
    assert!(cost.is_none());
}
