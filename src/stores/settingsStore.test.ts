import { describe, it, expect, beforeEach } from 'vitest';
import { useSettingsStore, WORKFLOW_PRESETS, WORKFLOW_STAGE_INFO, MODEL_OPTIONS, DEFAULT_STAGE_ORDER } from './settingsStore';
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
      useSettingsStore.getState().updateAgentConfig('claude', {
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
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.plannerAutoApprove).toBe(false);
      expect(config.plannerModel).toBe('opus-4.5');
      expect(config.plannerMaxExplorations).toBe(10);
      expect(config.plannerTimeoutMinutes).toBe(10);
      expect(config.plannerMaxRetries).toBe(2);
      expect(config.codeReviewMaxIterations).toBe(3);
      expect(config.stageTimeoutHours).toBe(1);
      expect(config.stageMaxRetries).toBe(2);
    });

    it('sets code review max iterations', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { codeReviewMaxIterations: 5 });
      expect(useSettingsStore.getState().getAgentConfig('claude').codeReviewMaxIterations).toBe(5);
    });

    it('sets code review max iterations to 1', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { codeReviewMaxIterations: 1 });
      expect(useSettingsStore.getState().getAgentConfig('claude').codeReviewMaxIterations).toBe(1);
    });

    it('sets code review max iterations to max value', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { codeReviewMaxIterations: 10 });
      expect(useSettingsStore.getState().getAgentConfig('claude').codeReviewMaxIterations).toBe(10);
    });

    it('sets planner max explorations', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { plannerMaxExplorations: 25 });
      expect(useSettingsStore.getState().getAgentConfig('claude').plannerMaxExplorations).toBe(25);
    });

    it('sets planner auto approve', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { plannerAutoApprove: true });
      expect(useSettingsStore.getState().getAgentConfig('claude').plannerAutoApprove).toBe(true);
    });

    it('sets planner model', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { plannerModel: 'opus-4.6' });
      expect(useSettingsStore.getState().getAgentConfig('claude').plannerModel).toBe('opus-4.6');
    });

    it('sets planner timeout minutes', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { plannerTimeoutMinutes: 10 });
      expect(useSettingsStore.getState().getAgentConfig('claude').plannerTimeoutMinutes).toBe(10);
    });

    it('sets planner max retries', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { plannerMaxRetries: 5 });
      expect(useSettingsStore.getState().getAgentConfig('claude').plannerMaxRetries).toBe(5);
    });

    it('sets stage timeout hours', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { stageTimeoutHours: 2 });
      expect(useSettingsStore.getState().getAgentConfig('claude').stageTimeoutHours).toBe(2);
    });

    it('sets stage max retries', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { stageMaxRetries: 3 });
      expect(useSettingsStore.getState().getAgentConfig('claude').stageMaxRetries).toBe(3);
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
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('opus-4.5');
      expect(migrated.plannerModel).toBeUndefined();
    });

    it('migrates unversioned "sonnet" to "sonnet-4.5" through full chain', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'sonnet' } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      // v0->v3: 'sonnet' passes through unchanged, v3->v4: 'sonnet'->'sonnet-4.5'
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('sonnet-4.5');
      expect(migrated.plannerModel).toBeUndefined();
    });

    it('migrates plannerTimeoutMinutes from 5 to 10 in v1->v2', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerTimeoutMinutes: 5 } as unknown,
        1
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerTimeoutMinutes).toBe(10);
      expect(migrated.plannerTimeoutMinutes).toBeUndefined();
    });

    it('preserves custom plannerTimeoutMinutes during v1->v2 migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerTimeoutMinutes: 8 } as unknown,
        1
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerTimeoutMinutes).toBe(8);
      expect(migrated.plannerTimeoutMinutes).toBeUndefined();
    });

    it('applies all migrations when upgrading from v0', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5 } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('opus-4.5');
      expect(agentConfigs.claude.plannerTimeoutMinutes).toBe(10);
      expect(migrated.plannerModel).toBeUndefined();
      expect(migrated.plannerTimeoutMinutes).toBeUndefined();
    });

    it('migrates unversioned "opus" to "opus-4.6" in v3->v4', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'opus' } as unknown,
        3
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('opus-4.6');
      expect(migrated.plannerModel).toBeUndefined();
    });

    it('migrates unversioned "sonnet" to "sonnet-4.5" in v3->v4', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'sonnet' } as unknown,
        3
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('sonnet-4.5');
      expect(migrated.plannerModel).toBeUndefined();
    });

    it('preserves already-versioned values in v3->v4', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'opus-4.5' } as unknown,
        3
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('opus-4.5');
      expect(migrated.plannerModel).toBeUndefined();
    });
  });

  describe('generic agent settings', () => {
    it('has claude defaults in agentSettings', () => {
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings).toBeDefined();
      expect(claudeSettings.authToken).toBe('');
    });

    it('sets agent settings for a specific agent', () => {
      useSettingsStore.getState().setAgentSettings('claude', {
        authToken: 'token123',
        apiKey: 'key456',
      });
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings.authToken).toBe('token123');
      expect(claudeSettings.apiKey).toBe('key456');
    });

    it('sets individual agent setting', () => {
      useSettingsStore.getState().setAgentSetting('claude', 'authToken', 'my-token');
      expect(useSettingsStore.getState().getAgentSettings('claude').authToken).toBe('my-token');
    });

    it('preserves existing values when setting partial', () => {
      useSettingsStore.getState().setAgentSettings('claude', {
        authToken: 'initial-token',
        apiKey: 'initial-key',
      });
      useSettingsStore.getState().setAgentSettings('claude', {
        authToken: 'updated-token',
      });
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings.authToken).toBe('updated-token');
      expect(claudeSettings.apiKey).toBe('initial-key');
    });

    it('supports arbitrary agent IDs', () => {
      useSettingsStore.getState().setAgentSettings('windsurf', {
        apiKey: 'ws-key',
      });
      expect(useSettingsStore.getState().getAgentSettings('windsurf').apiKey).toBe('ws-key');
    });

    it('getAgentSettings returns empty for unknown agent', () => {
      const settings = useSettingsStore.getState().getAgentSettings('unknown');
      expect(settings).toEqual({});
    });
  });

  describe('workflow settings', () => {
    beforeEach(() => {
      useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'balanced');
    });

    it('has balanced preset and stages by default', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.workflowPreset).toBe('balanced');
      expect(config.workflowStages).toEqual(WORKFLOW_PRESETS.balanced.stages);
    });

    describe('setWorkflowPreset', () => {
      const presetKeys: Exclude<WorkflowPreset, 'custom'>[] = [
        'comprehensive', 'balanced', 'vibe', 'standard', 'quick-fix', 'fastest',
      ];

      it.each(presetKeys)('applies "%s" preset stages', (preset) => {
        useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', preset);
        const config = useSettingsStore.getState().getAgentConfig('claude');
        expect(config.workflowPreset).toBe(preset);
        expect(config.workflowStages).toEqual(WORKFLOW_PRESETS[preset].stages);
      });

      it('sets "custom" without changing stages', () => {
        useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'comprehensive');
        const stagesBefore = { ...useSettingsStore.getState().getAgentConfig('claude').workflowStages };
        useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'custom');
        const config = useSettingsStore.getState().getAgentConfig('claude');
        expect(config.workflowPreset).toBe('custom');
        expect(config.workflowStages).toEqual(stagesBefore);
      });
    });

    describe('setWorkflowStages', () => {
      it('bulk-sets stages and switches preset to custom', () => {
        const custom = { ...WORKFLOW_PRESETS.fastest.stages };
        useSettingsStore.getState().updateAgentConfig('claude', {
          workflowStages: custom,
          workflowPreset: 'custom',
        });
        const config = useSettingsStore.getState().getAgentConfig('claude');
        expect(config.workflowPreset).toBe('custom');
        expect(config.workflowStages).toEqual(custom);
      });
    });

    describe('setWorkflowStageConfig', () => {
      it('updates a single stage model and switches to custom', () => {
        useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'balanced');
        useSettingsStore.getState().setAgentConfigStage('claude', 'plan', { model: 'sonnet-4.5' });
        const config = useSettingsStore.getState().getAgentConfig('claude');
        expect(config.workflowPreset).toBe('custom');
        expect(config.workflowStages.plan.model).toBe('sonnet-4.5');
        expect(config.workflowStages.plan.enabled).toBe(true);
      });

      it('toggles a stage enabled state', () => {
        useSettingsStore.getState().setAgentConfigStage('claude', 'deslop', { enabled: false });
        expect(useSettingsStore.getState().getAgentConfig('claude').workflowStages.deslop.enabled).toBe(false);
        useSettingsStore.getState().setAgentConfigStage('claude', 'deslop', { enabled: true });
        expect(useSettingsStore.getState().getAgentConfig('claude').workflowStages.deslop.enabled).toBe(true);
      });

      it('preserves other stages when updating one', () => {
        useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'comprehensive');
        const before = { ...useSettingsStore.getState().getAgentConfig('claude').workflowStages };
        useSettingsStore.getState().setAgentConfigStage('claude', 'commit', { model: 'sonnet-4.5' });
        const after = useSettingsStore.getState().getAgentConfig('claude').workflowStages;
        expect(after.plan).toEqual(before.plan);
        expect(after.implement).toEqual(before.implement);
        expect(after.commit.model).toBe('sonnet-4.5');
      });
    });
  });

  describe('workflow preset data', () => {
    const allStageKeys: WorkflowStageKey[] = [
      'branchGen', 'plan', 'implement', 'codeReview', 'deslop',
      'cleanup', 'unitTests', 'finalReview', 'commit',
    ];

    it('every preset defines all 8 stage keys', () => {
      for (const [name, preset] of Object.entries(WORKFLOW_PRESETS)) {
        for (const key of allStageKeys) {
          expect(preset.stages[key], `${name} missing ${key}`).toBeDefined();
          expect(typeof preset.stages[key].enabled).toBe('boolean');
          expect(['opus-4.6', 'opus-4.5', 'sonnet-4.6', 'sonnet-4.5']).toContain(preset.stages[key].model);
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

    it('comprehensive preset enables all stages with opus-4.6 (except branchGen which uses sonnet-4.6)', () => {
      const stages = WORKFLOW_PRESETS.comprehensive.stages;
      for (const key of allStageKeys) {
        expect(stages[key].enabled).toBe(true);
        if (key === 'branchGen') {
          expect(stages[key].model).toBe('sonnet-4.6');
        } else {
          expect(stages[key].model).toBe('opus-4.6');
        }
      }
    });

    it('fastest preset enables all stages with sonnet-4.6', () => {
      const stages = WORKFLOW_PRESETS.fastest.stages;
      for (const key of allStageKeys) {
        expect(stages[key].enabled).toBe(true);
        expect(stages[key].model).toBe('sonnet-4.6');
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
      expect(required).toEqual(['branchGen', 'plan', 'implement', 'commit']);
      expect(optional).toEqual(['codeReview', 'cleanup', 'unitTests', 'finalReview', 'deslop']);
    });

    it('WORKFLOW_STAGE_INFO covers all stage keys', () => {
      const infoKeys = new Set(WORKFLOW_STAGE_INFO.map(s => s.key));
      for (const key of allStageKeys) {
        expect(infoKeys.has(key), `Missing stage key: ${key}`).toBe(true);
      }
      expect(infoKeys.size).toBe(allStageKeys.length);
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
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.workflowPreset).toBe('balanced');
      expect(agentConfigs.claude.workflowStages).toEqual(WORKFLOW_PRESETS.balanced.stages);
      expect(migrated.workflowPreset).toBeUndefined();
      expect(migrated.workflowStages).toBeUndefined();
    });

    it('full migration from v0 includes workflow settings', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5 } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('opus-4.5');
      expect(agentConfigs.claude.plannerTimeoutMinutes).toBe(10);
      expect(agentConfigs.claude.workflowPreset).toBe('balanced');
      expect(agentConfigs.claude.workflowStages).toBeDefined();
      expect(migrated.plannerModel).toBeUndefined();
      expect(migrated.plannerTimeoutMinutes).toBeUndefined();
      expect(migrated.workflowPreset).toBeUndefined();
      expect(migrated.workflowStages).toBeUndefined();
    });

    it('preserves existing fields during v4->v5 migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'opus-4.6', stageTimeoutMinutes: 45 } as unknown,
        4
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('opus-4.6');
      // v6->v7 migration converts stageTimeoutMinutes (45) to stageTimeoutHours (ceil(45/60) = 1)
      expect(agentConfigs.claude.stageTimeoutHours).toBe(1);
      expect(migrated.plannerModel).toBeUndefined();
      expect(migrated.stageTimeoutHours).toBeUndefined();
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
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutHours).toBeUndefined();
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });

    it('converts 60 minutes to 1 hour', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 60 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutHours).toBeUndefined();
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });

    it('converts 90 minutes to 2 hours (rounds up)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 90 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(2);
      expect(migrated.stageTimeoutHours).toBeUndefined();
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });

    it('defaults to 1 hour when stageTimeoutMinutes is absent', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {} as unknown,
        6
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutHours).toBeUndefined();
    });

    it('converts 1 minute to 1 hour (minimum clamped)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 1 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutHours).toBeUndefined();
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });

    it('converts 59 minutes to 1 hour (ceil)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 59 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(1);
      expect(migrated.stageTimeoutHours).toBeUndefined();
    });

    it('converts 61 minutes to 2 hours (ceil)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 61 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(2);
      expect(migrated.stageTimeoutHours).toBeUndefined();
    });

    it('converts 120 minutes to exactly 2 hours', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 120 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(2);
      expect(migrated.stageTimeoutHours).toBeUndefined();
    });

    it('converts 121 minutes to 3 hours (ceil)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { stageTimeoutMinutes: 121 } as unknown,
        6
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.stageTimeoutHours).toBe(3);
      expect(migrated.stageTimeoutHours).toBeUndefined();
    });

    it('full chain from v0 converts timeout minutes to hours', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5, stageTimeoutMinutes: 45 } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.plannerModel).toBe('opus-4.5');
      expect(agentConfigs.claude.plannerTimeoutMinutes).toBe(10);
      expect(agentConfigs.claude.stageTimeoutHours).toBe(1);
      expect(migrated.plannerModel).toBeUndefined();
      expect(migrated.plannerTimeoutMinutes).toBeUndefined();
      expect(migrated.stageTimeoutHours).toBeUndefined();
      expect(migrated.stageTimeoutMinutes).toBeUndefined();
    });
  });

  describe('claude CLI option settings via generic API', () => {
    beforeEach(() => {
      useSettingsStore.getState().setAgentSettings('claude', {
        authToken: '',
        apiKey: '',
        baseUrl: '',
        modelOverride: '',
        thinkingEnabled: true,
        extendedContext: false,
        chromeEnabled: false,
      });
    });

    it('has correct CLI option defaults in agentSettings', () => {
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings.thinkingEnabled).toBe(true);
      expect(claudeSettings.extendedContext).toBe(false);
      expect(claudeSettings.chromeEnabled).toBe(false);
    });

    it('sets thinking enabled via generic setAgentSetting', () => {
      useSettingsStore.getState().setAgentSetting('claude', 'thinkingEnabled', false);
      expect(useSettingsStore.getState().getAgentSettings('claude').thinkingEnabled).toBe(false);
    });

    it('sets extended context via generic setAgentSetting', () => {
      useSettingsStore.getState().setAgentSetting('claude', 'extendedContext', true);
      expect(useSettingsStore.getState().getAgentSettings('claude').extendedContext).toBe(true);
    });

    it('sets chrome enabled via generic setAgentSetting', () => {
      useSettingsStore.getState().setAgentSetting('claude', 'chromeEnabled', true);
      expect(useSettingsStore.getState().getAgentSettings('claude').chromeEnabled).toBe(true);
    });

    it('sets multiple CLI options via setAgentSettings', () => {
      useSettingsStore.getState().setAgentSettings('claude', {
        thinkingEnabled: false,
        extendedContext: true,
        chromeEnabled: true,
      });
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings.thinkingEnabled).toBe(false);
      expect(claudeSettings.extendedContext).toBe(true);
      expect(claudeSettings.chromeEnabled).toBe(true);
    });

    it('preserves existing settings when setting partial', () => {
      useSettingsStore.getState().setAgentSetting('claude', 'thinkingEnabled', false);
      useSettingsStore.getState().setAgentSetting('claude', 'extendedContext', true);
      useSettingsStore.getState().setAgentSettings('claude', {
        authToken: 'new-token',
      });
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings.thinkingEnabled).toBe(false);
      expect(claudeSettings.extendedContext).toBe(true);
      expect(claudeSettings.chromeEnabled).toBe(false);
    });
  });

  describe('workflow migration v7->v8->v9->v12 (CLI option settings)', () => {
    it('adds CLI option defaults when migrating from v7 (moved to agentConfigs by v12)', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {} as unknown,
        7
      ) as unknown as Record<string, unknown>;
      // v8 adds defaults, v9 moves them to agentSettings, v12 moves to agentConfigs
      const agentConfigs = migrated.agentConfigs as Record<string, { settings: Record<string, unknown> }>;
      expect(agentConfigs.claude.settings.thinkingEnabled).toBe(true);
      expect(agentConfigs.claude.settings.extendedContext).toBe(false);
      expect(agentConfigs.claude.settings.chromeEnabled).toBe(false);
      // Legacy top-level fields should be removed by v9
      expect(migrated.claudeThinkingEnabled).toBeUndefined();
      // agentSettings should be removed by v12
      expect(migrated.agentSettings).toBeUndefined();
    });

    it('preserves existing CLI option values during migration (moved to agentConfigs)', () => {
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
      // v8 preserves existing values, v9 moves them to agentSettings, v12 moves to agentConfigs
      const agentConfigs = migrated.agentConfigs as Record<string, { settings: Record<string, unknown> }>;
      expect(agentConfigs.claude.settings.thinkingEnabled).toBe(false);
      expect(agentConfigs.claude.settings.extendedContext).toBe(true);
      expect(agentConfigs.claude.settings.chromeEnabled).toBe(true);
    });

    it('full migration from v0 includes CLI option defaults in agentConfigs', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5 } as unknown,
        0
      ) as unknown as Record<string, unknown>;
      const agentConfigs = migrated.agentConfigs as Record<string, { settings: Record<string, unknown> }>;
      expect(agentConfigs.claude.settings.thinkingEnabled).toBe(true);
      expect(agentConfigs.claude.settings.extendedContext).toBe(false);
      expect(agentConfigs.claude.settings.chromeEnabled).toBe(false);
    });
  });

  describe('setAgentConfigStageOrder', () => {
    beforeEach(() => {
      useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'balanced');
    });

    it('updates stage order and switches to custom', () => {
      const newOrder: WorkflowStageKey[] = [
        'branchGen', 'plan', 'implement',
        'deslop', 'cleanup', 'codeReview', 'unitTests', 'finalReview',
        'commit',
      ];
      useSettingsStore.getState().setAgentConfigStageOrder('claude', newOrder);
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.workflowPreset).toBe('custom');
      expect(config.stageOrder).toEqual(newOrder);
    });

    it('does not affect other agents', () => {
      useSettingsStore.getState().setAgentConfigWorkflowPreset('cursor', 'balanced');
      const newOrder: WorkflowStageKey[] = [
        'branchGen', 'plan', 'implement',
        'deslop', 'codeReview', 'cleanup', 'unitTests', 'finalReview',
        'commit',
      ];
      useSettingsStore.getState().setAgentConfigStageOrder('claude', newOrder);
      const cursorConfig = useSettingsStore.getState().getAgentConfig('cursor');
      expect(cursorConfig.stageOrder).toEqual(DEFAULT_STAGE_ORDER);
      expect(cursorConfig.workflowPreset).toBe('balanced');
    });

    it('stores a copy (not a reference)', () => {
      const order: WorkflowStageKey[] = [...DEFAULT_STAGE_ORDER];
      useSettingsStore.getState().setAgentConfigStageOrder('claude', order);
      order[3] = 'deslop';
      expect(useSettingsStore.getState().getAgentConfig('claude').stageOrder[3]).toBe('codeReview');
    });
  });

  describe('setAgentConfigWorkflowPreset sets stageOrder', () => {
    it('preset sets stageOrder from preset definition', () => {
      useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'vibe');
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.stageOrder).toEqual(WORKFLOW_PRESETS.vibe.stageOrder);
    });

    it('switching to custom preserves existing stageOrder', () => {
      useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'vibe');
      const orderBefore = [...useSettingsStore.getState().getAgentConfig('claude').stageOrder];
      useSettingsStore.getState().setAgentConfigWorkflowPreset('claude', 'custom');
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.stageOrder).toEqual(orderBefore);
    });
  });

  describe('workflow preset stageOrder data', () => {
    it('every preset defines a stageOrder with all 9 stages', () => {
      const allKeys = new Set(DEFAULT_STAGE_ORDER);
      for (const [name, preset] of Object.entries(WORKFLOW_PRESETS)) {
        expect(preset.stageOrder, `${name} stageOrder`).toHaveLength(9);
        for (const key of allKeys) {
          expect(preset.stageOrder.includes(key), `${name} missing ${key}`).toBe(true);
        }
      }
    });

    it('every preset stageOrder starts with branchGen/plan/implement and ends with commit', () => {
      for (const [name, preset] of Object.entries(WORKFLOW_PRESETS)) {
        expect(preset.stageOrder.slice(0, 3), `${name} start`).toEqual(['branchGen', 'plan', 'implement']);
        expect(preset.stageOrder[preset.stageOrder.length - 1], `${name} end`).toBe('commit');
      }
    });
  });

  describe('workflow migration v12->v13 (stageOrder)', () => {
    it('adds stageOrder to existing agentConfigs', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: { workflowPreset: 'balanced', workflowStages: {} },
            cursor: { workflowPreset: 'custom', workflowStages: {} },
          },
        } as unknown,
        12
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { stageOrder: unknown }>;
      expect(configs.claude.stageOrder).toEqual(DEFAULT_STAGE_ORDER);
      expect(configs.cursor.stageOrder).toEqual(DEFAULT_STAGE_ORDER);
    });

    it('does not overwrite existing stageOrder', () => {
      const customOrder = ['branchGen', 'plan', 'implement', 'deslop', 'codeReview', 'cleanup', 'unitTests', 'finalReview', 'commit'];
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: { stageOrder: customOrder },
          },
        } as unknown,
        12
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { stageOrder: unknown }>;
      expect(configs.claude.stageOrder).toEqual(customOrder);
    });

    it('full migration from v0 includes stageOrder', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default' } as unknown,
        0
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { stageOrder: unknown }>;
      expect(configs.claude.stageOrder).toEqual(DEFAULT_STAGE_ORDER);
      expect(configs.cursor.stageOrder).toEqual(DEFAULT_STAGE_ORDER);
      expect(configs.codex.stageOrder).toEqual(DEFAULT_STAGE_ORDER);
    });
  });

  describe('persist config', () => {
    it('uses version 13', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      expect(options.version).toBe(13);
    });
  });

  describe('validation agent settings', () => {
    it('has correct default validationModel', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.validationModel).toBe('sonnet-4.6');
    });
  });

  describe('diagnostic agent settings', () => {
    it('has correct default diagnosticModel', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.diagnosticModel).toBe('sonnet-4.6');
    });

    it('sets diagnosticModel', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { diagnosticModel: 'opus-4.5' });
      expect(useSettingsStore.getState().getAgentConfig('claude').diagnosticModel).toBe('opus-4.5');
    });
  });

  describe('MODEL_OPTIONS', () => {
    it('includes all expected models', () => {
      const values = MODEL_OPTIONS.map(o => o.value);
      expect(values).toContain('opus-4.6');
      expect(values).toContain('opus-4.5');
      expect(values).toContain('sonnet-4.6');
      expect(values).toContain('sonnet-4.5');
    });
  });

  describe('workflow migration v9->v10', () => {
    it('adds branchGen stage to existing workflowStages', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          workflowStages: {
            plan: { enabled: true, model: 'opus-4.6' },
            implement: { enabled: true, model: 'opus-4.6' },
            codeReview: { enabled: true, model: 'opus-4.6' },
            deslop: { enabled: true, model: 'opus-4.5' },
            cleanup: { enabled: true, model: 'sonnet-4.5' },
            unitTests: { enabled: true, model: 'opus-4.5' },
            finalReview: { enabled: true, model: 'opus-4.5' },
            commit: { enabled: true, model: 'sonnet-4.5' },
          },
        } as unknown,
        9
      ) as unknown as Record<string, unknown>;

      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      const stages = agentConfigs.claude.workflowStages as Record<string, { enabled: boolean; model: string }>;
      expect(stages.branchGen).toEqual({ enabled: true, model: 'sonnet-4.6' });
      // Existing stages preserved
      expect(stages.plan.model).toBe('opus-4.6');
      expect(migrated.workflowStages).toBeUndefined();
    });

    it('does not overwrite branchGen if already present', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          workflowStages: {
            branchGen: { enabled: true, model: 'opus-4.5' },
            plan: { enabled: true, model: 'opus-4.6' },
            implement: { enabled: true, model: 'opus-4.6' },
            codeReview: { enabled: true, model: 'opus-4.6' },
            deslop: { enabled: true, model: 'opus-4.5' },
            cleanup: { enabled: true, model: 'sonnet-4.5' },
            unitTests: { enabled: true, model: 'opus-4.5' },
            finalReview: { enabled: true, model: 'opus-4.5' },
            commit: { enabled: true, model: 'sonnet-4.5' },
          },
        } as unknown,
        9
      ) as unknown as Record<string, unknown>;

      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      const stages = agentConfigs.claude.workflowStages as Record<string, { enabled: boolean; model: string }>;
      expect(stages.branchGen.model).toBe('opus-4.5');
      expect(migrated.workflowStages).toBeUndefined();
    });

    it('adds diagnosticModel default', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { workflowStages: {} } as unknown,
        9
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.diagnosticModel).toBe('sonnet-4.6');
      expect(migrated.diagnosticModel).toBeUndefined();
    });

    it('does not overwrite existing diagnosticModel', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { workflowStages: {}, diagnosticModel: 'opus-4.6' } as unknown,
        9
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.diagnosticModel).toBe('opus-4.6');
      expect(migrated.diagnosticModel).toBeUndefined();
    });

    it('full migration from v0 includes branchGen and diagnosticModel', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { plannerModel: 'default', plannerTimeoutMinutes: 5 } as unknown,
        0
      ) as unknown as Record<string, unknown>;

      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.diagnosticModel).toBe('sonnet-4.6');
      const stages = agentConfigs.claude.workflowStages as Record<string, { enabled: boolean; model: string }>;
      expect(stages.branchGen).toEqual({ enabled: true, model: 'sonnet-4.6' });
      expect(migrated.diagnosticModel).toBeUndefined();
      expect(migrated.workflowStages).toBeUndefined();
    });
  });

  describe('workflow migration v10->v11 (sonnet-4.5 to sonnet-4.6)', () => {
    it('upgrades validationModel from sonnet-4.5 to sonnet-4.6', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { validationModel: 'sonnet-4.5', diagnosticModel: 'opus-4.6' } as unknown,
        10
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.validationModel).toBe('sonnet-4.6');
      expect(agentConfigs.claude.diagnosticModel).toBe('opus-4.6');
      expect(migrated.validationModel).toBeUndefined();
      expect(migrated.diagnosticModel).toBeUndefined();
    });

    it('upgrades diagnosticModel from sonnet-4.5 to sonnet-4.6', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { diagnosticModel: 'sonnet-4.5' } as unknown,
        10
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.diagnosticModel).toBe('sonnet-4.6');
      expect(migrated.diagnosticModel).toBeUndefined();
    });

    it('upgrades sonnet-4.5 in workflow stages to sonnet-4.6', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          workflowStages: {
            branchGen: { enabled: true, model: 'sonnet-4.5' },
            plan: { enabled: true, model: 'opus-4.6' },
            implement: { enabled: true, model: 'opus-4.5' },
            commit: { enabled: true, model: 'sonnet-4.5' },
          },
        } as unknown,
        10
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      const stages = agentConfigs.claude.workflowStages as Record<string, { enabled: boolean; model: string }>;
      expect(stages.branchGen.model).toBe('sonnet-4.6');
      expect(stages.plan.model).toBe('opus-4.6');
      expect(stages.implement.model).toBe('opus-4.5');
      expect(stages.commit.model).toBe('sonnet-4.6');
      expect(migrated.workflowStages).toBeUndefined();
    });

    it('does not touch non-sonnet-4.5 models', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { validationModel: 'opus-4.6', diagnosticModel: 'opus-4.5' } as unknown,
        10
      ) as unknown as Record<string, unknown>;
      // v12: moves to agentConfigs.claude
      const agentConfigs = migrated.agentConfigs as Record<string, any>;
      expect(agentConfigs.claude.validationModel).toBe('opus-4.6');
      expect(agentConfigs.claude.diagnosticModel).toBe('opus-4.5');
      expect(migrated.validationModel).toBeUndefined();
      expect(migrated.diagnosticModel).toBeUndefined();
    });
  });

  describe('generic agentSettings', () => {
    beforeEach(() => {
      useSettingsStore.getState().setAgentSettings('claude', {
        authToken: '',
        apiKey: '',
        baseUrl: '',
        modelOverride: '',
        thinkingEnabled: true,
        extendedContext: false,
        chromeEnabled: false,
      });
    });

    it('has claude defaults in initial agentSettings', () => {
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings).toBeDefined();
      expect(claudeSettings.thinkingEnabled).toBe(true);
      expect(claudeSettings.extendedContext).toBe(false);
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

  describe('generic agent settings API', () => {
    it('setAgentSetting updates agentSettings map', () => {
      useSettingsStore.getState().setAgentSetting('claude', 'authToken', 'tok');
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings.authToken).toBe('tok');
    });

    it('setAgentSettings merges with existing settings', () => {
      useSettingsStore.getState().setAgentSettings('claude', {
        authToken: 'new-tok',
        thinkingEnabled: false,
      });
      const claudeSettings = useSettingsStore.getState().getAgentSettings('claude');
      expect(claudeSettings.authToken).toBe('new-tok');
      expect(claudeSettings.thinkingEnabled).toBe(false);
      // Existing defaults should be preserved
      expect(claudeSettings.chromeEnabled).toBe(false);
    });

    it('persists agentConfigs', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const full = useSettingsStore.getState();
      const persisted = options.partialize ? options.partialize(full) as unknown as Record<string, unknown> : full as unknown as Record<string, unknown>;
      expect(persisted).toHaveProperty('agentConfigs');
    });
  });

  describe('migration v8->v9->v12 (Claude fields to agentConfigs)', () => {
    it('migrates v8 Claude fields into agentConfigs map', () => {
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

      const agentConfigs = migrated.agentConfigs as Record<string, { settings: Record<string, unknown> }>;
      expect(agentConfigs.claude.settings.authToken).toBe('tok');
      expect(agentConfigs.claude.settings.apiKey).toBe('key');
      expect(agentConfigs.claude.settings.baseUrl).toBe('https://api.example.com');
      expect(agentConfigs.claude.settings.modelOverride).toBe('model');
      expect(agentConfigs.claude.settings.thinkingEnabled).toBe(false);
      expect(agentConfigs.claude.settings.extendedContext).toBe(true);
      expect(agentConfigs.claude.settings.chromeEnabled).toBe(true);
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
      // agentSettings should be removed by v12
      expect(migrated.agentSettings).toBeUndefined();
    });

    it('sets defaults for missing boolean fields', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { claudeAuthToken: 'tok' } as unknown,
        8
      ) as unknown as Record<string, unknown>;

      const agentConfigs = migrated.agentConfigs as Record<string, { settings: Record<string, unknown> }>;
      expect(agentConfigs.claude.settings.thinkingEnabled).toBe(true);
      expect(agentConfigs.claude.settings.extendedContext).toBe(false);
      expect(agentConfigs.claude.settings.chromeEnabled).toBe(false);
    });

    it('full migration from v7 runs v8 then v9 then v12 in correct order', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        { claudeAuthToken: 'tok' } as unknown,
        7
      ) as unknown as Record<string, unknown>;

      // v8 adds CLI defaults, then v9 moves everything to agentSettings, then v12 moves to agentConfigs
      const agentConfigs = migrated.agentConfigs as Record<string, { settings: Record<string, unknown> }>;
      expect(agentConfigs.claude.settings.authToken).toBe('tok');
      expect(agentConfigs.claude.settings.thinkingEnabled).toBe(true);
      expect(agentConfigs.claude.settings.extendedContext).toBe(false);
      expect(agentConfigs.claude.settings.chromeEnabled).toBe(false);
      // Legacy fields should be cleaned up
      expect(migrated.claudeAuthToken).toBeUndefined();
      expect(migrated.claudeThinkingEnabled).toBeUndefined();
      // agentSettings should be removed by v12
      expect(migrated.agentSettings).toBeUndefined();
    });

    it('full migration from v0 produces agentConfigs', () => {
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

      // Flat planner fields should be moved to agentConfigs by v12
      const agentConfigs = migrated.agentConfigs as Record<string, { plannerModel: string; settings: Record<string, unknown> }>;
      expect(agentConfigs.claude.plannerModel).toBe('opus-4.5');
      expect(agentConfigs.claude.settings.authToken).toBe('old-tok');
      expect(agentConfigs.claude.settings.thinkingEnabled).toBe(true);
      // Flat fields should be removed by v12
      expect(migrated.plannerModel).toBeUndefined();
    });
  });
});
