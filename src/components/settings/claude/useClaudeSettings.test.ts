import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useClaudeSettings } from './useClaudeSettings';
import * as tauri from '../../../lib/tauri';
import { useSettingsStore } from '../../../stores/settingsStore';

vi.mock('../../../lib/tauri', () => ({
  getAgentStatus: vi.fn(),
  getAgentSettings: vi.fn(),
  setAgentSettings: vi.fn(),
  syncAgentConfigs: vi.fn().mockResolvedValue(undefined),
  getProjects: vi.fn(),
  browseForDirectory: vi.fn(),
  getAvailableCommands: vi.fn(),
  installCommandsToUser: vi.fn(),
  installCommandsToProject: vi.fn(),
  checkCommandsInstalled: vi.fn(),
  checkUserCommandsInstalled: vi.fn(),
}));

const mockAgentStatus = {
  isAvailable: true,
  version: '1.2.3',
};

const mockSettings: Record<string, unknown> = {
  auth_token: 'test-token',
  api_key: 'test-api-key',
  base_url: 'https://api.example.com',
  model_override: 'claude-opus-4',
  thinking_enabled: true,
  extended_context_enabled: false,
  chrome_enabled: false,
};

describe('useClaudeSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(tauri.getAgentStatus).mockResolvedValue(mockAgentStatus);
    vi.mocked(tauri.getAgentSettings).mockResolvedValue({ ...mockSettings });
    vi.mocked(tauri.setAgentSettings).mockResolvedValue(undefined);
    vi.mocked(tauri.getProjects).mockResolvedValue([]);
    vi.mocked(tauri.getAvailableCommands).mockResolvedValue([]);
    vi.mocked(tauri.checkUserCommandsInstalled).mockResolvedValue(false);
  });

  describe('initialization', () => {
    it('loads API settings on mount', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.apiSettings.authToken).toBe('test-token');
      expect(result.current.apiSettings.apiKey).toBe('test-api-key');
      expect(result.current.apiSettings.baseUrl).toBe('https://api.example.com');
      expect(result.current.apiSettings.modelOverride).toBe('claude-opus-4');
    });

    it('updates settings store with loaded API settings', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));
      await waitFor(() => {
        const state = useSettingsStore.getState();
        return (state.getAgentSettings('claude').authToken as string) === 'test-token';
      });

      const storeState = useSettingsStore.getState();
      const claudeSettings = storeState.getAgentSettings('claude');
      expect(claudeSettings.authToken).toBe('test-token');
      expect(claudeSettings.apiKey).toBe('test-api-key');
    });

    it('handles empty API settings values', async () => {
      vi.mocked(tauri.getAgentSettings).mockResolvedValue({});

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.apiSettings.authToken).toBe('');
      expect(result.current.apiSettings.apiKey).toBe('');
    });
  });

  describe('save API settings', () => {
    it('saves API settings successfully', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.apiSettings.setAuthToken('new-token');
        result.current.apiSettings.setApiKey('new-api-key');
      });

      await act(async () => {
        await result.current.apiSettings.save();
      });

      expect(tauri.setAgentSettings).toHaveBeenCalledWith('claude', expect.objectContaining({
        auth_token: 'new-token',
        api_key: 'new-api-key',
      }));
      expect(result.current.success).toBe('Claude API settings saved successfully!');
    });

    it('converts empty strings to null when saving', async () => {
      vi.mocked(tauri.getAgentSettings).mockResolvedValue({});

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.apiSettings.save();
      });

      expect(tauri.setAgentSettings).toHaveBeenCalledWith('claude', expect.objectContaining({
        auth_token: null,
        api_key: null,
        base_url: null,
        model_override: null,
      }));
    });

    it('sets error on save failure', async () => {
      vi.mocked(tauri.setAgentSettings).mockRejectedValue(new Error('Save failed'));

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.apiSettings.save();
      });

      expect(result.current.error).toContain('Failed to save API settings');
    });
  });

  describe('API settings setters', () => {
    it('allows setting individual API fields', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      act(() => {
        result.current.apiSettings.setAuthToken('new-auth');
        result.current.apiSettings.setApiKey('new-key');
        result.current.apiSettings.setBaseUrl('https://new.com');
        result.current.apiSettings.setModelOverride('new-model');
      });

      expect(result.current.apiSettings.authToken).toBe('new-auth');
      expect(result.current.apiSettings.apiKey).toBe('new-key');
      expect(result.current.apiSettings.baseUrl).toBe('https://new.com');
      expect(result.current.apiSettings.modelOverride).toBe('new-model');
    });
  });

  describe('CLI options', () => {
    it('loads CLI options from backend on mount', async () => {
      vi.mocked(tauri.getAgentSettings).mockResolvedValue({
        ...mockSettings,
        thinking_enabled: false,
        extended_context_enabled: true,
        chrome_enabled: true,
      });

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.cliOptions.thinkingEnabled).toBe(false);
      expect(result.current.cliOptions.extendedContext).toBe(true);
      expect(result.current.cliOptions.chromeEnabled).toBe(true);
    });

    it('defaults CLI options when missing from backend', async () => {
      vi.mocked(tauri.getAgentSettings).mockResolvedValue({});

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.cliOptions.thinkingEnabled).toBe(true);
      expect(result.current.cliOptions.extendedContext).toBe(false);
      expect(result.current.cliOptions.chromeEnabled).toBe(false);
    });

    it('auto-saves when toggling a CLI option', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        result.current.cliOptions.setChromeEnabled(true);
      });

      expect(tauri.setAgentSettings).toHaveBeenCalledWith('claude', expect.objectContaining({
        chrome_enabled: expect.any(Boolean),
      }));
    });

    it('sets error when CLI option auto-save fails', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      vi.mocked(tauri.setAgentSettings).mockRejectedValueOnce(
        new Error('Network error')
      );

      await act(async () => {
        result.current.cliOptions.setChromeEnabled(true);
      });

      expect(result.current.error).toContain('Failed to save CLI option');
    });
  });
});
