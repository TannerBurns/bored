import { cn } from '../../lib/utils';
import type { AgentRun, RunCostData, AggregatedCost } from '../../types';

/** Extract cost data from a run's metadata, returning null if absent. */
export function getRunCost(run: AgentRun): RunCostData | null {
  const meta = run.metadata as Record<string, unknown> | undefined;
  if (!meta) return null;
  const cost = meta.cost;
  if (!cost || typeof cost !== 'object') return null;
  return cost as RunCostData;
}

interface CostBadgeProps {
  cost: RunCostData | AggregatedCost | null | undefined;
  className?: string;
  showTokens?: boolean;
  size?: 'sm' | 'md';
}

function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  if (usd < 1.0) return `$${usd.toFixed(3)}`;
  return `$${usd.toFixed(2)}`;
}

function formatTokens(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}K`;
  return count.toString();
}


/** Derive total cost from the per-model breakdown when available.
 *  This is the single source of truth — the total always equals the
 *  sum of the "By model" values shown in the tooltip. */
export function getTotalCost(cost: RunCostData | AggregatedCost): number {
  const models = 'modelUsage' in cost ? cost.modelUsage :
    'modelTotals' in cost ? cost.modelTotals : null;

  if (models && Object.keys(models).length > 0) {
    return Object.values(models).reduce((sum, m) => sum + (m.costUsd ?? 0), 0);
  }
  return 'totalCostUsd' in cost ? cost.totalCostUsd : 0;
}

function getInputTokens(cost: RunCostData | AggregatedCost): number {
  if ('totalInputTokens' in cost) return cost.totalInputTokens;
  if ('inputTokens' in cost) return cost.inputTokens;
  return 0;
}

function getOutputTokens(cost: RunCostData | AggregatedCost): number {
  if ('totalOutputTokens' in cost) return cost.totalOutputTokens;
  if ('outputTokens' in cost) return cost.outputTokens;
  return 0;
}

export function CostBadge({ cost, className, showTokens = false, size = 'sm' }: CostBadgeProps) {
  if (!cost) return null;

  const totalCost = getTotalCost(cost);
  if (totalCost === 0 && getInputTokens(cost) === 0) return null;

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full font-mono',
        size === 'sm' ? 'px-1.5 py-0.5 text-[10px]' : 'px-2 py-0.5 text-xs',
        'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20',
        className
      )}
      title={buildTooltip(cost)}
    >
      <svg xmlns="http://www.w3.org/2000/svg" width={size === 'sm' ? 10 : 12} height={size === 'sm' ? 10 : 12} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <line x1="12" y1="1" x2="12" y2="23" />
        <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6" />
      </svg>
      {formatCost(totalCost)}
      {showTokens && (
        <span className="opacity-60">
          ({formatTokens(getInputTokens(cost) + getOutputTokens(cost))} tok)
        </span>
      )}
    </span>
  );
}

function buildTooltip(cost: RunCostData | AggregatedCost): string {
  const lines: string[] = [];

  const modelUsage = 'modelUsage' in cost ? cost.modelUsage : 
    'modelTotals' in cost ? cost.modelTotals : {};
  const modelEntries = Object.entries(modelUsage);

  // Derive token counts from the per-model data (same source as cost)
  // so the numbers shown in the tooltip actually explain the total.
  // Fall back to top-level usage fields only when no model data exists.
  let input = 0;
  let output = 0;
  let cacheRead = 0;
  let cacheWrite = 0;

  if (modelEntries.length > 0) {
    for (const [, data] of modelEntries) {
      input += data.inputTokens ?? 0;
      output += data.outputTokens ?? 0;
      cacheRead += data.cacheReadTokens ?? 0;
      cacheWrite += data.cacheCreationTokens ?? 0;
    }
  } else {
    input = getInputTokens(cost);
    output = getOutputTokens(cost);
    cacheRead = 'totalCacheReadTokens' in cost ? cost.totalCacheReadTokens :
      'cacheReadTokens' in cost ? cost.cacheReadTokens : 0;
    cacheWrite = 'totalCacheCreationTokens' in cost ? cost.totalCacheCreationTokens :
      'cacheCreationTokens' in cost ? cost.cacheCreationTokens : 0;
  }

  lines.push(`Input: ${formatTokens(input)} tokens`);
  lines.push(`Output: ${formatTokens(output)} tokens`);
  if (cacheRead > 0) {
    lines.push(`Cache read: ${formatTokens(cacheRead)} tokens`);
  }
  if (cacheWrite > 0) {
    lines.push(`Cache write: ${formatTokens(cacheWrite)} tokens`);
  }

  lines.push(`Total: ${formatCost(getTotalCost(cost))}`);

  if (modelEntries.length > 0) {
    lines.push('');
    lines.push('By model:');
    for (const [model, data] of modelEntries) {
      lines.push(`  ${model}: ${formatCost(data.costUsd)}`);
    }
  }

  if ('runCount' in cost && cost.runCount > 1) {
    lines.push('');
    lines.push(`Across ${cost.runCount} runs`);
  }

  return lines.join('\n');
}

/** Standalone cost summary for ticket modals */
export function CostSummary({ cost, className }: { cost: AggregatedCost | null | undefined; className?: string }) {
  if (!cost || (cost.totalCostUsd === 0 && cost.totalInputTokens === 0)) return null;

  return (
    <div className={cn('flex items-center gap-2 text-xs text-board-text-muted', className)}>
      <span className="font-medium">Total Cost:</span>
      <CostBadge cost={cost} size="md" />
      {cost.runCount > 0 && (
        <span className="opacity-60">
          across {cost.runCount} run{cost.runCount !== 1 ? 's' : ''}
        </span>
      )}
    </div>
  );
}
