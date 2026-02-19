//! Tests for `RunCostData::zero_out_costs` and aggregation with local/zero-cost runs.

use std::collections::HashMap;

use super::{AggregatedCost, ModelCostData, RunCostData};

#[test]
fn zero_out_costs_zeroes_usd_preserves_tokens() {
    let mut cost = RunCostData {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: 200,
        cache_creation_tokens: 100,
        total_cost_usd: 0.05,
        model_usage: {
            let mut m = HashMap::new();
            m.insert("my-model".to_string(), ModelCostData {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: 200,
                cache_creation_tokens: 100,
                cost_usd: 0.05,
            });
            m
        },
        is_estimated: false,
    };

    cost.zero_out_costs();

    assert_eq!(cost.total_cost_usd, 0.0);
    assert_eq!(cost.input_tokens, 1000);
    assert_eq!(cost.output_tokens, 500);
    assert_eq!(cost.cache_read_tokens, 200);
    assert_eq!(cost.cache_creation_tokens, 100);
    assert!(!cost.is_estimated);
    let model = cost.model_usage.get("my-model").unwrap();
    assert_eq!(model.cost_usd, 0.0);
    assert_eq!(model.input_tokens, 1000);
    assert_eq!(model.output_tokens, 500);
}

#[test]
fn zero_out_costs_handles_multiple_models() {
    let mut cost = RunCostData {
        input_tokens: 300,
        output_tokens: 150,
        total_cost_usd: 0.10,
        model_usage: {
            let mut m = HashMap::new();
            m.insert("model-a".to_string(), ModelCostData {
                input_tokens: 100,
                output_tokens: 50,
                cost_usd: 0.03,
                ..Default::default()
            });
            m.insert("model-b".to_string(), ModelCostData {
                input_tokens: 200,
                output_tokens: 100,
                cost_usd: 0.07,
                ..Default::default()
            });
            m
        },
        ..Default::default()
    };

    cost.zero_out_costs();

    assert_eq!(cost.total_cost_usd, 0.0);
    assert_eq!(cost.model_usage["model-a"].cost_usd, 0.0);
    assert_eq!(cost.model_usage["model-b"].cost_usd, 0.0);
    assert_eq!(cost.model_usage["model-a"].input_tokens, 100);
    assert_eq!(cost.model_usage["model-b"].input_tokens, 200);
}

#[test]
fn zero_out_costs_noop_when_already_zero() {
    let mut cost = RunCostData::default();
    cost.zero_out_costs();
    assert_eq!(cost.total_cost_usd, 0.0);
}

#[test]
fn aggregated_cost_add_with_zero_cost_local_run() {
    let mut agg = AggregatedCost::default();

    let mut local_run = RunCostData {
        input_tokens: 1000,
        output_tokens: 500,
        cache_read_tokens: 200,
        cache_creation_tokens: 0,
        total_cost_usd: 0.05,
        model_usage: {
            let mut m = HashMap::new();
            m.insert("llama3.2".to_string(), ModelCostData {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: 200,
                cache_creation_tokens: 0,
                cost_usd: 0.05,
            });
            m
        },
        is_estimated: false,
    };
    local_run.zero_out_costs();

    agg.add(&local_run);

    assert_eq!(agg.total_cost_usd, 0.0);
    assert_eq!(agg.total_input_tokens, 1000);
    assert_eq!(agg.total_output_tokens, 500);
    assert_eq!(agg.run_count, 1);
    let model = agg.model_totals.get("llama3.2").unwrap();
    assert_eq!(model.cost_usd, 0.0);
    assert_eq!(model.input_tokens, 1000);
}

#[test]
fn aggregated_cost_mixes_local_and_api_runs() {
    let mut agg = AggregatedCost::default();

    let api_run = RunCostData {
        input_tokens: 500,
        output_tokens: 100,
        total_cost_usd: 0.02,
        model_usage: {
            let mut m = HashMap::new();
            m.insert("opus-4.6".to_string(), ModelCostData {
                input_tokens: 500,
                output_tokens: 100,
                cost_usd: 0.02,
                ..Default::default()
            });
            m
        },
        is_estimated: false,
        ..Default::default()
    };
    agg.add(&api_run);

    let mut local_run = RunCostData {
        input_tokens: 2000,
        output_tokens: 800,
        total_cost_usd: 0.10,
        model_usage: {
            let mut m = HashMap::new();
            m.insert("llama3.2".to_string(), ModelCostData {
                input_tokens: 2000,
                output_tokens: 800,
                cost_usd: 0.10,
                ..Default::default()
            });
            m
        },
        is_estimated: false,
        ..Default::default()
    };
    local_run.zero_out_costs();
    agg.add(&local_run);

    assert_eq!(agg.run_count, 2);
    assert_eq!(agg.total_input_tokens, 2500);
    assert_eq!(agg.total_output_tokens, 900);
    assert!(
        (agg.total_cost_usd - 0.02).abs() < 0.001,
        "total should only reflect the API run cost, got {}",
        agg.total_cost_usd,
    );
    assert_eq!(agg.model_totals["opus-4.6"].cost_usd, 0.02);
    assert_eq!(agg.model_totals["llama3.2"].cost_usd, 0.0);
    assert_eq!(agg.model_totals["llama3.2"].input_tokens, 2000);
}
