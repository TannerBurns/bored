import { useState, useEffect, useCallback } from 'react';
import {
  getDashboardSummary,
  getDashboardTrends,
  getModelBreakdown,
  getAgentBreakdown,
  backfillGitStats,
} from '../lib/tauri';
import type {
  DashboardSummary,
  DashboardTrendPoint,
  ModelBreakdownEntry,
  AgentBreakdownEntry,
} from '../types';

export type TimeRange = 7 | 30 | 90 | null;

interface UseDashboardDataResult {
  summary: DashboardSummary | null;
  trends: DashboardTrendPoint[];
  modelBreakdown: ModelBreakdownEntry[];
  agentBreakdown: AgentBreakdownEntry[];
  timeRange: TimeRange;
  setTimeRange: (range: TimeRange) => void;
  isLoading: boolean;
  error: string | null;
  refresh: () => void;
}

export function useDashboardData(): UseDashboardDataResult {
  const [timeRange, setTimeRange] = useState<TimeRange>(30);
  const [summary, setSummary] = useState<DashboardSummary | null>(null);
  const [trends, setTrends] = useState<DashboardTrendPoint[]>([]);
  const [modelBreakdown, setModelBreakdown] = useState<ModelBreakdownEntry[]>([]);
  const [agentBreakdown, setAgentBreakdown] = useState<AgentBreakdownEntry[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      backfillGitStats().catch(() => {});

      const days = timeRange ?? undefined;
      const trendDays = timeRange ?? 90;

      const [summaryData, trendsData, modelsData, agentsData] = await Promise.all([
        getDashboardSummary(days),
        getDashboardTrends(trendDays),
        getModelBreakdown(days),
        getAgentBreakdown(days),
      ]);

      setSummary(summaryData);
      setTrends(trendsData);
      setModelBreakdown(modelsData);
      setAgentBreakdown(agentsData);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, [timeRange]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  return {
    summary,
    trends,
    modelBreakdown,
    agentBreakdown,
    timeRange,
    setTimeRange,
    isLoading,
    error,
    refresh: fetchData,
  };
}
