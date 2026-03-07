import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useSettingsStore, WORKFLOW_STAGE_INFO, DEFAULT_STAGE_ORDER, REQUIRED_STAGE_KEYS, BUILTIN_CATALOG_COMMANDS } from './settingsStore';

vi.mock('../lib/tauri', () => ({
  syncAgentConfigs: vi.fn().mockResolvedValue(undefined),
  setNotificationsEnabled: vi.fn().mockResolvedValue(undefined),
  listCursorModels: vi.fn().mockResolvedValue({ models: [], currentModel: null, defaultModel: null }),
}));

import { listCursorModels } from '../lib/tauri';

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
        plannerModel: 'claude-opus-4-5',
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
      expect(config.plannerModel).toBe('claude-opus-4-5');
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
        useSettingsStore.getState().setAgentConfigStage('claude', 'plan', { model: 'claude-sonnet-4-5' });
        const config = useSettingsStore.getState().getAgentConfig('claude');
        expect(config.workflowStages.plan.model).toBe('claude-sonnet-4-5');
        expect(config.workflowStages.plan.enabled).toBe(true);
      });

      it('disabling a non-required stage marks it as disabled', () => {
        const before = useSettingsStore.getState().getAgentConfig('claude');
        expect(before.workflowStages.deslop).toBeDefined();
        expect(before.workflowStages.deslop.enabled).toBe(true);

        useSettingsStore.getState().setAgentConfigStage('claude', 'deslop', { enabled: false });

        const after = useSettingsStore.getState().getAgentConfig('claude');
        expect(after.workflowStages.deslop).toBeDefined();
        expect(after.workflowStages.deslop.enabled).toBe(false);
      });

      it('disabling a non-existent stage still produces a valid config with model', () => {
        useSettingsStore.getState().setAgentConfigStage('claude', 'ghost-stage' as never, { enabled: false });
        const config = useSettingsStore.getState().getAgentConfig('claude');
        const stage = config.workflowStages['ghost-stage' as never];
        expect(stage).toBeDefined();
        expect(stage.enabled).toBe(false);
        expect(typeof stage.model).toBe('string');
        expect(stage.model.length).toBeGreaterThan(0);
      });

      it('disabling a required stage does NOT remove it', () => {
        useSettingsStore.getState().setAgentConfigStage('claude', 'plan', { enabled: false });
        const config = useSettingsStore.getState().getAgentConfig('claude');
        expect(config.workflowStages.plan).toBeDefined();
        expect(config.stageOrder).toContain('plan');
      });

      it('preserves other stages when updating one', () => {
        const before = { ...useSettingsStore.getState().getAgentConfig('claude').workflowStages };
        useSettingsStore.getState().setAgentConfigStage('claude', 'commit', { model: 'claude-sonnet-4-5' });
        const after = useSettingsStore.getState().getAgentConfig('claude').workflowStages;
        expect(after.plan).toEqual(before.plan);
        expect(after.implement).toEqual(before.implement);
        expect(after.commit.model).toBe('claude-sonnet-4-5');
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

    it('toggleCatalogCommand is a no-op for unknown command ID', () => {
      const catalogBefore = useSettingsStore.getState().commandsCatalog;
      const configBefore = useSettingsStore.getState().getAgentConfig('claude');
      useSettingsStore.getState().toggleCatalogCommand('nonexistent-cmd');
      const catalogAfter = useSettingsStore.getState().commandsCatalog;
      const configAfter = useSettingsStore.getState().getAgentConfig('claude');
      expect(catalogAfter).toEqual(catalogBefore);
      expect(configAfter.stageOrder).toEqual(configBefore.stageOrder);
    });

    it('addCustomCommand is a no-op for duplicate ID', () => {
      useSettingsStore.getState().addCustomCommand({
        id: 'dup-cmd', name: 'Dup', description: 'First', enabled: true, source: 'custom', filename: 'dup.md',
      });
      const countBefore = useSettingsStore.getState().commandsCatalog.length;
      useSettingsStore.getState().addCustomCommand({
        id: 'dup-cmd', name: 'Dup2', description: 'Second', enabled: true, source: 'custom', filename: 'dup2.md',
      });
      expect(useSettingsStore.getState().commandsCatalog.length).toBe(countBefore);
      expect(useSettingsStore.getState().commandsCatalog.find((c) => c.id === 'dup-cmd')!.name).toBe('Dup');
    });

    it('addCustomCommand with enabled:false does not add to agent configs', () => {
      useSettingsStore.getState().addCustomCommand({
        id: 'disabled-cmd', name: 'Disabled', description: 'Off', enabled: false, source: 'custom', filename: 'disabled-cmd.md',
      });
      const catalog = useSettingsStore.getState().commandsCatalog;
      expect(catalog.find((c) => c.id === 'disabled-cmd')).toBeDefined();
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.workflowStages['disabled-cmd']).toBeUndefined();
      expect(config.stageOrder).not.toContain('disabled-cmd');
    });

    it('toggling ON inserts new stage before commit in stageOrder', () => {
      useSettingsStore.getState().toggleCatalogCommand('add-tests');
      const config = useSettingsStore.getState().getAgentConfig('claude');
      const addTestsIdx = config.stageOrder.indexOf('add-tests');
      const commitIdx = config.stageOrder.indexOf('commit');
      expect(addTestsIdx).toBeGreaterThan(-1);
      expect(commitIdx).toBeGreaterThan(-1);
      expect(addTestsIdx).toBeLessThan(commitIdx);
    });

    it('removeCustomCommand also cleans up all agent stageOrder and workflowStages', () => {
      useSettingsStore.getState().addCustomCommand({
        id: 'cleanup-test', name: 'Cleanup Test', description: 'T', enabled: true, source: 'custom', filename: 'cleanup-test.md',
      });
      expect(useSettingsStore.getState().getAgentConfig('claude').stageOrder).toContain('cleanup-test');
      expect(useSettingsStore.getState().getAgentConfig('cursor').stageOrder).toContain('cleanup-test');

      useSettingsStore.getState().removeCustomCommand('cleanup-test');
      expect(useSettingsStore.getState().getAgentConfig('claude').stageOrder).not.toContain('cleanup-test');
      expect(useSettingsStore.getState().getAgentConfig('claude').workflowStages['cleanup-test']).toBeUndefined();
      expect(useSettingsStore.getState().getAgentConfig('cursor').stageOrder).not.toContain('cleanup-test');
    });

    it('toggleCatalogCommand affects all agents including codex', () => {
      const cmd = useSettingsStore.getState().commandsCatalog.find((c) => c.id === 'fix-lint');
      if (cmd?.enabled) {
        useSettingsStore.getState().toggleCatalogCommand('fix-lint');
      }
      useSettingsStore.getState().toggleCatalogCommand('fix-lint');
      const codexConfig = useSettingsStore.getState().getAgentConfig('codex');
      expect(codexConfig.workflowStages['fix-lint']).toBeDefined();
      expect(codexConfig.stageOrder).toContain('fix-lint');
    });

    it('toggling ON never inserts command after commit', () => {
      const disabledCmds = useSettingsStore.getState().commandsCatalog.filter(
        (c) => !c.enabled && c.id !== 'sync-with-main',
      );
      for (const cmd of disabledCmds) {
        useSettingsStore.getState().toggleCatalogCommand(cmd.id);
        const config = useSettingsStore.getState().getAgentConfig('claude');
        const cmdIdx = config.stageOrder.indexOf(cmd.id);
        const commitIdx = config.stageOrder.indexOf('commit');
        expect(cmdIdx).toBeGreaterThan(-1);
        expect(commitIdx).toBeGreaterThan(-1);
        expect(cmdIdx).toBeLessThan(commitIdx);
      }
    });

    it('toggling ON twice is idempotent for stageOrder position', () => {
      const cmd = useSettingsStore.getState().commandsCatalog.find((c) => c.id === 'fix-lint');
      if (cmd?.enabled) {
        useSettingsStore.getState().toggleCatalogCommand('fix-lint');
      }
      useSettingsStore.getState().toggleCatalogCommand('fix-lint');
      const orderAfterFirst = [...useSettingsStore.getState().getAgentConfig('claude').stageOrder];

      useSettingsStore.getState().toggleCatalogCommand('fix-lint');
      useSettingsStore.getState().toggleCatalogCommand('fix-lint');
      const orderAfterSecond = useSettingsStore.getState().getAgentConfig('claude').stageOrder;
      expect(orderAfterSecond).toEqual(orderAfterFirst);
    });

    it('addCustomCommand uses agent-appropriate default model for codex', () => {
      useSettingsStore.getState().addCustomCommand({
        id: 'codex-model-cmd', name: 'Codex Model', description: 'Test', enabled: true, source: 'custom', filename: 'codex-model-cmd.md',
      });
      const codexConfig = useSettingsStore.getState().getAgentConfig('codex');
      expect(codexConfig.workflowStages['codex-model-cmd'].model).toBe('gpt-5.2-codex');
      const claudeConfig = useSettingsStore.getState().getAgentConfig('claude');
      expect(claudeConfig.workflowStages['codex-model-cmd'].model).toBe('claude-sonnet-4-6');
    });

    it('addCustomCommand with enabled:true inserts before commit', () => {
      useSettingsStore.getState().addCustomCommand({
        id: 'custom-before-commit',
        name: 'Custom',
        description: 'Test placement',
        enabled: true,
        source: 'custom',
        filename: 'custom-before-commit.md',
      });
      const config = useSettingsStore.getState().getAgentConfig('claude');
      const customIdx = config.stageOrder.indexOf('custom-before-commit');
      const commitIdx = config.stageOrder.indexOf('commit');
      expect(customIdx).toBeGreaterThan(-1);
      expect(customIdx).toBeLessThan(commitIdx);
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

  describe('cursor models state', () => {
    it('has cursorModels empty by default', () => {
      expect(useSettingsStore.getState().cursorModels).toEqual([]);
    });

    it('has cursorModelsSynced false by default', () => {
      expect(useSettingsStore.getState().cursorModelsSynced).toBe(false);
    });

    it('setCursorModels sets models', () => {
      const models = [
        { value: 'opus-4.6-thinking', label: 'Opus 4.6 (Thinking)' },
        { value: 'sonnet-4.5', label: 'Sonnet 4.5' },
      ];
      useSettingsStore.getState().setCursorModels(models);
      expect(useSettingsStore.getState().cursorModels).toEqual(models);
    });

    it('setCursorModels replaces previous models', () => {
      useSettingsStore.getState().setCursorModels([{ value: 'a', label: 'A' }]);
      useSettingsStore.getState().setCursorModels([{ value: 'b', label: 'B' }]);
      expect(useSettingsStore.getState().cursorModels).toEqual([{ value: 'b', label: 'B' }]);
    });
  });

  describe('syncCursorModels', () => {
    it('first sync with currentModel updates all cursor config models', async () => {
      const mockResult = {
        models: [
          { id: 'opus-4.6-thinking', label: 'Opus (Thinking)', isDefault: true, isCurrent: true },
          { id: 'sonnet-4.5', label: 'Sonnet 4.5', isDefault: false, isCurrent: false },
        ],
        currentModel: 'opus-4.6-thinking',
        defaultModel: 'opus-4.6-thinking',
      };
      vi.mocked(listCursorModels).mockResolvedValueOnce(mockResult);

      useSettingsStore.setState({ cursorModelsSynced: false });
      await useSettingsStore.getState().syncCursorModels();

      const state = useSettingsStore.getState();
      expect(state.cursorModelsSynced).toBe(true);
      expect(state.cursorModels).toHaveLength(2);
      expect(state.cursorModels[0].value).toBe('opus-4.6-thinking');

      const cursorConfig = state.getAgentConfig('cursor');
      expect(cursorConfig.autoPilotModel).toBe('opus-4.6-thinking');
      expect(cursorConfig.plannerModel).toBe('opus-4.6-thinking');
      expect(cursorConfig.generalModel).toBe('opus-4.6-thinking');
      expect(cursorConfig.ticketBuilderModel).toBe('opus-4.6-thinking');
      expect(cursorConfig.validationModel).toBe('opus-4.6-thinking');
      expect(cursorConfig.diagnosticModel).toBe('opus-4.6-thinking');
      for (const stage of Object.values(cursorConfig.workflowStages)) {
        expect(stage.model).toBe('opus-4.6-thinking');
      }
    });

    it('subsequent sync does not overwrite cursor config models', async () => {
      const mockResult = {
        models: [
          { id: 'opus-4.6-thinking', label: 'Opus (Thinking)', isDefault: true, isCurrent: true },
        ],
        currentModel: 'opus-4.6-thinking',
        defaultModel: 'opus-4.6-thinking',
      };
      vi.mocked(listCursorModels).mockResolvedValueOnce(mockResult);

      useSettingsStore.setState({ cursorModelsSynced: true });
      useSettingsStore.getState().updateAgentConfig('cursor', { plannerModel: 'sonnet-4.5' });
      await useSettingsStore.getState().syncCursorModels();

      expect(useSettingsStore.getState().getAgentConfig('cursor').plannerModel).toBe('sonnet-4.5');
    });

    it('first sync without currentModel does not update cursor config models', async () => {
      const mockResult = {
        models: [
          { id: 'auto', label: 'Auto', isDefault: false, isCurrent: false },
        ],
        currentModel: null,
        defaultModel: null,
      };
      vi.mocked(listCursorModels).mockResolvedValueOnce(mockResult);

      useSettingsStore.setState({ cursorModelsSynced: false });
      const configBefore = useSettingsStore.getState().getAgentConfig('cursor');
      await useSettingsStore.getState().syncCursorModels();

      const state = useSettingsStore.getState();
      expect(state.cursorModelsSynced).toBe(true);
      expect(state.cursorModels).toHaveLength(1);
      expect(state.getAgentConfig('cursor').plannerModel).toBe(configBefore.plannerModel);
    });

    it('first sync does not affect other agents', async () => {
      const mockResult = {
        models: [
          { id: 'opus-4.6-thinking', label: 'Opus', isDefault: true, isCurrent: true },
        ],
        currentModel: 'opus-4.6-thinking',
        defaultModel: 'opus-4.6-thinking',
      };
      vi.mocked(listCursorModels).mockResolvedValueOnce(mockResult);

      const claudeConfigBefore = useSettingsStore.getState().getAgentConfig('claude');
      useSettingsStore.setState({ cursorModelsSynced: false });
      await useSettingsStore.getState().syncCursorModels();

      const claudeConfigAfter = useSettingsStore.getState().getAgentConfig('claude');
      expect(claudeConfigAfter.plannerModel).toBe(claudeConfigBefore.plannerModel);
    });
  });

  describe('persist config', () => {
    it('uses version 19', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      expect(options.version).toBe(19);
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

    it('preserves stages that do not need remapping', () => {
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
                deslop: { enabled: true, model: 'opus-4.5' },
                commit: { enabled: true, model: 'sonnet-4.6' },
              },
              stageOrder: ['branchGen', 'plan', 'implement', 'codeReview', 'cleanup', 'deslop', 'commit'],
            },
          },
        } as unknown,
        13
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, any>;
      expect(configs.claude.workflowStages.branchGen).toBeDefined();
      expect(configs.claude.workflowStages.plan).toBeDefined();
      expect(configs.claude.workflowStages.cleanup).toBeDefined();
      expect(configs.claude.workflowStages.deslop).toBeDefined();
      expect(configs.claude.workflowStages.commit).toBeDefined();
    });

    it('handles missing agentConfigs gracefully', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {} as unknown,
        13
      ) as unknown as Record<string, unknown>;

      expect(migrated.commandsCatalog).toBeDefined();
      expect((migrated.commandsCatalog as any[]).length).toBe(BUILTIN_CATALOG_COMMANDS.length);
    });

    it('handles multiple agents in migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              workflowPreset: 'balanced',
              workflowStages: {
                codeReview: { enabled: true, model: 'opus-4.6' },
                unitTests: { enabled: false, model: 'opus-4.5' },
              },
              stageOrder: ['branchGen', 'plan', 'implement', 'codeReview', 'unitTests', 'commit'],
            },
            cursor: {
              workflowPreset: 'balanced',
              workflowStages: {
                codeReview: { enabled: true, model: 'opus-4.5' },
                finalReview: { enabled: true, model: 'opus-4.5' },
              },
              stageOrder: ['branchGen', 'plan', 'implement', 'codeReview', 'finalReview', 'commit'],
            },
          },
        } as unknown,
        13
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, any>;

      expect(configs.claude.workflowStages['code-review']).toBeDefined();
      expect(configs.claude.workflowStages['unit-tests']).toBeDefined();
      expect(configs.claude.stageOrder).toContain('code-review');
      expect(configs.claude.stageOrder).toContain('unit-tests');

      expect(configs.cursor.workflowStages['code-review']).toBeDefined();
      expect(configs.cursor.workflowStages['review-changes']).toBeDefined();
      expect(configs.cursor.stageOrder).toContain('code-review');
      expect(configs.cursor.stageOrder).toContain('review-changes');

      const catalog = migrated.commandsCatalog as Array<{ id: string; enabled: boolean }>;
      const codeReview = catalog.find((c) => c.id === 'code-review');
      expect(codeReview?.enabled).toBe(true);
      const reviewChanges = catalog.find((c) => c.id === 'review-changes');
      expect(reviewChanges?.enabled).toBe(true);
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

  });

  describe('validation agent settings', () => {
    it('has correct default validationModel', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.validationModel).toBe('claude-sonnet-4-6');
    });
  });

  describe('auto-pilot settings', () => {
    it('has autoPilotEnabled false by default for claude', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.autoPilotEnabled).toBe(false);
    });

    it('has autoPilotEnabled false by default for cursor', () => {
      const config = useSettingsStore.getState().getAgentConfig('cursor');
      expect(config.autoPilotEnabled).toBe(false);
    });

    it('has autoPilotEnabled false by default for codex', () => {
      const config = useSettingsStore.getState().getAgentConfig('codex');
      expect(config.autoPilotEnabled).toBe(false);
    });

    it('toggles autoPilotEnabled to true', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { autoPilotEnabled: true });
      expect(useSettingsStore.getState().getAgentConfig('claude').autoPilotEnabled).toBe(true);
    });

    it('toggles autoPilotEnabled back to false', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { autoPilotEnabled: true });
      useSettingsStore.getState().updateAgentConfig('claude', { autoPilotEnabled: false });
      expect(useSettingsStore.getState().getAgentConfig('claude').autoPilotEnabled).toBe(false);
    });

    it('autoPilotEnabled is per-agent', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { autoPilotEnabled: true });
      expect(useSettingsStore.getState().getAgentConfig('claude').autoPilotEnabled).toBe(true);
      expect(useSettingsStore.getState().getAgentConfig('cursor').autoPilotEnabled).toBe(false);
    });

    it('does not affect other config fields when toggling autoPilot', () => {
      const before = useSettingsStore.getState().getAgentConfig('claude');
      useSettingsStore.getState().updateAgentConfig('claude', { autoPilotEnabled: true });
      const after = useSettingsStore.getState().getAgentConfig('claude');
      expect(after.plannerModel).toBe(before.plannerModel);
      expect(after.stageOrder).toEqual(before.stageOrder);
      expect(after.stageTimeoutHours).toBe(before.stageTimeoutHours);
    });
  });

  describe('migration: version < 15 adds autoPilotEnabled', () => {
    it('adds autoPilotEnabled: false to all agents when missing', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              workflowStages: { plan: { enabled: true, model: 'opus-4.6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
            cursor: {
              workflowStages: { plan: { enabled: true, model: 'opus-4.6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
          },
          commandsCatalog: [],
        } as unknown,
        14
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { autoPilotEnabled?: boolean }>;
      expect(configs.claude.autoPilotEnabled).toBe(false);
      expect(configs.cursor.autoPilotEnabled).toBe(false);
    });

    it('preserves existing autoPilotEnabled: true through migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              autoPilotEnabled: true,
              workflowStages: { plan: { enabled: true, model: 'opus-4.6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
          },
          commandsCatalog: [],
        } as unknown,
        14
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { autoPilotEnabled?: boolean }>;
      expect(configs.claude.autoPilotEnabled).toBe(true);
    });
  });

  describe('migration: version < 16 removes thinking settings and resets model sync', () => {
    it('removes thinkingEnabled from cursor settings', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            cursor: {
              workflowStages: { plan: { enabled: true, model: 'opus-4.6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
              settings: { thinkingEnabled: true },
              autoPilotEnabled: false,
            },
          },
          commandsCatalog: [],
          cursorModelsSynced: true,
          cursorModels: [{ value: 'x', label: 'X' }],
        } as unknown,
        15
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { settings?: Record<string, unknown> }>;
      expect(configs.cursor.settings?.thinkingEnabled).toBeUndefined();
      expect(configs.cursor.settings?.thinking_enabled).toBeUndefined();
    });

    it('resets cursorModelsSynced to false', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: { cursor: { settings: {} } },
          commandsCatalog: [],
          cursorModelsSynced: true,
          cursorModels: [{ value: 'x', label: 'X' }],
        } as unknown,
        15
      ) as unknown as Record<string, unknown>;

      expect(migrated.cursorModelsSynced).toBe(false);
      expect(migrated.cursorModels).toEqual([]);
    });

    it('handles missing cursor settings gracefully', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: { claude: { settings: { authToken: 'tok' } } },
          commandsCatalog: [],
        } as unknown,
        15
      ) as unknown as Record<string, unknown>;

      expect(migrated.cursorModelsSynced).toBe(false);
      expect(migrated.cursorModels).toEqual([]);
    });

    it('removes both thinkingEnabled variants from cursor settings', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            cursor: {
              settings: { thinkingEnabled: true, thinking_enabled: false, someOtherSetting: 42 },
            },
          },
          commandsCatalog: [],
        } as unknown,
        15
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { settings?: Record<string, unknown> }>;
      expect(configs.cursor.settings?.thinkingEnabled).toBeUndefined();
      expect(configs.cursor.settings?.thinking_enabled).toBeUndefined();
      expect(configs.cursor.settings?.someOtherSetting).toBe(42);
    });
  });

  describe('sync payload shape (agentConfigs structure)', () => {
    it('agentConfigs include autoPilotEnabled for all agents', () => {
      const state = useSettingsStore.getState();
      for (const agentId of ['claude', 'cursor']) {
        const config = state.getAgentConfig(agentId);
        expect(typeof config.autoPilotEnabled).toBe('boolean');
        expect(config).toHaveProperty('workflowStages');
        expect(config).toHaveProperty('stageOrder');
        expect(config).toHaveProperty('diagnosticModel');
      }
    });

    it('autoPilotEnabled survives updateAgentConfig round-trip', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { autoPilotEnabled: true });
      const config = useSettingsStore.getState().agentConfigs.claude;
      expect(config).toBeDefined();
      expect(config.autoPilotEnabled).toBe(true);
      expect(config.stageOrder).toBeDefined();
      expect(config.workflowStages).toBeDefined();
    });

    it('agentConfigs has stageOrder array for each agent', () => {
      const state = useSettingsStore.getState();
      const config = state.getAgentConfig('claude');
      expect(Array.isArray(config.stageOrder)).toBe(true);
      expect(config.stageOrder.length).toBeGreaterThan(0);
    });
  });

  describe('sync payload includes new model and config fields', () => {
    it('agentConfigs include autoCompleteTickets for all agents', () => {
      const state = useSettingsStore.getState();
      for (const agentId of ['claude', 'cursor', 'codex']) {
        const config = state.getAgentConfig(agentId);
        expect(typeof config.autoCompleteTickets).toBe('boolean');
      }
    });

    it('agentConfigs include generalModel for all agents', () => {
      const state = useSettingsStore.getState();
      for (const agentId of ['claude', 'cursor', 'codex']) {
        const config = state.getAgentConfig(agentId);
        expect(typeof config.generalModel).toBe('string');
        expect(config.generalModel.length).toBeGreaterThan(0);
      }
    });

    it('agentConfigs include plannerModel, ticketBuilderModel, and validationModel for all agents', () => {
      const state = useSettingsStore.getState();
      for (const agentId of ['claude', 'cursor', 'codex']) {
        const config = state.getAgentConfig(agentId);
        expect(typeof config.plannerModel).toBe('string');
        expect(config.plannerModel.length).toBeGreaterThan(0);
        expect(typeof config.ticketBuilderModel).toBe('string');
        expect(config.ticketBuilderModel.length).toBeGreaterThan(0);
        expect(typeof config.validationModel).toBe('string');
        expect(config.validationModel.length).toBeGreaterThan(0);
      }
    });

    it('autoCompleteTickets survives updateAgentConfig round-trip', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { autoCompleteTickets: true });
      const config = useSettingsStore.getState().agentConfigs.claude;
      expect(config.autoCompleteTickets).toBe(true);
      expect(config.stageOrder).toBeDefined();
      expect(config.generalModel).toBeDefined();
    });
  });

  describe('diagnostic agent settings', () => {
    it('has correct default diagnosticModel', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.diagnosticModel).toBe('claude-sonnet-4-6');
    });

    it('sets diagnosticModel', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { diagnosticModel: 'claude-opus-4-5' });
      expect(useSettingsStore.getState().getAgentConfig('claude').diagnosticModel).toBe('claude-opus-4-5');
    });
  });

  describe('generalModel settings', () => {
    beforeEach(() => {
      useSettingsStore.getState().updateAgentConfig('claude', { generalModel: 'claude-opus-4-6' });
      useSettingsStore.getState().updateAgentConfig('cursor', { generalModel: 'claude-opus-4-6' });
    });

    it('has correct default generalModel for claude', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.generalModel).toBe('claude-opus-4-6');
    });

    it('has correct default generalModel for codex', () => {
      const config = useSettingsStore.getState().getAgentConfig('codex');
      expect(config.generalModel).toBe('gpt-5.4');
    });

    it('sets generalModel', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { generalModel: 'claude-opus-4-5' });
      expect(useSettingsStore.getState().getAgentConfig('claude').generalModel).toBe('claude-opus-4-5');
    });

    it('generalModel is per-agent', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { generalModel: 'claude-opus-4-5' });
      expect(useSettingsStore.getState().getAgentConfig('claude').generalModel).toBe('claude-opus-4-5');
      expect(useSettingsStore.getState().getAgentConfig('cursor').generalModel).toBe('claude-opus-4-6');
    });
  });

  describe('ticketBuilderModel settings', () => {
    beforeEach(() => {
      useSettingsStore.getState().updateAgentConfig('claude', { ticketBuilderModel: 'claude-opus-4-5' });
      useSettingsStore.getState().updateAgentConfig('cursor', { ticketBuilderModel: 'claude-opus-4-5' });
    });

    it('has correct default ticketBuilderModel for claude', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.ticketBuilderModel).toBe('claude-opus-4-5');
    });

    it('has correct default ticketBuilderModel for cursor', () => {
      const config = useSettingsStore.getState().getAgentConfig('cursor');
      expect(config.ticketBuilderModel).toBe('claude-opus-4-5');
    });

    it('has correct default ticketBuilderModel for codex', () => {
      const config = useSettingsStore.getState().getAgentConfig('codex');
      expect(config.ticketBuilderModel).toBe('gpt-5.4');
    });

    it('sets ticketBuilderModel', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { ticketBuilderModel: 'claude-sonnet-4-6' });
      expect(useSettingsStore.getState().getAgentConfig('claude').ticketBuilderModel).toBe('claude-sonnet-4-6');
    });

    it('ticketBuilderModel is per-agent', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { ticketBuilderModel: 'claude-sonnet-4-6' });
      expect(useSettingsStore.getState().getAgentConfig('claude').ticketBuilderModel).toBe('claude-sonnet-4-6');
      expect(useSettingsStore.getState().getAgentConfig('cursor').ticketBuilderModel).toBe('claude-opus-4-5');
    });

    it('does not affect other config fields when updating ticketBuilderModel', () => {
      const before = useSettingsStore.getState().getAgentConfig('claude');
      useSettingsStore.getState().updateAgentConfig('claude', { ticketBuilderModel: 'claude-sonnet-4-5' });
      const after = useSettingsStore.getState().getAgentConfig('claude');
      expect(after.plannerModel).toBe(before.plannerModel);
      expect(after.generalModel).toBe(before.generalModel);
      expect(after.validationModel).toBe(before.validationModel);
    });
  });

  describe('auto-complete tickets settings', () => {
    beforeEach(() => {
      useSettingsStore.getState().updateAgentConfig('claude', { autoCompleteTickets: false });
      useSettingsStore.getState().updateAgentConfig('cursor', { autoCompleteTickets: false });
      useSettingsStore.getState().updateAgentConfig('codex', { autoCompleteTickets: false });
    });

    it('has autoCompleteTickets false by default for claude', () => {
      const config = useSettingsStore.getState().getAgentConfig('claude');
      expect(config.autoCompleteTickets).toBe(false);
    });

    it('has autoCompleteTickets false by default for cursor', () => {
      const config = useSettingsStore.getState().getAgentConfig('cursor');
      expect(config.autoCompleteTickets).toBe(false);
    });

    it('has autoCompleteTickets false by default for codex', () => {
      const config = useSettingsStore.getState().getAgentConfig('codex');
      expect(config.autoCompleteTickets).toBe(false);
    });

    it('toggles autoCompleteTickets to true', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { autoCompleteTickets: true });
      expect(useSettingsStore.getState().getAgentConfig('claude').autoCompleteTickets).toBe(true);
    });

    it('autoCompleteTickets is per-agent', () => {
      useSettingsStore.getState().updateAgentConfig('claude', { autoCompleteTickets: true });
      expect(useSettingsStore.getState().getAgentConfig('claude').autoCompleteTickets).toBe(true);
      expect(useSettingsStore.getState().getAgentConfig('cursor').autoCompleteTickets).toBe(false);
    });
  });

  describe('migration: version < 18 adds generalModel', () => {
    it('adds generalModel with correct default for claude and cursor', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              autoPilotEnabled: false,
              workflowStages: { plan: { enabled: true, model: 'claude-opus-4-6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
            cursor: {
              autoPilotEnabled: false,
              workflowStages: { plan: { enabled: true, model: 'claude-opus-4-6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
          },
          commandsCatalog: [],
        } as unknown,
        17
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { generalModel?: string }>;
      expect(configs.claude.generalModel).toBe('claude-opus-4-6');
      expect(configs.cursor.generalModel).toBe('claude-opus-4-6');
    });

    it('adds generalModel with codex default for codex agent', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            codex: {
              autoPilotEnabled: false,
              workflowStages: { plan: { enabled: true, model: 'gpt-5.3-codex' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
          },
          commandsCatalog: [],
        } as unknown,
        17
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { generalModel?: string }>;
      expect(configs.codex.generalModel).toBe('gpt-5.4');
    });

    it('preserves existing generalModel through migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              autoPilotEnabled: false,
              generalModel: 'claude-opus-4-5',
              workflowStages: { plan: { enabled: true, model: 'claude-opus-4-6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
          },
          commandsCatalog: [],
        } as unknown,
        17
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { generalModel?: string }>;
      expect(configs.claude.generalModel).toBe('claude-opus-4-5');
    });
  });

  describe('migration: version < 19 adds autoCompleteTickets', () => {
    it('adds autoCompleteTickets: false to all agents when missing', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              autoPilotEnabled: false,
              workflowStages: { plan: { enabled: true, model: 'claude-opus-4-6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
            cursor: {
              autoPilotEnabled: false,
              workflowStages: { plan: { enabled: true, model: 'claude-opus-4-6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
          },
          commandsCatalog: [],
        } as unknown,
        18
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { autoCompleteTickets?: boolean }>;
      expect(configs.claude.autoCompleteTickets).toBe(false);
      expect(configs.cursor.autoCompleteTickets).toBe(false);
    });

    it('preserves existing autoCompleteTickets: true through migration', () => {
      const { persist } = useSettingsStore;
      const options = persist.getOptions();
      const migrated = options.migrate!(
        {
          agentConfigs: {
            claude: {
              autoPilotEnabled: false,
              autoCompleteTickets: true,
              workflowStages: { plan: { enabled: true, model: 'claude-opus-4-6' } },
              stageOrder: ['branchGen', 'plan', 'implement', 'commit'],
            },
          },
          commandsCatalog: [],
        } as unknown,
        18
      ) as unknown as Record<string, unknown>;

      const configs = migrated.agentConfigs as Record<string, { autoCompleteTickets?: boolean }>;
      expect(configs.claude.autoCompleteTickets).toBe(true);
    });
  });
});
