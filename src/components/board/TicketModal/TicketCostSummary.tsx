import { useEffect, useState } from 'react';
import type { AgentRun, AggregatedCost } from '../../../types';
import { getTicketCost, backfillRunCosts } from '../../../lib/tauri';
import { CostSummary } from '../../common/CostBadge';

interface TicketCostSummaryProps {
  ticketId: string;
  agentRuns: AgentRun[];
}

export function TicketCostSummary({ ticketId, agentRuns }: TicketCostSummaryProps) {
  const [cost, setCost] = useState<AggregatedCost | null>(null);
  const [backfillTriggered, setBackfillTriggered] = useState(false);

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
  }, [ticketId, agentRuns.length, backfillTriggered]);

  if (!cost || (cost.totalCostUsd === 0 && cost.runCount === 0)) {
    return null;
  }

  return <CostSummary cost={cost} className="mb-3" />;
}
