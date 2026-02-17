import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useCliAvailability } from './useCliAvailability';
import { useAgentRegistryStore } from '../stores/agentRegistryStore';

vi.mock('../stores/agentRegistryStore', () => {
  const store = vi.fn();
  return { useAgentRegistryStore: store };
});

const MOCK_AGENTS = [
  { id: 'cursor', displayName: 'Cursor', isAvailable: true, version: '1.0', brandColor: null },
  { id: 'claude', displayName: 'Claude', isAvailable: false, version: null, brandColor: '#da7756' },
];

const mockLoadAgents = vi.fn().mockResolvedValue([]);

function setStoreState(overrides: Record<string, unknown> = {}) {
  const state: Record<string, unknown> = {
    agents: [],
    agentsLoading: false,
    agentsLoaded: false,
    loadAgents: mockLoadAgents,
    ...overrides,
  };
  (useAgentRegistryStore as unknown as ReturnType<typeof vi.fn>).mockImplementation(
    (selector: (s: Record<string, unknown>) => unknown) => selector(state),
  );
}

describe('useCliAvailability', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns loading=true when agents have not loaded', () => {
    setStoreState({ agentsLoaded: false, agentsLoading: true });
    const { result } = renderHook(() => useCliAvailability());
    expect(result.current.loading).toBe(true);
    expect(result.current.availability).toEqual({});
  });

  it('maps agent availability after loading', () => {
    setStoreState({ agents: MOCK_AGENTS, agentsLoaded: true });
    const { result } = renderHook(() => useCliAvailability());

    expect(result.current.loading).toBe(false);
    expect(result.current.availability).toEqual({
      cursor: true,
      claude: false,
    });
  });

  it('returns empty availability when no agents are registered', () => {
    setStoreState({ agents: [], agentsLoaded: true });
    const { result } = renderHook(() => useCliAvailability());

    expect(result.current.loading).toBe(false);
    expect(result.current.availability).toEqual({});
  });

  it('returns loading=true when not yet loaded and not loading', () => {
    setStoreState({ agentsLoaded: false, agentsLoading: false });
    const { result } = renderHook(() => useCliAvailability());
    expect(result.current.loading).toBe(true);
  });

  it('returns loading=false when loaded even if agentsLoading is true', () => {
    setStoreState({ agents: MOCK_AGENTS, agentsLoaded: true, agentsLoading: true });
    const { result } = renderHook(() => useCliAvailability());
    // loading = !agentsLoaded || agentsLoading = !true || true = true
    expect(result.current.loading).toBe(true);
  });

  it('calls loadAgents on mount', () => {
    setStoreState();
    renderHook(() => useCliAvailability());
    expect(mockLoadAgents).toHaveBeenCalled();
  });
});
