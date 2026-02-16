import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useCliAvailability } from './useCliAvailability';

const mockGetAvailableAgents = vi.fn();
vi.mock('../lib/tauri', () => ({
  getAvailableAgents: (...args: unknown[]) => mockGetAvailableAgents(...args),
}));

const MOCK_AGENTS = [
  { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0', brandColor: null },
  { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756' },
];

describe('useCliAvailability', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns loading=true initially', () => {
    mockGetAvailableAgents.mockReturnValue(new Promise(() => {})); // never resolves
    const { result } = renderHook(() => useCliAvailability());
    expect(result.current.loading).toBe(true);
    expect(result.current.availability).toEqual({});
  });

  it('maps agent availability after loading', async () => {
    mockGetAvailableAgents.mockResolvedValue(MOCK_AGENTS);
    const { result } = renderHook(() => useCliAvailability());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.availability).toEqual({
      cursor: true,
      claude: false,
    });
  });

  it('returns empty availability when no agents are registered', async () => {
    mockGetAvailableAgents.mockResolvedValue([]);
    const { result } = renderHook(() => useCliAvailability());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.availability).toEqual({});
  });

  it('returns empty availability on error', async () => {
    mockGetAvailableAgents.mockRejectedValue(new Error('network error'));
    const { result } = renderHook(() => useCliAvailability());

    await waitFor(() => expect(result.current.loading).toBe(false));

    expect(result.current.availability).toEqual({});
  });
});
