import { useEffect, useRef, useState, useMemo } from 'react';
import type { AgentRun, AggregatedCost } from '../../../types';
import { getTicketCost, backfillRunCosts } from '../../../lib/tauri';
import { CostSummary } from '../../common/CostBadge';

interface TicketCostSummaryProps {
  ticketId: string;
  agentRuns: AgentRun[];
}

function useRunsCostFingerprint(agentRuns: AgentRun[]): string {
  return useMemo(() => {
    const parts = agentRuns.map(r => {
      const hasCost = r.metadata && typeof r.metadata === 'object' && 'cost' in r.metadata;
      return `${r.id}:${r.status}:${hasCost ? '1' : '0'}`;
    });
    return parts.join('|');
  }, [agentRuns]);
}

export function TicketCostSummary({ ticketId, agentRuns }: TicketCostSummaryProps) {
  const [cost, setCost] = useState<AggregatedCost | null>(null);
  const backfilledRunCountRef = useRef<Map<string, number>>(new Map());
  const costFingerprint = useRunsCostFingerprint(agentRuns);

  useEffect(() => {
    let cancelled = false;

    async function loadCost() {
      try {
        const ticketCost = await getTicketCost(ticketId);
        if (!cancelled) {
          setCost(ticketCost);

          const finishedRuns = agentRuns.filter(
            r => r.status === 'finished' || r.status === 'error'
          );
          const lastBackfilledCount =
            backfilledRunCountRef.current.get(ticketId) ?? 0;

          if (
            finishedRuns.length > lastBackfilledCount &&
            finishedRuns.length > 0 &&
            ticketCost.runCount < finishedRuns.length
          ) {
            try {
              const count = await backfillRunCosts();
              if (count > 0) {
                const updatedCost = await getTicketCost(ticketId);
                if (!cancelled) {
                  setCost(updatedCost);
                  // Only mark backfilled once this ticket's runs are all costed
                  if (updatedCost.runCount >= finishedRuns.length) {
                    backfilledRunCountRef.current.set(ticketId, finishedRuns.length);
                  }
                }
              }
              // If count === 0 or this ticket's runs still lack cost data,
              // ref stays unchanged so retry is possible on next trigger
            } catch {
              // Backfill is best-effort; ref not updated so retry is possible
            }
          }
        }
      } catch {
        // Cost data is optional, don't show errors
      }
    }

    loadCost();
    return () => { cancelled = true; };
  }, [ticketId, costFingerprint, agentRuns.length]);

  if (!cost || (cost.totalCostUsd === 0 && cost.runCount === 0)) {
    return null;
  }

  return <CostSummary cost={cost} className="mb-3" />;
}
