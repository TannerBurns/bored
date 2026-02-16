import { describe, it, expect, beforeEach, vi } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useClaudeSettings } from './useClaudeSettings';
import * as tauri from '../../../lib/tauri';
import { useSettingsStore } from '../../../stores/settingsStore';

vi.mock('../../../lib/tauri', () => ({
  getAgentStatus: vi.fn(),
  installAgentHooksGlobal: vi.fn(),
  installAgentHooksProject: vi.fn(),
  getAgentHooksConfig: vi.fn(),
  getClaudeApiSettings: vi.fn(),
  setClaudeApiSettings: vi.fn(),
  getProjects: vi.fn(),
  browseForDirectory: vi.fn(),
  getAvailableCommands: vi.fn(),
  installCommandsToUser: vi.fn(),
  installCommandsToProject: vi.fn(),
  checkCommandsInstalled: vi.fn(),
  checkUserCommandsInstalled: vi.fn(),
}));

Object.assign(navigator, {
  clipboard: {
    writeText: vi.fn(() => Promise.resolve()),
  },
});

const mockAgentStatus = {
  isAvailable: true,
  version: '1.2.3',
  hookScriptPath: '/path/to/claude-hook.sh',
  globalHooksInstalled: true,
};

const mockApiSettings = {
  authToken: 'test-token',
  apiKey: 'test-api-key',
  baseUrl: 'https://api.example.com',
  modelOverride: 'claude-opus-4',
  thinkingEnabled: true,
  extendedContextEnabled: false,
  chromeEnabled: false,
};

describe('useClaudeSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useSettingsStore.setState({
      claudeAuthToken: '',
      claudeApiKey: '',
      claudeBaseUrl: '',
      claudeModelOverride: '',
      claudeThinkingEnabled: true,
      claudeExtendedContext: false,
      claudeChromeEnabled: false,
    });
    vi.mocked(tauri.getAgentStatus).mockResolvedValue(mockAgentStatus);
    vi.mocked(tauri.getClaudeApiSettings).mockResolvedValue(mockApiSettings);
    vi.mocked(tauri.setClaudeApiSettings).mockResolvedValue(undefined);
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

    it('loads userHooksInstalled from Claude status', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.userHooksInstalled).toBe(true);
    });

    it('updates settings store with loaded API settings', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));
      // Wait for the effect to run and update the store
      await waitFor(() => {
        const state = useSettingsStore.getState();
        return state.claudeAuthToken === 'test-token';
      });

      const storeState = useSettingsStore.getState();
      expect(storeState.claudeAuthToken).toBe('test-token');
      expect(storeState.claudeApiKey).toBe('test-api-key');
    });

    it('handles null API settings values', async () => {
      vi.mocked(tauri.getClaudeApiSettings).mockResolvedValue({
        authToken: null,
        apiKey: null,
        baseUrl: null,
        modelOverride: null,
        thinkingEnabled: null,
        extendedContextEnabled: null,
        chromeEnabled: null,
      });

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

      expect(tauri.setClaudeApiSettings).toHaveBeenCalledWith({
        authToken: 'new-token',
        apiKey: 'new-api-key',
        baseUrl: 'https://api.example.com',
        modelOverride: 'claude-opus-4',
        thinkingEnabled: true,
        extendedContextEnabled: false,
        chromeEnabled: false,
      });
      expect(result.current.success).toBe('Claude API settings saved successfully!');
    });

    it('converts empty strings to null when saving', async () => {
      vi.mocked(tauri.getClaudeApiSettings).mockResolvedValue({
        authToken: null,
        apiKey: null,
        baseUrl: null,
        modelOverride: null,
        thinkingEnabled: null,
        extendedContextEnabled: null,
        chromeEnabled: null,
      });

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.apiSettings.save();
      });

      expect(tauri.setClaudeApiSettings).toHaveBeenCalledWith({
        authToken: null,
        apiKey: null,
        baseUrl: null,
        modelOverride: null,
        thinkingEnabled: true,
        extendedContextEnabled: false,
        chromeEnabled: false,
      });
    });

    it('sets error on save failure', async () => {
      vi.mocked(tauri.setClaudeApiSettings).mockRejectedValue(new Error('Save failed'));

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.apiSettings.save();
      });

      expect(result.current.error).toContain('Failed to save API settings');
    });

    it('sets saving flag during save', async () => {
      let resolveSave: () => void;
      const savePromise = new Promise<void>((resolve) => {
        resolveSave = resolve;
      });
      vi.mocked(tauri.setClaudeApiSettings).mockReturnValue(savePromise);

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      let savePromiseFromHook: Promise<void>;
      act(() => {
        savePromiseFromHook = result.current.apiSettings.save();
      });

      expect(result.current.apiSettings.saving).toBe(true);

      await act(async () => {
        resolveSave!();
        await savePromiseFromHook;
      });

      expect(result.current.apiSettings.saving).toBe(false);
    });

    it('updates store after successful save', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      // Set new values
      act(() => {
        result.current.apiSettings.setAuthToken('new-saved-token');
      });

      // After save, the hook reloads from backend which updates the store
      await act(async () => {
        await result.current.apiSettings.save();
      });

      // The store should have been updated with the normalized values from getClaudeApiSettings
      const storeState = useSettingsStore.getState();
      expect(storeState.claudeAuthToken).toBe('test-token'); // From mockApiSettings reload
    });

    it('syncs CLI options back to store after save', async () => {
      // Backend will return updated CLI options after save
      vi.mocked(tauri.getClaudeApiSettings)
        .mockResolvedValueOnce(mockApiSettings) // initial load
        .mockResolvedValueOnce({
          ...mockApiSettings,
          thinkingEnabled: false,
          extendedContextEnabled: true,
          chromeEnabled: true,
        }); // post-save reload

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      // Verify initial CLI option state
      expect(result.current.cliOptions.thinkingEnabled).toBe(true);
      expect(result.current.cliOptions.extendedContext).toBe(false);
      expect(result.current.cliOptions.chromeEnabled).toBe(false);

      await act(async () => {
        await result.current.apiSettings.save();
      });

      // CLI options should be updated from the backend response
      expect(result.current.cliOptions.thinkingEnabled).toBe(false);
      expect(result.current.cliOptions.extendedContext).toBe(true);
      expect(result.current.cliOptions.chromeEnabled).toBe(true);

      // Zustand store should also reflect the updated values
      const storeState = useSettingsStore.getState();
      expect(storeState.claudeThinkingEnabled).toBe(false);
      expect(storeState.claudeExtendedContext).toBe(true);
      expect(storeState.claudeChromeEnabled).toBe(true);
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
      vi.mocked(tauri.getClaudeApiSettings).mockResolvedValue({
        ...mockApiSettings,
        thinkingEnabled: false,
        extendedContextEnabled: true,
        chromeEnabled: true,
      });

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      expect(result.current.cliOptions.thinkingEnabled).toBe(false);
      expect(result.current.cliOptions.extendedContext).toBe(true);
      expect(result.current.cliOptions.chromeEnabled).toBe(true);
    });

    it('defaults CLI options when null from backend', async () => {
      vi.mocked(tauri.getClaudeApiSettings).mockResolvedValue({
        ...mockApiSettings,
        thinkingEnabled: null,
        extendedContextEnabled: null,
        chromeEnabled: null,
      });

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

      expect(tauri.setClaudeApiSettings).toHaveBeenCalled();
    });

    it('includes CLI options in the save payload', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        result.current.cliOptions.setExtendedContext(true);
      });

      // The last call should include the CLI option in the payload
      const calls = vi.mocked(tauri.setClaudeApiSettings).mock.calls;
      const lastCall = calls[calls.length - 1]?.[0];
      expect(lastCall).toMatchObject({
        extendedContextEnabled: true,
        thinkingEnabled: true,
        chromeEnabled: false,
      });
    });

    it('sets error when CLI option auto-save fails', async () => {
      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      // Make the next save call fail
      vi.mocked(tauri.setClaudeApiSettings).mockRejectedValueOnce(
        new Error('Network error')
      );

      await act(async () => {
        result.current.cliOptions.setChromeEnabled(true);
      });

      expect(result.current.error).toContain('Failed to save CLI option');
    });

    it('sets saving flag during CLI option toggle', async () => {
      let resolveSave: () => void;
      const savePromise = new Promise<void>((resolve) => {
        resolveSave = resolve;
      });

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      // Make setClaudeApiSettings hang (for initial load it already resolved)
      vi.mocked(tauri.setClaudeApiSettings).mockReturnValueOnce(savePromise);

      let togglePromise: Promise<void> | undefined;
      act(() => {
        togglePromise = result.current.cliOptions.setChromeEnabled(true) as unknown as Promise<void>;
      });

      // saving should be true while the save is in progress
      expect(result.current.cliOptions.saving).toBe(true);

      await act(async () => {
        resolveSave!();
        await togglePromise;
      });

      expect(result.current.cliOptions.saving).toBe(false);
    });
  });

  describe('extends useAgentSettings', () => {
    it('provides hook installation capabilities', async () => {
      vi.mocked(tauri.installAgentHooksGlobal).mockResolvedValue(undefined);

      const { result } = renderHook(() => useClaudeSettings());

      await waitFor(() => expect(result.current.loading).toBe(false));

      await act(async () => {
        await result.current.hookInstall.install();
      });

      expect(tauri.installAgentHooksGlobal).toHaveBeenCalledWith('claude', '/path/to/claude-hook.sh');
      expect(result.current.success).toContain('Hooks installed in user settings');
    });
  });
});
