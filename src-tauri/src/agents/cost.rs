//! Cost extraction and estimation for agent runs.
//!
//! - Claude Code CLI: parses token usage and cost from stream-json `result` messages.
//! - Cursor CLI: estimates cost based on model pricing tables (no native cost data available).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Normalize a model name to a canonical short form.
///
/// The Claude API reports model names like `claude-opus-4-6`, while the
/// ticket/config layer uses short names like `opus-4.6`. Without normalization,
/// `model_totals` ends up with duplicate entries for the same model. This
/// function maps every known variant to the canonical short form so that costs
/// are always aggregated under a single key.
pub fn normalize_model_name(name: &str) -> String {
    let lower = name.to_lowercase().replace('_', "-");
    match lower.as_str() {
        "claude-opus-4-6" | "claude-opus-4.6" => "opus-4.6".to_string(),
        "claude-opus-4-5" | "claude-opus-4.5" => "opus-4.5".to_string(),
        "claude-sonnet-4-5" | "claude-sonnet-4.5" => "sonnet-4.5".to_string(),
        "claude-haiku-3" | "claude-haiku-3-5" | "claude-haiku-3.5" => "haiku-3.5".to_string(),
        _ => {
            // Strip leading "claude-" prefix for any unknown model to avoid
            // "claude-X" vs "X" duplicates.
            let stripped = lower.strip_prefix("claude-").unwrap_or(&lower);
            stripped.to_string()
        }
    }
}

/// Cost and token usage data for a single agent run.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RunCostData {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_cost_usd: f64,
    /// Per-model breakdown (model name -> cost data)
    #[serde(default)]
    pub model_usage: HashMap<String, ModelCostData>,
    /// Whether the cost is an estimate (true for Cursor) or authoritative (false for Claude)
    pub is_estimated: bool,
}

/// Per-model cost breakdown.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelCostData {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cost_usd: f64,
}

/// Aggregated cost across multiple runs (for ticket or board summaries).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedCost {
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub run_count: u32,
    pub estimated_count: u32,
    /// Per-model totals across all runs
    #[serde(default)]
    pub model_totals: HashMap<String, ModelCostData>,
}

impl AggregatedCost {
    /// Add a run's cost data to the aggregate.
    pub fn add(&mut self, cost: &RunCostData) {
        self.total_cost_usd += cost.total_cost_usd;
        self.total_input_tokens += cost.input_tokens;
        self.total_output_tokens += cost.output_tokens;
        self.total_cache_read_tokens += cost.cache_read_tokens;
        self.total_cache_creation_tokens += cost.cache_creation_tokens;
        self.run_count += 1;
        if cost.is_estimated {
            self.estimated_count += 1;
        }

        for (model, data) in &cost.model_usage {
            // Normalize during aggregation so that legacy data stored with
            // non-canonical names (e.g. "claude-opus-4-6") is merged correctly.
            let canonical = normalize_model_name(model);
            let entry = self.model_totals.entry(canonical).or_default();
            entry.input_tokens += data.input_tokens;
            entry.output_tokens += data.output_tokens;
            entry.cache_read_tokens += data.cache_read_tokens;
            entry.cache_creation_tokens += data.cache_creation_tokens;
            entry.cost_usd += data.cost_usd;
        }
    }
}

/// Extract cost/usage data from Claude Code's stream-json output.
///
/// The `result` type JSON line contains usage and modelUsage fields:
/// ```json
/// {"type":"result","result":"text...","usage":{"input_tokens":1234,"output_tokens":5678,
///   "cache_read_input_tokens":900,"cache_creation_input_tokens":200,"total_cost_usd":0.12},
///  "modelUsage":{"claude-opus-4-6":{"inputTokens":1234,"outputTokens":5678,"costUSD":0.12}}}
/// ```
pub fn extract_cost_from_stream_json(stream_output: &str) -> Option<RunCostData> {
    for line in stream_output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }

        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            if json.get("type").and_then(|t| t.as_str()) == Some("result") {
                return parse_cost_from_result_json(&json);
            }
        }
    }
    None
}

/// Parse cost data from a `result` type JSON object.
fn parse_cost_from_result_json(json: &serde_json::Value) -> Option<RunCostData> {
    let usage = json.get("usage")?;

    let input_tokens = usage
        .get("input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache_read_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let cache_creation_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let total_cost_usd = usage
        .get("total_cost_usd")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);

    let mut model_usage = HashMap::new();
    if let Some(model_usage_json) = json.get("modelUsage").and_then(|v| v.as_object()) {
        for (model_name, model_data) in model_usage_json {
            let data = ModelCostData {
                input_tokens: model_data
                    .get("inputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                output_tokens: model_data
                    .get("outputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                cache_read_tokens: model_data
                    .get("cacheReadInputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                cache_creation_tokens: model_data
                    .get("cacheCreationInputTokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0),
                cost_usd: model_data
                    .get("costUSD")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0),
            };
            // Normalize model name so that e.g. "claude-opus-4-6" and "opus-4.6"
            // are merged under the same key.
            let canonical = normalize_model_name(model_name);
            let entry = model_usage.entry(canonical).or_insert_with(ModelCostData::default);
            entry.input_tokens += data.input_tokens;
            entry.output_tokens += data.output_tokens;
            entry.cache_read_tokens += data.cache_read_tokens;
            entry.cache_creation_tokens += data.cache_creation_tokens;
            entry.cost_usd += data.cost_usd;
        }
    }

    if input_tokens == 0
        && output_tokens == 0
        && cache_read_tokens == 0
        && cache_creation_tokens == 0
        && total_cost_usd == 0.0
    {
        return None;
    }

    // Ensure total_cost_usd is at least the sum of per-model costs.
    // The API's usage.total_cost_usd and modelUsage.*.costUSD are independent
    // fields; the total can sometimes lag behind the model-level breakdown.
    let model_cost_sum: f64 = model_usage.values().map(|d| d.cost_usd).sum();
    let total_cost_usd = total_cost_usd.max(model_cost_sum);

    Some(RunCostData {
        input_tokens,
        output_tokens,
        cache_read_tokens,
        cache_creation_tokens,
        total_cost_usd,
        model_usage,
        is_estimated: false,
    })
}

/// Model pricing per million tokens (USD).
struct ModelPricing {
    input_per_mtok: f64,
    output_per_mtok: f64,
}

/// Get pricing for a model. Falls back to Sonnet pricing if unknown.
fn get_model_pricing(model: &str) -> ModelPricing {
    let normalized = model.to_lowercase().replace(['-', '_'], " ");

    if normalized.contains("opus") {
        ModelPricing {
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
        }
    } else if normalized.contains("sonnet") {
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        }
    } else if normalized.contains("haiku") {
        ModelPricing {
            input_per_mtok: 0.25,
            output_per_mtok: 1.25,
        }
    } else {
        // Default to Sonnet pricing
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        }
    }
}

/// Estimate cost for a Cursor run based on model and output size.
///
/// This is a rough estimate since Cursor does not expose token usage data.
/// Uses ~4 characters per token as an approximation.
pub fn estimate_cost(model: &str, output_chars: usize, duration_secs: f64) -> RunCostData {
    let pricing = get_model_pricing(model);

    // ~4 chars/token, ~500 input tokens/second heuristics
    let estimated_output_tokens = (output_chars as f64 / 4.0) as u64;
    let estimated_input_tokens = (duration_secs * 500.0) as u64;

    let input_cost = estimated_input_tokens as f64 * pricing.input_per_mtok / 1_000_000.0;
    let output_cost = estimated_output_tokens as f64 * pricing.output_per_mtok / 1_000_000.0;
    let total_cost = input_cost + output_cost;

    let model_name = normalize_model_name(model);
    let mut model_usage = HashMap::new();
    model_usage.insert(
        model_name,
        ModelCostData {
            input_tokens: estimated_input_tokens,
            output_tokens: estimated_output_tokens,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
            cost_usd: total_cost,
        },
    );

    RunCostData {
        input_tokens: estimated_input_tokens,
        output_tokens: estimated_output_tokens,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
        total_cost_usd: total_cost,
        model_usage,
        is_estimated: true,
    }
}

/// Extract cost from agent output, trying Claude parsing first, then falling back to estimation.
pub fn extract_or_estimate_cost(
    stdout: &str,
    model: &str,
    duration_secs: f64,
    is_claude: bool,
) -> Option<RunCostData> {
    if is_claude {
        if let Some(cost) = extract_cost_from_stream_json(stdout) {
            return Some(cost);
        }
    }

    let output_chars = stdout.len();
    if output_chars > 0 || duration_secs > 0.0 {
        Some(estimate_cost(model, output_chars, duration_secs))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Cache-hit scenario: only cache_read tokens, zero input/output/cost
        let stream_output = r#"{"type":"result","result":"","usage":{"input_tokens":0,"output_tokens":0,"cache_read_input_tokens":500,"cache_creation_input_tokens":0,"total_cost_usd":0.0}}"#;
        let cost = extract_cost_from_stream_json(stream_output).unwrap();
        assert_eq!(cost.input_tokens, 0);
        assert_eq!(cost.output_tokens, 0);
        assert_eq!(cost.cache_read_tokens, 500);
        assert!(!cost.is_estimated);
    }

    #[test]
    fn parse_cost_cache_creation_only_returns_some() {
        // Cache-write scenario: only cache_creation tokens
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
        // Model name should be normalized from "claude-opus-4-6" -> "opus-4.6"
        assert!(cost.model_usage.contains_key("opus-4.6"));
    }

    #[test]
    fn estimate_cost_sonnet() {
        let cost_opus = estimate_cost("opus-4.6", 4000, 10.0);
        let cost_sonnet = estimate_cost("sonnet-4.5", 4000, 10.0);
        // Opus should be more expensive than Sonnet
        assert!(cost_opus.total_cost_usd > cost_sonnet.total_cost_usd);
    }

    #[test]
    fn estimate_cost_zero_output() {
        let cost = estimate_cost("sonnet-4.5", 0, 5.0);
        assert!(cost.is_estimated);
        assert_eq!(cost.output_tokens, 0);
        // Should still have input tokens from duration
        assert!(cost.input_tokens > 0);
    }

    #[test]
    fn extract_or_estimate_prefers_parsed() {
        let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.01}}"#;
        let cost =
            extract_or_estimate_cost(stream_output, "opus-4.6", 10.0, true).unwrap();
        // Should use parsed data, not estimation
        assert!(!cost.is_estimated);
        assert_eq!(cost.input_tokens, 100);
    }

    #[test]
    fn extract_or_estimate_falls_back_for_cursor() {
        let plain_output = "This is plain text from Cursor agent";
        let cost = extract_or_estimate_cost(plain_output, "opus-4.6", 10.0, false).unwrap();
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
        // Model names should be normalized
        assert!(cost.model_usage.contains_key("opus-4.6"));
        assert!(cost.model_usage.contains_key("sonnet-4.5"));
        let opus = &cost.model_usage["opus-4.6"];
        let sonnet = &cost.model_usage["sonnet-4.5"];
        assert_eq!(opus.input_tokens + sonnet.input_tokens, 100);
        // total_cost_usd should be at least the sum of model costs
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
    fn extract_or_estimate_claude_no_result_falls_back() {
        // Claude stream-json with no result line -> falls back to estimation
        let stream_output = r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"text":"hello"}}}"#;
        let cost = extract_or_estimate_cost(stream_output, "opus-4.6", 5.0, true).unwrap();
        assert!(cost.is_estimated);
    }

    #[test]
    fn extract_or_estimate_empty_stdout_zero_duration_returns_none() {
        let cost = extract_or_estimate_cost("", "opus-4.6", 0.0, false);
        assert!(cost.is_none());
    }

    #[test]
    fn extract_or_estimate_empty_stdout_positive_duration() {
        let cost = extract_or_estimate_cost("", "opus-4.6", 5.0, false).unwrap();
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
    fn total_cost_corrected_when_model_costs_exceed_api_total() {
        // Simulate a case where the API's total_cost_usd only reflects one model
        // but modelUsage includes costs from multiple models.
        let stream_output = r#"{"type":"result","result":"text","usage":{"input_tokens":100,"output_tokens":50,"total_cost_usd":0.04},"modelUsage":{"claude-opus-4-6":{"inputTokens":80,"outputTokens":40,"costUSD":0.04},"claude-sonnet-4-5":{"inputTokens":20,"outputTokens":10,"costUSD":0.01}}}"#;
        let cost = extract_cost_from_stream_json(stream_output).unwrap();
        // total_cost_usd should be corrected to at least the sum of model costs
        assert!(
            (cost.total_cost_usd - 0.05).abs() < 0.0001,
            "total_cost_usd should be 0.05 (sum of model costs), got {}",
            cost.total_cost_usd
        );
    }

    #[test]
    fn estimate_cost_normalizes_model_name() {
        let cost = estimate_cost("opus-4.6", 4000, 10.0);
        assert!(cost.model_usage.contains_key("opus-4.6"));

        let cost2 = estimate_cost("claude-opus-4-6", 4000, 10.0);
        assert!(cost2.model_usage.contains_key("opus-4.6"));
        // Both should produce the same cost since they refer to the same model
        assert!((cost.total_cost_usd - cost2.total_cost_usd).abs() < 0.0001);
    }

    #[test]
    fn aggregated_cost_merges_different_name_variants() {
        let mut agg = AggregatedCost::default();

        // Run 1: model_usage uses API name "claude-opus-4-6"
        let mut usage1 = HashMap::new();
        usage1.insert("claude-opus-4-6".to_string(), ModelCostData {
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: 0.01,
            ..Default::default()
        });
        let cost1 = RunCostData {
            model_usage: usage1,
            total_cost_usd: 0.01,
            ..Default::default()
        };

        // Run 2: model_usage uses short name "opus-4.6" (from estimation)
        let mut usage2 = HashMap::new();
        usage2.insert("opus-4.6".to_string(), ModelCostData {
            input_tokens: 200,
            output_tokens: 100,
            cost_usd: 0.02,
            ..Default::default()
        });
        let cost2 = RunCostData {
            model_usage: usage2,
            total_cost_usd: 0.02,
            ..Default::default()
        };

        agg.add(&cost1);
        agg.add(&cost2);

        // Both should be merged under the canonical "opus-4.6" key
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
            input_tokens: 100,
            output_tokens: 50,
            cost_usd: 0.01,
            ..Default::default()
        });
        let cost1 = RunCostData {
            model_usage: usage1,
            total_cost_usd: 0.01,
            ..Default::default()
        };

        let mut usage2 = HashMap::new();
        usage2.insert("opus".to_string(), ModelCostData {
            input_tokens: 200,
            output_tokens: 100,
            cost_usd: 0.02,
            ..Default::default()
        });
        usage2.insert("sonnet".to_string(), ModelCostData {
            input_tokens: 50,
            output_tokens: 25,
            cost_usd: 0.005,
            ..Default::default()
        });
        let cost2 = RunCostData {
            model_usage: usage2,
            total_cost_usd: 0.025,
            ..Default::default()
        };

        agg.add(&cost1);
        agg.add(&cost2);

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
        // modelUsage has #[serde(default)] so it can be omitted
        let json = r#"{"inputTokens":100,"outputTokens":50,"cacheReadTokens":0,"cacheCreationTokens":0,"totalCostUsd":0.01,"isEstimated":false}"#;
        let cost: RunCostData = serde_json::from_str(json).unwrap();
        assert_eq!(cost.input_tokens, 100);
        assert!(cost.model_usage.is_empty());
    }

    #[test]
    fn aggregated_cost_serialization_roundtrip() {
        let mut agg = AggregatedCost::default();
        agg.add(&RunCostData {
            input_tokens: 500,
            output_tokens: 250,
            total_cost_usd: 0.05,
            is_estimated: true,
            ..Default::default()
        });
        let json = serde_json::to_string(&agg).unwrap();
        let parsed: AggregatedCost = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.run_count, 1);
        assert_eq!(parsed.estimated_count, 1);
        assert_eq!(parsed.total_input_tokens, 500);
    }
}
