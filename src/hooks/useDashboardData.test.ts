import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import type { DashboardSummary, DashboardTrendPoint } from '../types';

const mockGetDashboardSummary = vi.fn();
const mockGetDashboardTrends = vi.fn();
const mockGetModelBreakdown = vi.fn();
const mockGetAgentBreakdown = vi.fn();
const mockBackfillGitStats = vi.fn();

vi.mock('../lib/tauri', () => ({
  getDashboardSummary: (...args: unknown[]) => mockGetDashboardSummary(...args),
  getDashboardTrends: (...args: unknown[]) => mockGetDashboardTrends(...args),
  getModelBreakdown: (...args: unknown[]) => mockGetModelBreakdown(...args),
  getAgentBreakdown: (...args: unknown[]) => mockGetAgentBreakdown(...args),
  backfillGitStats: (...args: unknown[]) => mockBackfillGitStats(...args),
}));

import { useDashboardData } from './useDashboardData';

const makeSummary = (): DashboardSummary => ({
  ticketsCompleted: 5,
  tasksCompleted: 12,
  totalRuns: 20,
  successfulRuns: 18,
  successRate: 0.9,
  avgRunDurationSecs: 45.5,
  totalCostUsd: 1.23,
  totalInputTokens: 5000,
  totalOutputTokens: 2000,
  totalCacheReadTokens: 1000,
  totalCommits: 10,
  totalPrs: 3,
  totalLinesAdded: 500,
  totalLinesRemoved: 100,
  avgCycleTimeHours: 2.5,
});

const makeTrends = (): DashboardTrendPoint[] => [
  { date: '2025-01-01', ticketsCompleted: 1, tasksCompleted: 3, costUsd: 0.5, tokensUsed: 1000, runs: 2, commits: 1, linesAdded: 50, linesRemoved: 10 },
];

describe('useDashboardData', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetDashboardSummary.mockResolvedValue(makeSummary());
    mockGetDashboardTrends.mockResolvedValue(makeTrends());
    mockGetModelBreakdown.mockResolvedValue([]);
    mockGetAgentBreakdown.mockResolvedValue([]);
    mockBackfillGitStats.mockResolvedValue(0);
  });

  it('starts with isLoading true', () => {
    const { result } = renderHook(() => useDashboardData());
    expect(result.current.isLoading).toBe(true);
  });

  it('loads data and sets summary', async () => {
    const { result } = renderHook(() => useDashboardData());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.summary).toEqual(makeSummary());
    expect(result.current.trends).toEqual(makeTrends());
    expect(result.current.error).toBeNull();
  });

  it('defaults timeRange to 30', () => {
    const { result } = renderHook(() => useDashboardData());
    expect(result.current.timeRange).toBe(30);
  });

  it('refetches when timeRange changes', async () => {
    const { result } = renderHook(() => useDashboardData());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    act(() => {
      result.current.setTimeRange(7);
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(mockGetDashboardSummary).toHaveBeenCalledTimes(2);
  });

  it('sets error on fetch failure', async () => {
    mockGetDashboardSummary.mockRejectedValueOnce(new Error('network error'));

    const { result } = renderHook(() => useDashboardData());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.error).toBe('network error');
    expect(result.current.summary).toBeNull();
  });

  it('exposes a refresh function', async () => {
    const { result } = renderHook(() => useDashboardData());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    await act(async () => {
      result.current.refresh();
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(mockGetDashboardSummary).toHaveBeenCalledTimes(2);
  });

  it('passes null to getDashboardSummary when timeRange is null', async () => {
    const { result } = renderHook(() => useDashboardData());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    act(() => {
      result.current.setTimeRange(null);
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(mockGetDashboardSummary).toHaveBeenLastCalledWith(null);
  });

  it('uses 90 as fallback trend days when timeRange is null', async () => {
    const { result } = renderHook(() => useDashboardData());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    act(() => {
      result.current.setTimeRange(null);
    });

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(mockGetDashboardTrends).toHaveBeenLastCalledWith(90);
  });

  it('awaits backfillGitStats before fetching dashboard data', async () => {
    const callOrder: string[] = [];
    mockBackfillGitStats.mockImplementation(
      () => new Promise<number>((resolve) => {
        callOrder.push('backfill');
        resolve(0);
      }),
    );
    mockGetDashboardSummary.mockImplementation(
      () => new Promise((resolve) => {
        callOrder.push('summary');
        resolve(makeSummary());
      }),
    );

    const { result } = renderHook(() => useDashboardData());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    const backfillIdx = callOrder.indexOf('backfill');
    const summaryIdx = callOrder.indexOf('summary');
    expect(backfillIdx).toBeGreaterThanOrEqual(0);
    expect(summaryIdx).toBeGreaterThanOrEqual(0);
    expect(backfillIdx).toBeLessThan(summaryIdx);
  });

  it('still loads data when backfillGitStats rejects', async () => {
    mockBackfillGitStats.mockRejectedValue(new Error('git error'));

    const { result } = renderHook(() => useDashboardData());

    await waitFor(() => {
      expect(result.current.isLoading).toBe(false);
    });

    expect(result.current.summary).toEqual(makeSummary());
    expect(result.current.error).toBeNull();
  });
});
