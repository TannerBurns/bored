import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useCursorSettings } from './useCursorSettings';

const mockGetAgentStatus = vi.fn();
const mockInstallAgentHooksGlobal = vi.fn();
const mockInstallAgentHooksProject = vi.fn();
const mockGetAgentHooksConfig = vi.fn();

vi.mock('../../../lib/tauri', () => ({
  getAgentStatus: (...args: unknown[]) => mockGetAgentStatus(...args),
  installAgentHooksGlobal: (...args: unknown[]) => mockInstallAgentHooksGlobal(...args),
  installAgentHooksProject: (...args: unknown[]) => mockInstallAgentHooksProject(...args),
  getAgentHooksConfig: (...args: unknown[]) => mockGetAgentHooksConfig(...args),
  getProjects: vi.fn().mockResolvedValue([]),
  browseForDirectory: vi.fn(),
  getAvailableCommands: vi.fn().mockResolvedValue([]),
  installCommandsToUser: vi.fn(),
  installCommandsToProject: vi.fn(),
  checkCommandsInstalled: vi.fn().mockResolvedValue(false),
  checkUserCommandsInstalled: vi.fn().mockResolvedValue(false),
}));

describe('useCursorSettings', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetAgentStatus.mockResolvedValue({
      isAvailable: true,
      version: '0.48.0',
      globalHooksInstalled: true,
      hookScriptPath: '/path/to/hook.js',
    });
  });

  it('fetches cursor-specific globalHooksInstalled status', async () => {
    const { result } = renderHook(() => useCursorSettings());

    await waitFor(() => {
      expect(result.current.globalHooksInstalled).toBe(true);
    });

    // Verifies it called getAgentStatus with 'cursor'
    expect(mockGetAgentStatus).toHaveBeenCalledWith('cursor');
  });

  it('defaults globalHooksInstalled to false before fetch', () => {
    mockGetAgentStatus.mockReturnValue(new Promise(() => {})); // never resolves
    const { result } = renderHook(() => useCursorSettings());
    expect(result.current.globalHooksInstalled).toBe(false);
  });

  it('sets globalHooksInstalled to false when status fetch fails', async () => {
    mockGetAgentStatus.mockRejectedValue(new Error('not available'));
    const { result } = renderHook(() => useCursorSettings());

    // Wait for the effect to settle
    await waitFor(() => {
      expect(mockGetAgentStatus).toHaveBeenCalled();
    });

    // Should remain false (error path calls catch(() => {}))
    expect(result.current.globalHooksInstalled).toBe(false);
  });

  it('spreads base agent settings from useAgentSettings', async () => {
    const { result } = renderHook(() => useCursorSettings());

    await waitFor(() => {
      expect(result.current.globalHooksInstalled).toBe(true);
    });

    // Base properties from useAgentSettings should be present
    expect(result.current).toHaveProperty('loading');
    expect(result.current).toHaveProperty('status');
    expect(result.current).toHaveProperty('hookInstall');
    expect(result.current).toHaveProperty('reload');
  });
});
