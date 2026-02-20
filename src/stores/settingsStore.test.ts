import { describe, it, expect, beforeEach } from 'vitest';
import { useSettingsStore, WORKFLOW_STAGE_INFO, MODEL_OPTIONS, DEFAULT_STAGE_ORDER, REQUIRED_STAGE_KEYS, BUILTIN_CATALOG_COMMANDS } from './settingsStore';

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

    it('has commandsCatalog initialized', () => {
      const catalog = useSettingsStore.getState().commandsCatalog;
      expect(catalog.length).toBeGreaterThan(0);
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

    it('sets stage timeout hours', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { stageTimeoutHours: 2 });
      expect(useSettingsStore.getState().getAgentConfig('claude').stageTimeoutHours).toBe(2);
    });
  });

  describe('workflow settings', () => {
    describe('setAgentConfigStage', () => {
      it('updates a single stage model', () => {
        useSettingsStore.getState().setAgentConfigStage('claude', 'plan', { model: 'sonnet-4.5' });
        const config = useSettingsStore.getState().getAgentConfig('claude');
        expect(config.workflowStages.plan.model).toBe('sonnet-4.5');
        expect(config.workflowStages.plan.enabled).toBe(true);
      });

      it('disabling a non-required stage removes it from stageOrder and workflowStages', () => {
        const before = useSettingsStore.getState().getAgentConfig('claude');
        expect(before.workflowStages.deslop).toBeDefined();
        expect(before.stageOrder).toContain('deslop');

        useSettingsStore.getState().setAgentConfigStage('claude', 'deslop', { enabled: false });

        const after = useSettingsStore.getState().getAgentConfig('claude');
        expect(after.workflowStages.deslop).toBeUndefined();
        expect(after.stageOrder).not.toContain('deslop');
      });

      it('disabling a required stage does NOT remove it', () => {
        useSettingsStore.getState().setAgentConfigStage('claude', 'plan', { enabled: false });
        const config = useSettingsStore.getState().getAgentConfig('claude');
        expect(config.workflowStages.plan).toBeDefined();
        expect(config.stageOrder).toContain('plan');
      });

      it('preserves other stages when updating one', () => {
        const before = { ...useSettingsStore.getState().getAgentConfig('claude').workflowStages };
        useSettingsStore.getState().setAgentConfigStage('claude', 'commit', { model: 'sonnet-4.5' });
        const after = useSettingsStore.getState().getAgentConfig('claude').workflowStages;
        expect(after.plan).toEqual(before.plan);
        expect(after.implement).toEqual(before.implement);
        expect(after.commit.model).toBe('sonnet-4.5');
      });

      it('disabling per-agent does not affect other agents', () => {
        useSettingsStore.getState().setAgentConfigStage('claude', 'cleanup', { enabled: false });
        const cursorConfig = useSettingsStore.getState().getAgentConfig('cursor');
        expect(cursorConfig.workflowStages.cleanup).toBeDefined();
        expect(cursorConfig.stageOrder).toContain('cleanup');
      });
    });
  });

  describe('setAgentConfigStageOrder', () => {
    it('updates stage order', () => {
      const newOrder = [
        'branchGen', 'plan', 'implement',
        'deslop', 'cleanup', 'code-review', 'unit-tests', 'review-changes',
        'commit',
      ];
      useSettingsStore.getState().setAgentConfigStageOrder('claude', newOrder);
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.stageOrder).toEqual(newOrder);
    });

    it('does not affect other agents', () => {
      const newOrder = [
        'branchGen', 'plan', 'implement',
        'deslop', 'code-review', 'cleanup', 'unit-tests', 'review-changes',
        'commit',
      ];
      useSettingsStore.getState().setAgentConfigStageOrder('claude', newOrder);
      const cursorConfig = useSettingsStore.getState().getAgentConfig('cursor');
      expect(cursorConfig.stageOrder).toEqual(DEFAULT_STAGE_ORDER);
    });

    it('stores a copy (not a reference)', () => {
      const order = [...DEFAULT_STAGE_ORDER];
      useSettingsStore.getState().setAgentConfigStageOrder('claude', order);
      order[3] = 'deslop';
      expect(useSettingsStore.getState().getAgentConfig('claude').stageOrder[3]).toBe('code-review');
    });
  });

  describe('catalog commands', () => {
    it('toggleCatalogCommand toggles enabled state', () => {
      const initialState = useSettingsStore.getState().commandsCatalog.find((c) => c.id === 'deslop')!;
      const wasEnabled = initialState.enabled;

      useSettingsStore.getState().toggleCatalogCommand('deslop');
      const after = useSettingsStore.getState().commandsCatalog.find((c) => c.id === 'deslop')!;
      expect(after.enabled).toBe(!wasEnabled);
    });

    it('toggling ON adds stage to all agents', () => {
      useSettingsStore.getState().toggleCatalogCommand('add-tests');
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.workflowStages['add-tests']).toBeDefined();
      expect(config.stageOrder).toContain('add-tests');
    });

    it('toggling OFF removes stage from all agents', () => {
      const catalog = useSettingsStore.getState().commandsCatalog;
      const cmd = catalog.find((c) => c.id === 'add-tests');
      if (!cmd?.enabled) {
        useSettingsStore.getState().toggleCatalogCommand('add-tests');
      }
      useSettingsStore.getState().toggleCatalogCommand('add-tests');
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.workflowStages['add-tests']).toBeUndefined();
      expect(config.stageOrder).not.toContain('add-tests');
    });

    it('catalog toggle OFF removes command from all agents', () => {
      const catalog = useSettingsStore.getState().commandsCatalog;
      const cmd = catalog.find((c) => c.id === 'code-review');
      if (!cmd?.enabled) {
        useSettingsStore.getState().toggleCatalogCommand('code-review');
      }
      expect(useSettingsStore.getState().getAgentConfig('claude').workflowStages['code-review']).toBeDefined();

      useSettingsStore.getState().toggleCatalogCommand('code-review');

      const afterCatalog = useSettingsStore.getState().commandsCatalog.find((c) => c.id === 'code-review');
      expect(afterCatalog?.enabled).toBe(false);

      const claudeConfig = useSettingsStore.getState().getAgentConfig('claude');
      expect(claudeConfig.workflowStages['code-review']).toBeUndefined();
      expect(claudeConfig.stageOrder).not.toContain('code-review');

      const cursorConfig = useSettingsStore.getState().getAgentConfig('cursor');
      expect(cursorConfig.workflowStages['code-review']).toBeUndefined();
      expect(cursorConfig.stageOrder).not.toContain('code-review');
    });

    it('catalog toggle OFF then ON restores command to all agents', () => {
      const catalog = useSettingsStore.getState().commandsCatalog;
      const cmd = catalog.find((c) => c.id === 'code-review');
      if (cmd?.enabled) {
        useSettingsStore.getState().toggleCatalogCommand('code-review');
      }
      expect(useSettingsStore.getState().commandsCatalog.find((c) => c.id === 'code-review')?.enabled).toBe(false);

      useSettingsStore.getState().toggleCatalogCommand('code-review');

      const afterConfig = useSettingsStore.getState().getAgentConfig('claude');
      expect(afterConfig.workflowStages['code-review']).toBeDefined();
      expect(afterConfig.stageOrder).toContain('code-review');
    });

    it('addCustomCommand adds a new command', () => {
      useSettingsStore.getState().addCustomCommand({
        id: 'my-cmd',
        name: 'My Command',
        description: 'Test',
        enabled: true,
        source: 'custom',
        filename: 'my-cmd.md',
      });
      const catalog = useSettingsStore.getState().commandsCatalog;
      expect(catalog.find((c) => c.id === 'my-cmd')).toBeDefined();
    });

    it('removeCustomCommand removes a custom command', () => {
      useSettingsStore.getState().addCustomCommand({
        id: 'temp-cmd',
        name: 'Temp',
        description: 'Temporary',
        enabled: true,
        source: 'custom',
        filename: 'temp-cmd.md',
      });
      useSettingsStore.getState().removeCustomCommand('temp-cmd');
      const catalog = useSettingsStore.getState().commandsCatalog;
      expect(catalog.find((c) => c.id === 'temp-cmd')).toBeUndefined();
    });

    it('removeCustomCommand does not remove builtins', () => {
      useSettingsStore.getState().removeCustomCommand('cleanup');
      const catalog = useSettingsStore.getState().commandsCatalog;
      expect(catalog.find((c) => c.id === 'cleanup')).toBeDefined();
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

    it('getAgentSettings returns empty for unknown agent', () => {
      const settings = useSettingsStore.getState().getAgentSettings('unknown');
      expect(settings).toEqual({});
    });
  });

  describe('persist config', () => {
    it('uses version 14', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      expect(options.version).toBe(14);
    });
  });

  describe('persist migration v13->v14', () => {
    it('maps old camelCase stage keys to kebab-case', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              workflowPreset: 'balanced',
              workflowStages: {
                branchGen: { enabled: true, model: 'sonnet-4.6' },
                plan: { enabled: true, model: 'opus-4.6' },
                implement: { enabled: true, model: 'opus-4.6' },
                codeReview: { enabled: true, model: 'opus-4.6' },
                cleanup: { enabled: true, model: 'sonnet-4.6' },
                unitTests: { enabled: true, model: 'opus-4.5' },
                finalReview: { enabled: true, model: 'opus-4.5' },
                deslop: { enabled: true, model: 'opus-4.5' },
                commit: { enabled: true, model: 'sonnet-4.6' },
              },
              stageOrder: ['branchGen', 'plan', 'implement', 'codeReview', 'cleanup', 'unitTests', 'finalReview', 'deslop', 'commit'],
            },
          },
        } as unknown,
        13
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, any>;
      const stages = configs.claude.workflowStages;

      expect(stages['code-review']).toBeDefined();
      expect(stages['unit-tests']).toBeDefined();
      expect(stages['review-changes']).toBeDefined();
      expect(stages.codeReview).toBeUndefined();
      expect(stages.unitTests).toBeUndefined();
      expect(stages.finalReview).toBeUndefined();

      expect(configs.claude.stageOrder).toContain('code-review');
      expect(configs.claude.stageOrder).toContain('unit-tests');
      expect(configs.claude.stageOrder).toContain('review-changes');
      expect(configs.claude.stageOrder).not.toContain('codeReview');
    });

    it('removes workflowPreset from agent configs', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              workflowPreset: 'balanced',
              workflowStages: {},
              stageOrder: [],
            },
          },
        } as unknown,
        13
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, any>;
      expect(configs.claude.workflowPreset).toBeUndefined();
    });

    it('initializes commandsCatalog', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              workflowPreset: 'balanced',
              workflowStages: {
                branchGen: { enabled: true, model: 'sonnet-4.6' },
                plan: { enabled: true, model: 'opus-4.6' },
                implement: { enabled: true, model: 'opus-4.6' },
                codeReview: { enabled: true, model: 'opus-4.6' },
                cleanup: { enabled: false, model: 'sonnet-4.6' },
                unitTests: { enabled: true, model: 'opus-4.5' },
                finalReview: { enabled: false, model: 'opus-4.5' },
                deslop: { enabled: true, model: 'opus-4.5' },
                commit: { enabled: true, model: 'sonnet-4.6' },
              },
              stageOrder: ['branchGen', 'plan', 'implement', 'codeReview', 'cleanup', 'unitTests', 'finalReview', 'deslop', 'commit'],
            },
          },
        } as unknown,
        13
      ) as unknown as Record<string, unknown>;

      const catalog = migrated.commandsCatalog as Array<{ id: string; enabled: boolean }>;
      expect(catalog).toBeDefined();
      expect(catalog.length).toBe(BUILTIN_CATALOG_COMMANDS.length);

      const codeReview = catalog.find((c) => c.id === 'code-review');
      expect(codeReview?.enabled).toBe(true);

      const cleanup = catalog.find((c) => c.id === 'cleanup');
      expect(cleanup?.enabled).toBe(false);

      const unitTests = catalog.find((c) => c.id === 'unit-tests');
      expect(unitTests?.enabled).toBe(true);
    });
  });

  describe('constants', () => {
    it('WORKFLOW_STAGE_INFO only has required stages', () => {
      expect(WORKFLOW_STAGE_INFO).toHaveLength(4);
      for (const info of WORKFLOW_STAGE_INFO) {
        expect(info.required).toBe(true);
      }
    });

    it('REQUIRED_STAGE_KEYS matches WORKFLOW_STAGE_INFO keys', () => {
      for (const info of WORKFLOW_STAGE_INFO) {
        expect(REQUIRED_STAGE_KEYS.has(info.key)).toBe(true);
      }
    });

    it('MODEL_OPTIONS includes all expected models', () => {
      const values = MODEL_OPTIONS.map((o) => o.value);
      expect(values).toContain('opus-4.6');
      expect(values).toContain('opus-4.5');
      expect(values).toContain('sonnet-4.6');
      expect(values).toContain('sonnet-4.5');
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
});
