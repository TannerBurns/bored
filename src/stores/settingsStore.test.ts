import { describe, it, expect, beforeEach } from 'vitest';
import { useSettingsStore } from './settingsStore';

describe('useSettingsStore', () => {
  beforeEach(() => {
    useSettingsStore.setState({
      theme: 'dark',
      defaultAgentPref: 'any',
    });
  });

  describe('initial state', () => {
    it('has dark theme by default', () => {
      expect(useSettingsStore.getState().theme).toBe('dark');
    });

    it('has any agent preference by default', () => {
      expect(useSettingsStore.getState().defaultAgentPref).toBe('any');
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

  describe('setDefaultAgentPref', () => {
    it('sets preference to cursor', () => {
      useSettingsStore.getState().setDefaultAgentPref('cursor');
      expect(useSettingsStore.getState().defaultAgentPref).toBe('cursor');
    });

    it('sets preference to claude', () => {
      useSettingsStore.getState().setDefaultAgentPref('claude');
      expect(useSettingsStore.getState().defaultAgentPref).toBe('claude');
    });

    it('sets preference to any', () => {
      useSettingsStore.getState().setDefaultAgentPref('cursor');
      useSettingsStore.getState().setDefaultAgentPref('any');
      expect(useSettingsStore.getState().defaultAgentPref).toBe('any');
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
      useSettingsStore.getState().setClaudeModelOverride('claude-opus-4-5');
      expect(useSettingsStore.getState().claudeModelOverride).toBe('claude-opus-4-5');
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
