import { useEffect, useState, useMemo } from 'react';
import type { AgentRun, AggregatedCost } from '../../../types';
import { getTicketCost, backfillRunCosts } from '../../../lib/tauri';
import { CostSummary } from '../../common/CostBadge';

interface TicketCostSummaryProps {
  ticketId: string;
  agentRuns: AgentRun[];
}

/**
 * Derive a fingerprint from agent runs that changes whenever cost-relevant
 * data changes -- e.g. when a stage finishes and cost metadata is written.
 * This allows the component to refresh cost in real-time as stages complete
 * without requiring a separate event listener.
 */
function useRunsCostFingerprint(agentRuns: AgentRun[]): string {
  return useMemo(() => {
    // Build a fingerprint from: count, statuses, and whether cost data exists
    const parts = agentRuns.map(r => {
      const hasCost = r.metadata && typeof r.metadata === 'object' && 'cost' in r.metadata;
      return `${r.id}:${r.status}:${hasCost ? '1' : '0'}`;
    });
    return parts.join('|');
  }, [agentRuns]);
}

export function TicketCostSummary({ ticketId, agentRuns }: TicketCostSummaryProps) {
  const [cost, setCost] = useState<AggregatedCost | null>(null);
  const [backfillTriggered, setBackfillTriggered] = useState(false);
  const costFingerprint = useRunsCostFingerprint(agentRuns);

  useEffect(() => {
    let cancelled = false;

    async function loadCost() {
      try {
        const ticketCost = await getTicketCost(ticketId);
        if (!cancelled) {
          setCost(ticketCost);

          // If we have runs but no cost data, trigger a backfill
          if (
            !backfillTriggered &&
            agentRuns.length > 0 &&
            ticketCost.runCount === 0
          ) {
            setBackfillTriggered(true);
            try {
              const count = await backfillRunCosts();
              if (count > 0) {
                // Reload cost data after backfill
                const updatedCost = await getTicketCost(ticketId);
                if (!cancelled) {
                  setCost(updatedCost);
                }
              }
            } catch {
              // Backfill is best-effort
            }
          }
        }
      } catch {
        // Cost data is optional, don't show errors
      }
    }

    loadCost();
    return () => { cancelled = true; };
  }, [ticketId, costFingerprint, backfillTriggered, agentRuns.length]);

  if (!cost || (cost.totalCostUsd === 0 && cost.runCount === 0)) {
    return null;
  }

  return <CostSummary cost={cost} className="mb-3" />;
}
