import { describe, it, expect } from 'vitest';
import {
  mapModelForCodex,
  mapStagesForCodex,
  getDefaultConfigForAgent,
  validateStageOrder,
  expandStageKey,
  buildFullExecutionOrder,
  MODEL_OPTIONS,
  CODEX_MODEL_OPTIONS,
  DEFAULT_WORKFLOW_STAGES,
  DEFAULT_STAGE_ORDER,
  REQUIRED_STAGE_KEYS,
  RESERVED_INTERNAL_STAGE_IDS,
  BUILTIN_CATALOG_COMMANDS,
  WORKFLOW_STAGE_INFO,
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
      branchGen:         { enabled: true, model: 'sonnet-4.6' },
      plan:              { enabled: true, model: 'opus-4.6' },
      implement:         { enabled: true, model: 'opus-4.6' },
      'code-review':     { enabled: true, model: 'opus-4.5' },
      deslop:            { enabled: false, model: 'sonnet-4.6' },
      cleanup:           { enabled: true, model: 'sonnet-4.6' },
      'unit-tests':      { enabled: false, model: 'opus-4.5' },
      'review-changes':  { enabled: true, model: 'opus-4.5' },
      commit:            { enabled: true, model: 'sonnet-4.6' },
    };

    const result = mapStagesForCodex(input);

    expect(result.branchGen.model).toBe('gpt-5.2-codex');
    expect(result.plan.model).toBe('gpt-5.3-codex');
    expect(result.implement.model).toBe('gpt-5.3-codex');
    expect(result['code-review'].model).toBe('gpt-5.3-codex');
    expect(result.deslop.model).toBe('gpt-5.2-codex');
    expect(result.commit.model).toBe('gpt-5.2-codex');
  });

  it('preserves enabled/disabled state', () => {
    const result = mapStagesForCodex(DEFAULT_WORKFLOW_STAGES);
    expect(result.cleanup.enabled).toBe(true);
    expect(result.plan.enabled).toBe(true);
  });

  it('does not mutate the input', () => {
    const input = { ...DEFAULT_WORKFLOW_STAGES };
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
    expect(config.settings).toEqual({ ossEnabled: false, localProvider: 'ollama', modelOverride: '' });
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

  it('does not include workflowPreset in configs', () => {
    const config = getDefaultConfigForAgent('claude');
    expect(config).not.toHaveProperty('workflowPreset');
  });
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

  it('DEFAULT_STAGE_ORDER contains all 9 stage keys', () => {
    expect(DEFAULT_STAGE_ORDER).toHaveLength(9);
    expect(DEFAULT_STAGE_ORDER[0]).toBe('branchGen');
    expect(DEFAULT_STAGE_ORDER[1]).toBe('plan');
    expect(DEFAULT_STAGE_ORDER[2]).toBe('implement');
    expect(DEFAULT_STAGE_ORDER[DEFAULT_STAGE_ORDER.length - 1]).toBe('commit');
  });

  it('DEFAULT_STAGE_ORDER has required stages pinned at start and end', () => {
    const requiredStart = DEFAULT_STAGE_ORDER.slice(0, 3);
    expect(requiredStart).toEqual(['branchGen', 'plan', 'implement']);
    expect(DEFAULT_STAGE_ORDER[DEFAULT_STAGE_ORDER.length - 1]).toBe('commit');
  });

  it('REQUIRED_STAGE_KEYS has exactly 4 required stages', () => {
    expect(REQUIRED_STAGE_KEYS.size).toBe(4);
    expect(REQUIRED_STAGE_KEYS.has('branchGen')).toBe(true);
    expect(REQUIRED_STAGE_KEYS.has('plan')).toBe(true);
    expect(REQUIRED_STAGE_KEYS.has('implement')).toBe(true);
    expect(REQUIRED_STAGE_KEYS.has('commit')).toBe(true);
  });

  it('REQUIRED_STAGE_KEYS does not contain command IDs', () => {
    expect(REQUIRED_STAGE_KEYS.has('code-review')).toBe(false);
    expect(REQUIRED_STAGE_KEYS.has('cleanup')).toBe(false);
    expect(REQUIRED_STAGE_KEYS.has('deslop')).toBe(false);
  });

  it('RESERVED_INTERNAL_STAGE_IDS contains all expanded-only backend names', () => {
    expect(RESERVED_INTERNAL_STAGE_IDS.has('branch-gen')).toBe(true);
    expect(RESERVED_INTERNAL_STAGE_IDS.has('branch')).toBe(true);
    expect(RESERVED_INTERNAL_STAGE_IDS.has('plan-validation')).toBe(true);
    expect(RESERVED_INTERNAL_STAGE_IDS.has('code-review-fix')).toBe(true);
    expect(RESERVED_INTERNAL_STAGE_IDS.has('add-and-commit')).toBe(true);
  });

  it('RESERVED_INTERNAL_STAGE_IDS does not overlap with REQUIRED_STAGE_KEYS', () => {
    for (const id of RESERVED_INTERNAL_STAGE_IDS) {
      expect(REQUIRED_STAGE_KEYS.has(id)).toBe(false);
    }
  });

  it('RESERVED_INTERNAL_STAGE_IDS does not contain catalog command IDs', () => {
    for (const cmd of BUILTIN_CATALOG_COMMANDS) {
      expect(RESERVED_INTERNAL_STAGE_IDS.has(cmd.id)).toBe(false);
    }
  });

  it('WORKFLOW_STAGE_INFO has correct required flags', () => {
    const required = WORKFLOW_STAGE_INFO.filter((s) => s.required).map((s) => s.key);
    expect(required).toEqual(['branchGen', 'plan', 'implement', 'commit']);
  });

  it('WORKFLOW_STAGE_INFO only contains required stages', () => {
    expect(WORKFLOW_STAGE_INFO).toHaveLength(4);
    for (const info of WORKFLOW_STAGE_INFO) {
      expect(info.required).toBe(true);
    }
  });

  it('DEFAULT_WORKFLOW_STAGES has entries for all stages in DEFAULT_STAGE_ORDER', () => {
    for (const key of DEFAULT_STAGE_ORDER) {
      expect(DEFAULT_WORKFLOW_STAGES[key]).toBeDefined();
      expect(typeof DEFAULT_WORKFLOW_STAGES[key].enabled).toBe('boolean');
    }
  });

  it('DEFAULT_WORKFLOW_STAGES uses kebab-case for command keys', () => {
    expect(DEFAULT_WORKFLOW_STAGES['code-review']).toBeDefined();
    expect(DEFAULT_WORKFLOW_STAGES['unit-tests']).toBeDefined();
    expect(DEFAULT_WORKFLOW_STAGES['review-changes']).toBeDefined();
    expect(DEFAULT_WORKFLOW_STAGES['codeReview' as string]).toBeUndefined();
    expect(DEFAULT_WORKFLOW_STAGES['unitTests' as string]).toBeUndefined();
    expect(DEFAULT_WORKFLOW_STAGES['finalReview' as string]).toBeUndefined();
  });

  it('DEFAULT_STAGE_ORDER uses kebab-case for command keys', () => {
    expect(DEFAULT_STAGE_ORDER).toContain('code-review');
    expect(DEFAULT_STAGE_ORDER).toContain('unit-tests');
    expect(DEFAULT_STAGE_ORDER).toContain('review-changes');
    expect(DEFAULT_STAGE_ORDER).not.toContain('codeReview');
    expect(DEFAULT_STAGE_ORDER).not.toContain('unitTests');
    expect(DEFAULT_STAGE_ORDER).not.toContain('finalReview');
  });
});

describe('BUILTIN_CATALOG_COMMANDS', () => {
  it('has expected builtin commands', () => {
    const ids = BUILTIN_CATALOG_COMMANDS.map((c) => c.id);
    expect(ids).toContain('code-review');
    expect(ids).toContain('cleanup');
    expect(ids).toContain('unit-tests');
    expect(ids).toContain('review-changes');
    expect(ids).toContain('deslop');
    expect(ids).toContain('add-tests');
    expect(ids).toContain('fix-lint');
    expect(ids).toContain('sync-with-main');
    expect(ids).toContain('review-polish');
    expect(ids).toContain('patch-security');
    expect(ids).toContain('api-contract-check');
    expect(ids).toContain('observability-pass');
    expect(ids).toContain('integration-test');
  });

  it('all builtins have source "builtin"', () => {
    for (const cmd of BUILTIN_CATALOG_COMMANDS) {
      expect(cmd.source).toBe('builtin');
    }
  });

  it('all builtins have a filename', () => {
    for (const cmd of BUILTIN_CATALOG_COMMANDS) {
      expect(cmd.filename).toMatch(/\.md$/);
    }
  });

  it('some builtins are enabled by default', () => {
    const enabled = BUILTIN_CATALOG_COMMANDS.filter((c) => c.enabled);
    expect(enabled.length).toBeGreaterThan(0);
    expect(enabled.map((c) => c.id)).toContain('code-review');
    expect(enabled.map((c) => c.id)).toContain('cleanup');
  });

  it('new commands are disabled by default', () => {
    const newCmds = BUILTIN_CATALOG_COMMANDS.filter((c) =>
      ['add-tests', 'fix-lint', 'sync-with-main', 'review-polish', 'patch-security', 'api-contract-check', 'observability-pass', 'integration-test'].includes(c.id)
    );
    for (const cmd of newCmds) {
      expect(cmd.enabled).toBe(false);
    }
  });

  it('each builtin has filename matching id', () => {
    for (const cmd of BUILTIN_CATALOG_COMMANDS) {
      expect(cmd.filename).toBe(`${cmd.id}.md`);
    }
  });

  it('has no duplicate IDs', () => {
    const ids = BUILTIN_CATALOG_COMMANDS.map((c) => c.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe('validateStageOrder', () => {
  it('accepts valid default order', () => {
    expect(validateStageOrder(DEFAULT_STAGE_ORDER)).toBe(true);
  });

  it('rejects empty order', () => {
    expect(validateStageOrder([])).toBe(false);
  });

  it('rejects order not starting with branchGen', () => {
    expect(validateStageOrder(['plan', 'implement', 'commit'])).toBe(false);
  });

  it('rejects order not ending with commit', () => {
    expect(validateStageOrder(['branchGen', 'plan', 'implement'])).toBe(false);
  });

  it('rejects order with plan after implement', () => {
    expect(validateStageOrder(['branchGen', 'implement', 'plan', 'commit'])).toBe(false);
  });

  it('accepts custom command order between required stages', () => {
    expect(validateStageOrder([
      'branchGen', 'plan', 'my-custom', 'implement', 'cleanup', 'commit',
    ])).toBe(true);
  });

  it('rejects order missing plan', () => {
    expect(validateStageOrder(['branchGen', 'implement', 'commit'])).toBe(false);
  });

  it('rejects order missing implement', () => {
    expect(validateStageOrder(['branchGen', 'plan', 'commit'])).toBe(false);
  });

  it('rejects single-element order', () => {
    expect(validateStageOrder(['branchGen'])).toBe(false);
  });

  it('accepts order with only required stages', () => {
    expect(validateStageOrder(['branchGen', 'plan', 'implement', 'commit'])).toBe(true);
  });
});

describe('stageOrder in AgentConfig', () => {
  it('default configs include stageOrder', () => {
    for (const agentId of ['claude', 'cursor', 'codex']) {
      const config = getDefaultConfigForAgent(agentId);
      expect(config.stageOrder).toBeDefined();
      expect(config.stageOrder).toHaveLength(9);
    }
  });

  it('cloneConfig produces independent stageOrder copy', () => {
    const a = getDefaultConfigForAgent('claude');
    const b = getDefaultConfigForAgent('claude');
    a.stageOrder[3] = 'deslop';
    expect(b.stageOrder[3]).toBe('code-review');
  });
});

describe('expandStageKey', () => {
  it('expands branchGen into branch-gen and branch', () => {
    expect(expandStageKey('branchGen')).toEqual(['branch-gen', 'branch']);
  });

  it('expands plan into plan and plan-validation', () => {
    expect(expandStageKey('plan')).toEqual(['plan', 'plan-validation']);
  });

  it('expands implement to itself', () => {
    expect(expandStageKey('implement')).toEqual(['implement']);
  });

  it('expands code-review into code-review and code-review-fix', () => {
    expect(expandStageKey('code-review')).toEqual(['code-review', 'code-review-fix']);
  });

  it('expands commit into add-and-commit', () => {
    expect(expandStageKey('commit')).toEqual(['add-and-commit']);
  });

  it('passes through catalog commands unchanged', () => {
    expect(expandStageKey('cleanup')).toEqual(['cleanup']);
    expect(expandStageKey('add-tests')).toEqual(['add-tests']);
    expect(expandStageKey('my-custom-command')).toEqual(['my-custom-command']);
  });
});

describe('buildFullExecutionOrder', () => {
  it('builds correct order from DEFAULT_STAGE_ORDER', () => {
    const order = buildFullExecutionOrder(DEFAULT_STAGE_ORDER);
    expect(order).toEqual([
      'branch-gen', 'branch',
      'plan', 'plan-validation',
      'implement',
      'code-review', 'code-review-fix',
      'cleanup', 'unit-tests', 'review-changes', 'deslop',
      'add-and-commit',
    ]);
  });

  it('includes catalog commands in correct position', () => {
    const order = buildFullExecutionOrder([
      'branchGen', 'plan', 'implement',
      'code-review', 'add-tests', 'fix-lint', 'cleanup',
      'commit',
    ]);
    expect(order).toEqual([
      'branch-gen', 'branch',
      'plan', 'plan-validation',
      'implement',
      'code-review', 'code-review-fix',
      'add-tests', 'fix-lint', 'cleanup',
      'add-and-commit',
    ]);
  });

  it('deduplicates stages', () => {
    const order = buildFullExecutionOrder([
      'branchGen', 'plan', 'implement', 'implement', 'commit',
    ]);
    expect(order.filter(s => s === 'implement')).toHaveLength(1);
  });

  it('handles minimal required-only order', () => {
    const order = buildFullExecutionOrder(['branchGen', 'plan', 'implement', 'commit']);
    expect(order).toEqual([
      'branch-gen', 'branch',
      'plan', 'plan-validation',
      'implement',
      'add-and-commit',
    ]);
  });
});

describe('BUILTIN_CATALOG_COMMANDS / DEFAULT_WORKFLOW_STAGES consistency', () => {
  it('every enabled builtin has a matching entry in DEFAULT_WORKFLOW_STAGES', () => {
    const enabledBuiltins = BUILTIN_CATALOG_COMMANDS.filter((c) => c.enabled);
    for (const cmd of enabledBuiltins) {
      expect(DEFAULT_WORKFLOW_STAGES[cmd.id]).toBeDefined();
    }
  });

  it('every enabled builtin appears in DEFAULT_STAGE_ORDER', () => {
    const enabledBuiltins = BUILTIN_CATALOG_COMMANDS.filter((c) => c.enabled);
    for (const cmd of enabledBuiltins) {
      expect(DEFAULT_STAGE_ORDER).toContain(cmd.id);
    }
  });

  it('disabled builtins do NOT appear in DEFAULT_STAGE_ORDER', () => {
    const disabledBuiltins = BUILTIN_CATALOG_COMMANDS.filter((c) => !c.enabled);
    for (const cmd of disabledBuiltins) {
      expect(DEFAULT_STAGE_ORDER).not.toContain(cmd.id);
    }
  });

  it('BUILTIN_CATALOG_COMMANDS IDs are all kebab-case', () => {
    for (const cmd of BUILTIN_CATALOG_COMMANDS) {
      expect(cmd.id).toMatch(/^[a-z][a-z0-9-]*$/);
    }
  });
});
