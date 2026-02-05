import { describe, it, expect, beforeEach } from 'vitest';
import { useSettingsStore } from './settingsStore';

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
        plannerModel: 'opus',
        plannerMaxExplorations: 10,
        plannerTimeoutMinutes: 5,
        plannerMaxRetries: 2,
        codeReviewMaxIterations: 3,
        stageTimeoutMinutes: 30,
        stageMaxRetries: 2,
      });
    });

    it('has correct planner defaults', () => {
      const state = useSettingsStore.getState();
      expect(state.plannerAutoApprove).toBe(false);
      expect(state.plannerModel).toBe('opus');
      expect(state.plannerMaxExplorations).toBe(10);
      expect(state.plannerTimeoutMinutes).toBe(5);
      expect(state.plannerMaxRetries).toBe(2);
      expect(state.codeReviewMaxIterations).toBe(3);
      expect(state.stageTimeoutMinutes).toBe(30);
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
      useSettingsStore.getState().setPlannerModel('opus');
      expect(useSettingsStore.getState().plannerModel).toBe('opus');
    });

    it('sets planner timeout minutes', () => {
      useSettingsStore.getState().setPlannerTimeoutMinutes(10);
      expect(useSettingsStore.getState().plannerTimeoutMinutes).toBe(10);
    });

    it('sets planner max retries', () => {
      useSettingsStore.getState().setPlannerMaxRetries(5);
      expect(useSettingsStore.getState().plannerMaxRetries).toBe(5);
    });

    it('sets stage timeout minutes', () => {
      useSettingsStore.getState().setStageTimeoutMinutes(60);
      expect(useSettingsStore.getState().stageTimeoutMinutes).toBe(60);
    });

    it('sets stage max retries', () => {
      useSettingsStore.getState().setStageMaxRetries(3);
      expect(useSettingsStore.getState().stageMaxRetries).toBe(3);
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
});
