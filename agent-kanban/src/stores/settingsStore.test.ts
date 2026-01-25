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
});
