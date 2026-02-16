//! Cost estimation for agents without native cost reporting (e.g. Cursor).

use std::collections::HashMap;

use super::{normalize_model_name, ModelCostData, RunCostData};

/// Model pricing per million tokens (USD).
///
/// Rates sourced from the Anthropic pricing page:
/// <https://docs.anthropic.com/en/docs/about-claude/pricing>
struct ModelPricing {
    input_per_mtok: f64,
    output_per_mtok: f64,
    /// Prompt-cache read price (0.1x base input).
    cache_read_per_mtok: f64,
    /// Prompt-cache write price (5-min TTL = 1.25x base input).
    cache_write_per_mtok: f64,
}

/// Get pricing for a model. Falls back to Sonnet pricing if unknown.
///
/// Pricing as of 2026 (Claude 4.x generation):
///   Opus  4.6 / 4.5:  $5  input, $25 output, $0.50 cache-read, $6.25 cache-write
///   Sonnet 4.5 / 4  :  $3  input, $15 output, $0.30 cache-read, $3.75 cache-write
///   Haiku  4.5       :  $1  input, $5  output, $0.10 cache-read, $1.25 cache-write
fn get_model_pricing(model: &str) -> ModelPricing {
    let normalized = model.to_lowercase().replace(['-', '_'], " ");

    if normalized.contains("opus") {
        ModelPricing {
            input_per_mtok: 5.0,
            output_per_mtok: 25.0,
            cache_read_per_mtok: 0.50,
            cache_write_per_mtok: 6.25,
        }
    } else if normalized.contains("sonnet") {
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            cache_read_per_mtok: 0.30,
            cache_write_per_mtok: 3.75,
        }
    } else if normalized.contains("haiku") {
        ModelPricing {
            input_per_mtok: 1.0,
            output_per_mtok: 5.0,
            cache_read_per_mtok: 0.10,
            cache_write_per_mtok: 1.25,
        }
    } else {
        ModelPricing {
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
            cache_read_per_mtok: 0.30,
            cache_write_per_mtok: 3.75,
        }
    }
}

/// Compute the cost for a set of token counts using the pricing table.
///
/// This accounts for input, output, cache-read and cache-write tokens at
/// their respective rates from the Anthropic pricing page.  Useful for
/// verifying that an API-reported total matches the token breakdown.
pub fn compute_cost_from_tokens(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
    cache_creation_tokens: u64,
) -> f64 {
    let pricing = get_model_pricing(model);
    let input_cost = input_tokens as f64 * pricing.input_per_mtok / 1_000_000.0;
    let output_cost = output_tokens as f64 * pricing.output_per_mtok / 1_000_000.0;
    let cache_read_cost = cache_read_tokens as f64 * pricing.cache_read_per_mtok / 1_000_000.0;
    let cache_write_cost =
        cache_creation_tokens as f64 * pricing.cache_write_per_mtok / 1_000_000.0;
    input_cost + output_cost + cache_read_cost + cache_write_cost
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
