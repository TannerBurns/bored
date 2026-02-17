import { describe, it, expect, beforeEach } from 'vitest';
import { useSettingsStore, WORKFLOW_PRESETS, WORKFLOW_STAGE_INFO } from './settingsStore';
import type { WorkflowPreset, WorkflowStageKey } from './settingsStore';

describe('useSettingsStore', () => {
  beforeEach(() => {
    useSettingsStore.setState({
      theme: 'dark',
    });
  });

  describe('initial state', () => {
    it('has dark theme by default', () => {
      expect(useSettingsStore.getState().theme).toBe('dark');
    });
  });

  describe('setTheme', () => {
    it('sets theme to light', () => {
      useSettingsStore.getState().setTheme('light');
      expect(useSettingsStore.getState().theme).toBe('light');
    });

    it('sets theme to dark', () => {
      useSettingsStore.getState().setTheme('light');
      useSettingsStore.getState().setTheme('dark');
      expect(useSettingsStore.getState().theme).toBe('dark');
    });

    it('sets theme to system', () => {
      useSettingsStore.getState().setTheme('system');
      expect(useSettingsStore.getState().theme).toBe('system');
    });
  });

  describe('planner settings', () => {
    beforeEach(() => {
      useSettingsStore.setState({
        plannerAutoApprove: false,
        plannerModel: 'opus-4.5',
        plannerMaxExplorations: 10,
        plannerTimeoutMinutes: 10,
        plannerMaxRetries: 2,
        codeReviewMaxIterations: 3,
        stageTimeoutHours: 1,
        stageMaxRetries: 2,
      });
    });

    it('has correct planner defaults', () => {
      const state = useSettingsStore.getState();
      expect(state.plannerAutoApprove).toBe(false);
      expect(state.plannerModel).toBe('opus-4.5');
      expect(state.plannerMaxExplorations).toBe(10);
      expect(state.plannerTimeoutMinutes).toBe(10);
      expect(state.plannerMaxRetries).toBe(2);
      expect(state.codeReviewMaxIterations).toBe(3);
      expect(state.stageTimeoutHours).toBe(1);
      expect(state.stageMaxRetries).toBe(2);
    });

    it('sets code review max iterations', () => {
      useSettingsStore.getState().setCodeReviewMaxIterations(5);
      expect(useSettingsStore.getState().codeReviewMaxIterations).toBe(5);
    });

    it('sets code review max iterations to 1', () => {
      useSettingsStore.getState().setCodeReviewMaxIterations(1);
      expect(useSettingsStore.getState().codeReviewMaxIterations).toBe(1);
    });

    it('sets code review max iterations to max value', () => {
      useSettingsStore.getState().setCodeReviewMaxIterations(10);
      expect(useSettingsStore.getState().codeReviewMaxIterations).toBe(10);
    });

    it('sets planner max explorations', () => {
      useSettingsStore.getState().setPlannerMaxExplorations(25);
      expect(useSettingsStore.getState().plannerMaxExplorations).toBe(25);
    });

    it('sets planner auto approve', () => {
      useSettingsStore.getState().setPlannerAutoApprove(true);
      expect(useSettingsStore.getState().plannerAutoApprove).toBe(true);
    });

    it('sets planner model', () => {
      useSettingsStore.getState().setPlannerModel('opus-4.6');
      expect(useSettingsStore.getState().plannerModel).toBe('opus-4.6');
    });

    it('sets planner timeout minutes', () => {
      useSettingsStore.getState().setPlannerTimeoutMinutes(10);
      expect(useSettingsStore.getState().plannerTimeoutMinutes).toBe(10);
    });

    it('sets planner max retries', () => {
      useSettingsStore.getState().setPlannerMaxRetries(5);
      expect(useSettingsStore.getState().plannerMaxRetries).toBe(5);
    });

    it('sets stage timeout hours', () => {
      useSettingsStore.getState().setStageTimeoutHours(2);
      expect(useSettingsStore.getState().stageTimeoutHours).toBe(2);
    });

    it('sets stage max retries', () => {
      useSettingsStore.getState().setStageMaxRetries(3);
      expect(useSettingsStore.getState().stageMaxRetries).toBe(3);
    });
  });

  describe('persist migration', () => {
    it('migrates plannerModel from "default" through full chain to "opus-4.5"', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default' } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      // v0->v1: 'default'->'opus', v2->v3: 'opus'->'opus-4.5' (already versioned, v4 is a no-op)
      expect(migrated.plannerModel).toBe('opus-4.5');
    });

    it('migrates unversioned "sonnet" to "sonnet-4.5" through full chain', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'sonnet' } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      // v0->v3: 'sonnet' passes through unchanged, v3->v4: 'sonnet'->'sonnet-4.5'
      expect(migrated.plannerModel).toBe('sonnet-4.5');
    });

    it('migrates plannerTimeoutMinutes from 5 to 10 in v1->v2', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerTimeoutMinutes: 5 } as unknown,
        1
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerTimeoutMinutes).toBe(10);
    });

    it('preserves custom plannerTimeoutMinutes during v1->v2 migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerTimeoutMinutes: 8 } as unknown,
        1
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerTimeoutMinutes).toBe(8);
    });

    it('applies all migrations when upgrading from v0', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5 } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerModel).toBe('opus-4.5');
      expect(migrated.plannerTimeoutMinutes).toBe(10);
    });

    it('migrates unversioned "opus" to "opus-4.6" in v3->v4', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'opus' } as unknown,
        3
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerModel).toBe('opus-4.6');
    });

    it('migrates unversioned "sonnet" to "sonnet-4.5" in v3->v4', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'sonnet' } as unknown,
        3
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerModel).toBe('sonnet-4.5');
    });

    it('preserves already-versioned values in v3->v4', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'opus-4.5' } as unknown,
        3
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerModel).toBe('opus-4.5');
    });
  });

  describe('claude API settings', () => {
    beforeEach(() => {
      useSettingsStore.setState({
        claudeAuthToken: '',
        claudeApiKey: '',
        claudeBaseUrl: '',
        claudeModelOverride: '',
      });
    });

    it('has empty Claude API settings by default', () => {
      const state = useSettingsStore.getState();
      expect(state.claudeAuthToken).toBe('');
      expect(state.claudeApiKey).toBe('');
      expect(state.claudeBaseUrl).toBe('');
      expect(state.claudeModelOverride).toBe('');
    });

    it('sets auth token', () => {
      useSettingsStore.getState().setClaudeAuthToken('my-token');
      expect(useSettingsStore.getState().claudeAuthToken).toBe('my-token');
    });

    it('sets api key', () => {
      useSettingsStore.getState().setClaudeApiKey('sk-ant-xxx');
      expect(useSettingsStore.getState().claudeApiKey).toBe('sk-ant-xxx');
    });

    it('sets base url', () => {
      useSettingsStore.getState().setClaudeBaseUrl('https://custom.api.com');
      expect(useSettingsStore.getState().claudeBaseUrl).toBe('https://custom.api.com');
    });

    it('sets model override', () => {
      useSettingsStore.getState().setClaudeModelOverride('claude-opus-4-6');
      expect(useSettingsStore.getState().claudeModelOverride).toBe('claude-opus-4-6');
    });

    it('sets all API settings at once', () => {
      useSettingsStore.getState().setClaudeApiSettings({
        authToken: 'token123',
        apiKey: 'key456',
        baseUrl: 'https://api.example.com',
        modelOverride: 'custom-model',
      });
      const state = useSettingsStore.getState();
      expect(state.claudeAuthToken).toBe('token123');
      expect(state.claudeApiKey).toBe('key456');
      expect(state.claudeBaseUrl).toBe('https://api.example.com');
      expect(state.claudeModelOverride).toBe('custom-model');
    });

    it('sets partial API settings without affecting others', () => {
      useSettingsStore.getState().setClaudeApiSettings({
        authToken: 'initial-token',
        apiKey: 'initial-key',
      });
      useSettingsStore.getState().setClaudeApiSettings({
        authToken: 'updated-token',
      });
      const state = useSettingsStore.getState();
      expect(state.claudeAuthToken).toBe('updated-token');
      expect(state.claudeApiKey).toBe('initial-key');
    });

    it('preserves existing values when undefined is passed', () => {
      useSettingsStore.getState().setClaudeApiSettings({
        authToken: 'existing-token',
        apiKey: 'existing-key',
      });
      useSettingsStore.getState().setClaudeApiSettings({
        authToken: 'new-token',
        apiKey: undefined,
      });
      const state = useSettingsStore.getState();
      expect(state.claudeAuthToken).toBe('new-token');
      expect(state.claudeApiKey).toBe('existing-key');
    });

    it('can explicitly set a field to empty string', () => {
      useSettingsStore.getState().setClaudeApiSettings({
        authToken: 'token',
        apiKey: 'key',
      });
      useSettingsStore.getState().setClaudeApiSettings({
        apiKey: '',
      });
      const state = useSettingsStore.getState();
      expect(state.claudeAuthToken).toBe('token');
      expect(state.claudeApiKey).toBe('');
    });
  });

  describe('workflow settings', () => {
    beforeEach(() => {
      useSettingsStore.getState().setWorkflowPreset('balanced');
    });

    it('has balanced preset and stages by default', () => {
      const state = useSettingsStore.getState();
      expect(state.workflowPreset).toBe('balanced');
      expect(state.workflowStages).toEqual(WORKFLOW_PRESETS.balanced.stages);
    });

    describe('setWorkflowPreset', () => {
      const presetKeys: Exclude<WorkflowPreset, 'custom'>[] = [
        'comprehensive', 'balanced', 'vibe', 'standard', 'quick-fix', 'fastest',
      ];

      it.each(presetKeys)('applies "%s" preset stages', (preset) => {
        useSettingsStore.getState().setWorkflowPreset(preset);
        const state = useSettingsStore.getState();
        expect(state.workflowPreset).toBe(preset);
        expect(state.workflowStages).toEqual(WORKFLOW_PRESETS[preset].stages);
      });

      it('sets "custom" without changing stages', () => {
        useSettingsStore.getState().setWorkflowPreset('comprehensive');
        const stagesBefore = { ...useSettingsStore.getState().workflowStages };
        useSettingsStore.getState().setWorkflowPreset('custom');
        const state = useSettingsStore.getState();
        expect(state.workflowPreset).toBe('custom');
        expect(state.workflowStages).toEqual(stagesBefore);
      });
    });

    describe('setWorkflowStages', () => {
      it('bulk-sets stages and switches preset to custom', () => {
        const custom = { ...WORKFLOW_PRESETS.fastest.stages };
        useSettingsStore.getState().setWorkflowStages(custom);
        const state = useSettingsStore.getState();
        expect(state.workflowPreset).toBe('custom');
        expect(state.workflowStages).toEqual(custom);
      });
    });

    describe('setWorkflowStageConfig', () => {
      it('updates a single stage model and switches to custom', () => {
        useSettingsStore.getState().setWorkflowPreset('balanced');
        useSettingsStore.getState().setWorkflowStageConfig('plan', { model: 'sonnet-4.5' });
        const state = useSettingsStore.getState();
        expect(state.workflowPreset).toBe('custom');
        expect(state.workflowStages.plan.model).toBe('sonnet-4.5');
        expect(state.workflowStages.plan.enabled).toBe(true);
      });

      it('toggles a stage enabled state', () => {
        useSettingsStore.getState().setWorkflowStageConfig('deslop', { enabled: false });
        expect(useSettingsStore.getState().workflowStages.deslop.enabled).toBe(false);
        useSettingsStore.getState().setWorkflowStageConfig('deslop', { enabled: true });
        expect(useSettingsStore.getState().workflowStages.deslop.enabled).toBe(true);
      });

      it('preserves other stages when updating one', () => {
        useSettingsStore.getState().setWorkflowPreset('comprehensive');
        const before = { ...useSettingsStore.getState().workflowStages };
        useSettingsStore.getState().setWorkflowStageConfig('commit', { model: 'sonnet-4.5' });
        const after = useSettingsStore.getState().workflowStages;
        expect(after.plan).toEqual(before.plan);
        expect(after.implement).toEqual(before.implement);
        expect(after.commit.model).toBe('sonnet-4.5');
      });
    });
  });

  describe('workflow preset data', () => {
    const allStageKeys: WorkflowStageKey[] = [
      'plan', 'implement', 'codeReview', 'deslop',
      'cleanup', 'unitTests', 'finalReview', 'commit',
    ];

    it('every preset defines all 8 stage keys', () => {
      for (const [name, preset] of Object.entries(WORKFLOW_PRESETS)) {
        for (const key of allStageKeys) {
          expect(preset.stages[key], `${name} missing ${key}`).toBeDefined();
          expect(typeof preset.stages[key].enabled).toBe('boolean');
          expect(['opus-4.6', 'opus-4.5', 'sonnet-4.5']).toContain(preset.stages[key].model);
        }
      }
    });

    it('required stages (plan, implement, commit) are enabled in all presets', () => {
      for (const [name, preset] of Object.entries(WORKFLOW_PRESETS)) {
        expect(preset.stages.plan.enabled, `${name}: plan`).toBe(true);
        expect(preset.stages.implement.enabled, `${name}: implement`).toBe(true);
        expect(preset.stages.commit.enabled, `${name}: commit`).toBe(true);
      }
    });

    it('comprehensive preset enables all stages with opus-4.6', () => {
      const stages = WORKFLOW_PRESETS.comprehensive.stages;
      for (const key of allStageKeys) {
        expect(stages[key].enabled).toBe(true);
        expect(stages[key].model).toBe('opus-4.6');
      }
    });

    it('fastest preset enables all stages with sonnet-4.5', () => {
      const stages = WORKFLOW_PRESETS.fastest.stages;
      for (const key of allStageKeys) {
        expect(stages[key].enabled).toBe(true);
        expect(stages[key].model).toBe('sonnet-4.5');
      }
    });

    it('quick-fix disables codeReview, deslop, unitTests, finalReview', () => {
      const stages = WORKFLOW_PRESETS['quick-fix'].stages;
      expect(stages.codeReview.enabled).toBe(false);
      expect(stages.deslop.enabled).toBe(false);
      expect(stages.unitTests.enabled).toBe(false);
      expect(stages.finalReview.enabled).toBe(false);
    });

    it('WORKFLOW_STAGE_INFO has correct required flags', () => {
      const required = WORKFLOW_STAGE_INFO.filter(s => s.required).map(s => s.key);
      const optional = WORKFLOW_STAGE_INFO.filter(s => !s.required).map(s => s.key);
      expect(required).toEqual(['plan', 'implement', 'commit']);
      expect(optional).toEqual(['codeReview', 'deslop', 'cleanup', 'unitTests', 'finalReview']);
    });

    it('WORKFLOW_STAGE_INFO covers all stage keys', () => {
      const infoKeys = WORKFLOW_STAGE_INFO.map(s => s.key);
      expect(infoKeys).toEqual(allStageKeys);
    });
  });

  describe('workflow migration v4->v5', () => {
    it('adds workflowPreset and workflowStages for v4 state', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'opus-4.5' } as unknown,
        4
      ) as unknown as Record<string, unknown>;
      expect(migrated.workflowPreset).toBe('balanced');
      expect(migrated.workflowStages).toEqual(WORKFLOW_PRESETS.balanced.stages);
    });

    it('full migration from v0 includes workflow settings', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5 } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerModel).toBe('opus-4.5');
      expect(migrated.plannerTimeoutMinutes).toBe(10);
      expect(migrated.workflowPreset).toBe('balanced');
      expect(migrated.workflowStages).toBeDefined();
    });

    it('preserves existing fields during v4->v5 migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'opus-4.6', stageTimeoutMinutes: 45 } as unknown,
        4
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerModel).toBe('opus-4.6');
      // v6->v7 migration converts stageTimeoutMinutes (45) to stageTimeoutHours (ceil(45/60) = 1)
      expect(migrated.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });
  });

  describe('workflow migration v6->v7 (timeout minutes to hours)', () => {
    it('converts 30 minutes to 1 hour', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 30 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });

    it('converts 60 minutes to 1 hour', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 60 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });

    it('converts 90 minutes to 2 hours (rounds up)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 90 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(2);
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });

    it('defaults to 1 hour when stageTimeoutMinutes is absent', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {} as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(1);
    });

    it('converts 1 minute to 1 hour (minimum clamped)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 1 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });

    it('converts 59 minutes to 1 hour (ceil)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 59 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(1);
    });

    it('converts 61 minutes to 2 hours (ceil)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 61 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(2);
    });

    it('converts 120 minutes to exactly 2 hours', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 120 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(2);
    });

    it('converts 121 minutes to 3 hours (ceil)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 121 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      expect(migrated.stageTimeoutHours).toBe(3);
    });

    it('full chain from v0 converts timeout minutes to hours', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5, stageTimeoutMinutes: 45 } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      expect(migrated.plannerModel).toBe('opus-4.5');
      expect(migrated.plannerTimeoutMinutes).toBe(10);
      expect(migrated.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });
  });

  describe('claude CLI option settings', () => {
    beforeEach(() => {
      useSettingsStore.setState({
        claudeThinkingEnabled: true,
        claudeExtendedContext: false,
        claudeChromeEnabled: false,
      });
    });

    it('has correct CLI option defaults', () => {
      const state = useSettingsStore.getState();
      expect(state.claudeThinkingEnabled).toBe(true);
      expect(state.claudeExtendedContext).toBe(false);
      expect(state.claudeChromeEnabled).toBe(false);
    });

    it('sets thinking enabled', () => {
      useSettingsStore.getState().setClaudeThinkingEnabled(false);
      expect(useSettingsStore.getState().claudeThinkingEnabled).toBe(false);
    });

    it('sets extended context', () => {
      useSettingsStore.getState().setClaudeExtendedContext(true);
      expect(useSettingsStore.getState().claudeExtendedContext).toBe(true);
    });

    it('sets chrome enabled', () => {
      useSettingsStore.getState().setClaudeChromeEnabled(true);
      expect(useSettingsStore.getState().claudeChromeEnabled).toBe(true);
    });

    it('sets CLI options via setClaudeApiSettings', () => {
      useSettingsStore.getState().setClaudeApiSettings({
        thinkingEnabled: false,
        extendedContext: true,
        chromeEnabled: true,
      });
      const state = useSettingsStore.getState();
      expect(state.claudeThinkingEnabled).toBe(false);
      expect(state.claudeExtendedContext).toBe(true);
      expect(state.claudeChromeEnabled).toBe(true);
    });

    it('preserves CLI options when setClaudeApiSettings omits them', () => {
      useSettingsStore.getState().setClaudeThinkingEnabled(false);
      useSettingsStore.getState().setClaudeExtendedContext(true);
      // Update only authToken, CLI options should be preserved
      useSettingsStore.getState().setClaudeApiSettings({
        authToken: 'new-token',
      });
      const state = useSettingsStore.getState();
      expect(state.claudeThinkingEnabled).toBe(false);
      expect(state.claudeExtendedContext).toBe(true);
      expect(state.claudeChromeEnabled).toBe(false);
    });
  });

  describe('workflow migration v7->v8->v9 (CLI option settings)', () => {
    it('adds CLI option defaults when migrating from v7 (moved to agentSettings by v9)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {} as unknown,
        7
      ) as unknown as Record<string, unknown>;
      // v8 adds defaults, v9 moves them to agentSettings
      const agentSettings = migrated.agentSettings as Record<string, Record<string, unknown>>;
      expect(agentSettings.claude.thinkingEnabled).toBe(true);
      expect(agentSettings.claude.extendedContext).toBe(false);
      expect(agentSettings.claude.chromeEnabled).toBe(false);
      // Legacy top-level fields should be removed by v9
      expect(migrated.claudeThinkingEnabled).toBeUndefined();
    });

    it('preserves existing CLI option values during migration (moved to agentSettings)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          claudeThinkingEnabled: false,
          claudeExtendedContext: true,
          claudeChromeEnabled: true,
        } as unknown,
        7
      ) as unknown as Record<string, unknown>;
      // v8 preserves existing values, v9 moves them to agentSettings
      const agentSettings = migrated.agentSettings as Record<string, Record<string, unknown>>;
      expect(agentSettings.claude.thinkingEnabled).toBe(false);
      expect(agentSettings.claude.extendedContext).toBe(true);
      expect(agentSettings.claude.chromeEnabled).toBe(true);
    });

    it('full migration from v0 includes CLI option defaults in agentSettings', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5 } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      const agentSettings = migrated.agentSettings as Record<string, Record<string, unknown>>;
      expect(agentSettings.claude.thinkingEnabled).toBe(true);
      expect(agentSettings.claude.extendedContext).toBe(false);
      expect(agentSettings.claude.chromeEnabled).toBe(false);
    });
  });

  describe('persist config', () => {
    it('uses version 9', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      expect(options.version).toBe(9);
    });
  });

  describe('generic agentSettings', () => {
    beforeEach(() => {
      useSettingsStore.setState({
        agentSettings: {
          claude: {
            authToken: '',
            apiKey: '',
            baseUrl: '',
            modelOverride: '',
            thinkingEnabled: true,
            extendedContext: false,
            chromeEnabled: false,
          },
        },
      });
    });

    it('has claude defaults in initial agentSettings', () => {
      const state = useSettingsStore.getState();
      expect(state.agentSettings.claude).toBeDefined();
      expect(state.agentSettings.claude.thinkingEnabled).toBe(true);
      expect(state.agentSettings.claude.extendedContext).toBe(false);
    });

    it('getAgentSettings returns empty object for unknown agent', () => {
      const settings = useSettingsStore.getState().getAgentSettings('unknown-agent');
      expect(settings).toEqual({});
    });

    it('getAgentSettings returns claude settings', () => {
      const settings = useSettingsStore.getState().getAgentSettings('claude');
      expect(settings.thinkingEnabled).toBe(true);
    });

    it('setAgentSetting updates a single key', () => {
      useSettingsStore.getState().setAgentSetting('claude', 'authToken', 'my-token');
      const settings = useSettingsStore.getState().getAgentSettings('claude');
      expect(settings.authToken).toBe('my-token');
      expect(settings.thinkingEnabled).toBe(true);
    });

    it('setAgentSettings merges multiple keys', () => {
      useSettingsStore.getState().setAgentSettings('claude', {
        authToken: 'tok',
        apiKey: 'key',
      });
      const settings = useSettingsStore.getState().getAgentSettings('claude');
      expect(settings.authToken).toBe('tok');
      expect(settings.apiKey).toBe('key');
      expect(settings.thinkingEnabled).toBe(true);
    });

    it('supports arbitrary new agent IDs', () => {
      useSettingsStore.getState().setAgentSettings('my-new-agent', {
        customField: 'value',
        enabled: true,
      });
      const settings = useSettingsStore.getState().getAgentSettings('my-new-agent');
      expect(settings.customField).toBe('value');
      expect(settings.enabled).toBe(true);
    });

    it('does not clobber other agents when updating one', () => {
      useSettingsStore.getState().setAgentSettings('my-new-agent', { foo: 'bar' });
      const claude = useSettingsStore.getState().getAgentSettings('claude');
      expect(claude.thinkingEnabled).toBe(true);
    });
  });

  describe('legacy Claude setters sync agentSettings', () => {
    beforeEach(() => {
      useSettingsStore.setState({
        agentSettings: {
          claude: {
            authToken: '',
            apiKey: '',
            baseUrl: '',
            modelOverride: '',
            thinkingEnabled: true,
            extendedContext: false,
            chromeEnabled: false,
          },
        },
        claudeAuthToken: '',
        claudeApiKey: '',
        claudeBaseUrl: '',
        claudeModelOverride: '',
        claudeThinkingEnabled: true,
        claudeExtendedContext: false,
        claudeChromeEnabled: false,
      });
    });

    it('setClaudeAuthToken updates both top-level and agentSettings', () => {
      useSettingsStore.getState().setClaudeAuthToken('tok');
      const state = useSettingsStore.getState();
      expect(state.claudeAuthToken).toBe('tok');
      expect(state.agentSettings.claude.authToken).toBe('tok');
    });

    it('setClaudeThinkingEnabled updates both top-level and agentSettings', () => {
      useSettingsStore.getState().setClaudeThinkingEnabled(false);
      const state = useSettingsStore.getState();
      expect(state.claudeThinkingEnabled).toBe(false);
      expect(state.agentSettings.claude.thinkingEnabled).toBe(false);
    });

    it('setClaudeApiSettings updates both top-level and agentSettings', () => {
      useSettingsStore.getState().setClaudeApiSettings({
        authToken: 'new-tok',
        thinkingEnabled: false,
      });
      const state = useSettingsStore.getState();
      expect(state.claudeAuthToken).toBe('new-tok');
      expect(state.claudeThinkingEnabled).toBe(false);
      expect(state.agentSettings.claude.authToken).toBe('new-tok');
      expect(state.agentSettings.claude.thinkingEnabled).toBe(false);
    });
  });

  describe('partialize excludes legacy fields', () => {
    it('does not include Claude derived fields in partialize output', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const full = useSettingsStore.getState();
      const partialized = options.partialize!(full);
      expect(partialized).not.toHaveProperty('claudeAuthToken');
      expect(partialized).not.toHaveProperty('claudeApiKey');
      expect(partialized).not.toHaveProperty('claudeBaseUrl');
      expect(partialized).not.toHaveProperty('claudeModelOverride');
      expect(partialized).not.toHaveProperty('claudeThinkingEnabled');
      expect(partialized).not.toHaveProperty('claudeExtendedContext');
      expect(partialized).not.toHaveProperty('claudeChromeEnabled');
    });

    it('preserves agentSettings in partialize output', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const full = useSettingsStore.getState();
      const partialized = options.partialize!(full) as Record<string, unknown>;
      expect(partialized).toHaveProperty('agentSettings');
    });
  });

  describe('migration v8->v9 (Claude fields to agentSettings)', () => {
    it('migrates v8 Claude fields into agentSettings map', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          claudeAuthToken: 'tok',
          claudeApiKey: 'key',
          claudeBaseUrl: 'https://api.example.com',
          claudeModelOverride: 'model',
          claudeThinkingEnabled: false,
          claudeExtendedContext: true,
          claudeChromeEnabled: true,
        } as unknown,
        8
      ) as unknown as Record<string, unknown>;

      const agentSettings = migrated.agentSettings as Record<string, Record<string, unknown>>;
      expect(agentSettings.claude.authToken).toBe('tok');
      expect(agentSettings.claude.apiKey).toBe('key');
      expect(agentSettings.claude.baseUrl).toBe('https://api.example.com');
      expect(agentSettings.claude.modelOverride).toBe('model');
      expect(agentSettings.claude.thinkingEnabled).toBe(false);
      expect(agentSettings.claude.extendedContext).toBe(true);
      expect(agentSettings.claude.chromeEnabled).toBe(true);
    });

    it('removes legacy fields from top-level state after v9 migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          claudeAuthToken: 'tok',
          claudeThinkingEnabled: true,
        } as unknown,
        8
      ) as unknown as Record<string, unknown>;

      expect(migrated.claudeAuthToken).toBeUndefined();
      expect(migrated.claudeThinkingEnabled).toBeUndefined();
    });

    it('sets defaults for missing boolean fields', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { claudeAuthToken: 'tok' } as unknown,
        8
      ) as unknown as Record<string, unknown>;

      const agentSettings = migrated.agentSettings as Record<string, Record<string, unknown>>;
      expect(agentSettings.claude.thinkingEnabled).toBe(true);
      expect(agentSettings.claude.extendedContext).toBe(false);
      expect(agentSettings.claude.chromeEnabled).toBe(false);
    });

    it('full migration from v7 runs v8 then v9 in correct order', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { claudeAuthToken: 'tok' } as unknown,
        7
      ) as unknown as Record<string, unknown>;

      // v8 adds CLI defaults, then v9 moves everything to agentSettings
      const agentSettings = migrated.agentSettings as Record<string, Record<string, unknown>>;
      expect(agentSettings.claude.authToken).toBe('tok');
      expect(agentSettings.claude.thinkingEnabled).toBe(true);
      expect(agentSettings.claude.extendedContext).toBe(false);
      expect(agentSettings.claude.chromeEnabled).toBe(false);
      // Legacy fields should be cleaned up
      expect(migrated.claudeAuthToken).toBeUndefined();
      expect(migrated.claudeThinkingEnabled).toBeUndefined();
    });

    it('full migration from v0 produces agentSettings', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          plannerModel: 'default',
          plannerTimeoutMinutes: 5,
          claudeAuthToken: 'old-tok',
        } as unknown,
        0
      ) as unknown as Record<string, unknown>;

      expect(migrated.plannerModel).toBe('opus-4.5');
      const agentSettings = migrated.agentSettings as Record<string, Record<string, unknown>>;
      expect(agentSettings.claude.authToken).toBe('old-tok');
      expect(agentSettings.claude.thinkingEnabled).toBe(true);
    });
  });
});
