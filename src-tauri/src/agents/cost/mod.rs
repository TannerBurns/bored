//! Cost extraction and estimation for agent runs.

mod estimation;
mod extraction;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_zero_cost;

pub use estimation::{compute_cost_from_tokens, estimate_cost};
pub use extraction::extract_cost_from_stream_json;

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
        "claude-sonnet-4-6" | "claude-sonnet-4.6" => "sonnet-4.6".to_string(),
        "claude-sonnet-4-5" | "claude-sonnet-4.5" => "sonnet-4.5".to_string(),
        "claude-haiku-3" | "claude-haiku-3-5" | "claude-haiku-3.5" => "haiku-3.5".to_string(),
        "gpt-5.4" | "gpt-5-4" => "gpt-5.4".to_string(),
        "gpt-5.3-codex" | "gpt-5-3-codex" => "gpt-5.3-codex".to_string(),
        "gpt-5.2-codex" | "gpt-5-2-codex" => "gpt-5.2-codex".to_string(),
        _ => {
            // Strip "claude-" prefix to avoid "claude-X" vs "X" duplicates.
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

impl RunCostData {
    /// Zero out all USD cost fields while preserving token counts.
    ///
    /// Used for local/self-hosted provider runs where token usage is still
    /// meaningful but there is no per-token API charge.
    pub fn zero_out_costs(&mut self) {
        self.total_cost_usd = 0.0;
        for data in self.model_usage.values_mut() {
            data.cost_usd = 0.0;
        }
    }
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
    ///
    /// `total_cost_usd` is always derived from `model_totals` so the two
    /// can never diverge.  Legacy runs without a per-model breakdown are
    /// attributed to an `"other"` bucket.
    pub fn add(&mut self, cost: &RunCostData) {
        self.total_input_tokens += cost.input_tokens;
        self.total_output_tokens += cost.output_tokens;
        self.total_cache_read_tokens += cost.cache_read_tokens;
        self.total_cache_creation_tokens += cost.cache_creation_tokens;
        self.run_count += 1;
        if cost.is_estimated {
            self.estimated_count += 1;
        }

        if cost.model_usage.is_empty() {
            if cost.total_cost_usd > 0.0 || cost.input_tokens > 0 || cost.output_tokens > 0
                || cost.cache_read_tokens > 0 || cost.cache_creation_tokens > 0 {
                let entry = self.model_totals.entry("other".to_string()).or_default();
                entry.input_tokens += cost.input_tokens;
                entry.output_tokens += cost.output_tokens;
                entry.cache_read_tokens += cost.cache_read_tokens;
                entry.cache_creation_tokens += cost.cache_creation_tokens;
                entry.cost_usd += cost.total_cost_usd;
            }
        } else {
            for (model, data) in &cost.model_usage {
                let canonical = normalize_model_name(model);
                let entry = self.model_totals.entry(canonical).or_default();
                entry.input_tokens += data.input_tokens;
                entry.output_tokens += data.output_tokens;
                entry.cache_read_tokens += data.cache_read_tokens;
                entry.cache_creation_tokens += data.cache_creation_tokens;
                entry.cost_usd += data.cost_usd;
            }
        }

        self.total_cost_usd = self.model_totals.values().map(|d| d.cost_usd).sum();
    }
}

/// Extract cost by agent ID, looking up the provider in the registry.
///
/// Falls back to estimation when the agent is unknown (uses simple heuristic).
pub fn extract_or_estimate_cost_by_agent(
    registry: &super::registry::AgentRegistry,
    agent_type: &str,
    stdout: &str,
    model: &str,
    duration_secs: f64,
) -> Option<RunCostData> {
    if let Some(provider) = registry.get(agent_type) {
        provider.extract_cost(stdout, model, duration_secs)
    } else {
        let output_chars = stdout.len();
        if output_chars > 0 || duration_secs > 0.0 {
            Some(estimate_cost(model, output_chars, duration_secs))
        } else {
            None
        }
    }
}
