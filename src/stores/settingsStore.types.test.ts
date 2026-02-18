import { describe, it, expect } from 'vitest';
import {
  mapModelForCodex,
  mapStagesForCodex,
  getDefaultConfigForAgent,
  getPresetStagesForAgent,
  WORKFLOW_PRESETS,
  MODEL_OPTIONS,
  CODEX_MODEL_OPTIONS,
  DEFAULT_WORKFLOW_PRESET,
  DEFAULT_WORKFLOW_STAGES,
  type WorkflowStages,
} from './settingsStore.types';

describe('mapModelForCodex', () => {
  it('maps opus models to gpt-5.3-codex', () => {
    expect(mapModelForCodex('opus-4.6')).toBe('gpt-5.3-codex');
    expect(mapModelForCodex('opus-4.5')).toBe('gpt-5.3-codex');
  });

  it('maps sonnet models to gpt-5.2-codex', () => {
    expect(mapModelForCodex('sonnet-4.6')).toBe('gpt-5.2-codex');
    expect(mapModelForCodex('sonnet-4.5')).toBe('gpt-5.2-codex');
  });

  it('passes through codex-native models unchanged', () => {
    expect(mapModelForCodex('gpt-5.3-codex')).toBe('gpt-5.3-codex');
    expect(mapModelForCodex('gpt-5.2-codex')).toBe('gpt-5.2-codex');
  });

  it('passes through unknown models unchanged', () => {
    expect(mapModelForCodex('custom-model')).toBe('custom-model');
    expect(mapModelForCodex('')).toBe('');
  });
});

describe('mapStagesForCodex', () => {
  it('maps all stage models to codex equivalents', () => {
    const input: WorkflowStages = {
      branchGen:   { enabled: true, model: 'sonnet-4.6' },
      plan:        { enabled: true, model: 'opus-4.6' },
      implement:   { enabled: true, model: 'opus-4.6' },
      codeReview:  { enabled: true, model: 'opus-4.5' },
      deslop:      { enabled: false, model: 'sonnet-4.6' },
      cleanup:     { enabled: true, model: 'sonnet-4.6' },
      unitTests:   { enabled: false, model: 'opus-4.5' },
      finalReview: { enabled: true, model: 'opus-4.5' },
      commit:      { enabled: true, model: 'sonnet-4.6' },
    };

    const result = mapStagesForCodex(input);

    expect(result.branchGen.model).toBe('gpt-5.2-codex');
    expect(result.plan.model).toBe('gpt-5.3-codex');
    expect(result.implement.model).toBe('gpt-5.3-codex');
    expect(result.codeReview.model).toBe('gpt-5.3-codex');
    expect(result.deslop.model).toBe('gpt-5.2-codex');
    expect(result.commit.model).toBe('gpt-5.2-codex');
  });

  it('preserves enabled/disabled state', () => {
    const input: WorkflowStages = {
      ...WORKFLOW_PRESETS.vibe.stages,
    };
    const result = mapStagesForCodex(input);
    expect(result.cleanup.enabled).toBe(false);
    expect(result.unitTests.enabled).toBe(false);
    expect(result.plan.enabled).toBe(true);
  });

  it('does not mutate the input', () => {
    const input = { ...WORKFLOW_PRESETS.balanced.stages };
    const originalPlan = input.plan.model;
    mapStagesForCodex(input);
    expect(input.plan.model).toBe(originalPlan);
  });
});

describe('getDefaultConfigForAgent', () => {
  it('returns claude config with Claude-specific settings', () => {
    const config = getDefaultConfigForAgent('claude');
    expect(config.plannerModel).toBe('opus-4.5');
    expect(config.diagnosticModel).toBe('sonnet-4.6');
    expect(config.settings).toHaveProperty('authToken');
    expect(config.settings).toHaveProperty('thinkingEnabled');
    expect(config.settings).toHaveProperty('chromeEnabled');
  });

  it('returns cursor config with thinking setting', () => {
    const config = getDefaultConfigForAgent('cursor');
    expect(config.plannerModel).toBe('opus-4.5');
    expect(config.settings).toHaveProperty('thinkingEnabled');
    expect(config.settings).not.toHaveProperty('authToken');
  });

  it('returns codex config with codex-native models', () => {
    const config = getDefaultConfigForAgent('codex');
    expect(config.plannerModel).toBe('gpt-5.3-codex');
    expect(config.validationModel).toBe('gpt-5.2-codex');
    expect(config.diagnosticModel).toBe('gpt-5.2-codex');
    expect(config.workflowStages.plan.model).toBe('gpt-5.3-codex');
    expect(config.workflowStages.branchGen.model).toBe('gpt-5.2-codex');
    expect(config.settings).toEqual({});
  });

  it('returns claude-based defaults for unknown agent', () => {
    const config = getDefaultConfigForAgent('unknown-agent');
    expect(config.plannerModel).toBe('opus-4.5');
    expect(config.settings).toEqual({});
  });

  it('returns independent copies (not shared references)', () => {
    const a = getDefaultConfigForAgent('claude');
    const b = getDefaultConfigForAgent('claude');
    a.plannerModel = 'modified';
    a.settings.authToken = 'modified';
    a.workflowStages.plan.model = 'modified';
    expect(b.plannerModel).toBe('opus-4.5');
    expect(b.settings.authToken).toBe('');
    expect(b.workflowStages.plan.model).toBe('opus-4.6');
  });
});

describe('getPresetStagesForAgent', () => {
  it('returns unmodified stages for claude', () => {
    const stages = getPresetStagesForAgent('balanced', 'claude');
    expect(stages.plan.model).toBe('opus-4.6');
    expect(stages.branchGen.model).toBe('sonnet-4.6');
  });

  it('returns unmodified stages for cursor', () => {
    const stages = getPresetStagesForAgent('comprehensive', 'cursor');
    expect(stages.plan.model).toBe('opus-4.6');
  });

  it('maps stages to codex models for codex agent', () => {
    const stages = getPresetStagesForAgent('balanced', 'codex');
    expect(stages.plan.model).toBe('gpt-5.3-codex');
    expect(stages.branchGen.model).toBe('gpt-5.2-codex');
    expect(stages.deslop.model).toBe('gpt-5.3-codex');
    expect(stages.cleanup.model).toBe('gpt-5.2-codex');
  });

  it('preserves enabled/disabled state for codex', () => {
    const stages = getPresetStagesForAgent('quick-fix', 'codex');
    expect(stages.codeReview.enabled).toBe(false);
    expect(stages.deslop.enabled).toBe(false);
    expect(stages.plan.enabled).toBe(true);
    expect(stages.commit.enabled).toBe(true);
  });

  it.each(['comprehensive', 'balanced', 'vibe', 'standard', 'quick-fix', 'fastest'] as const)(
    'all preset keys work for codex: %s',
    (preset) => {
      const stages = getPresetStagesForAgent(preset, 'codex');
      for (const key of Object.keys(stages) as (keyof WorkflowStages)[]) {
        expect(stages[key].model).toMatch(/^gpt-5\.\d-codex$/);
      }
    },
  );
});

describe('constants', () => {
  it('MODEL_OPTIONS has 4 Claude/Anthropic models', () => {
    expect(MODEL_OPTIONS).toHaveLength(4);
    expect(MODEL_OPTIONS.map((o) => o.value)).toEqual(['opus-4.6', 'opus-4.5', 'sonnet-4.6', 'sonnet-4.5']);
  });

  it('CODEX_MODEL_OPTIONS has 2 GPT models', () => {
    expect(CODEX_MODEL_OPTIONS).toHaveLength(2);
    expect(CODEX_MODEL_OPTIONS.map((o) => o.value)).toEqual(['gpt-5.3-codex', 'gpt-5.2-codex']);
  });

  it('DEFAULT_WORKFLOW_PRESET is balanced', () => {
    expect(DEFAULT_WORKFLOW_PRESET).toBe('balanced');
  });

  it('DEFAULT_WORKFLOW_STAGES matches balanced preset', () => {
    expect(DEFAULT_WORKFLOW_STAGES).toEqual(WORKFLOW_PRESETS.balanced.stages);
  });
});
