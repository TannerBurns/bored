import { cn } from '../../lib/utils';
import type { RunCostData, AggregatedCost } from '../../types';

interface CostBadgeProps {
  cost: RunCostData | AggregatedCost | null | undefined;
  className?: string;
  showTokens?: boolean;
  size?: 'sm' | 'md';
}

/** Format USD cost as a string */
function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(4)}`;
  if (usd < 1.0) return `$${usd.toFixed(3)}`;
  return `$${usd.toFixed(2)}`;
}

/** Format token count with K/M suffix */
function formatTokens(count: number): string {
  if (count >= 1_000_000) return `${(count / 1_000_000).toFixed(1)}M`;
  if (count >= 1_000) return `${(count / 1_000).toFixed(1)}K`;
  return count.toString();
}

/** Check if the cost data has any estimated values */
function isEstimated(cost: RunCostData | AggregatedCost): boolean {
  if ('isEstimated' in cost) return cost.isEstimated;
  if ('estimatedCount' in cost) return cost.estimatedCount > 0;
  return false;
}

/** Get total cost USD from either type */
function getTotalCost(cost: RunCostData | AggregatedCost): number {
  return 'totalCostUsd' in cost ? cost.totalCostUsd : 0;
}

/** Get total input tokens from either type */
function getInputTokens(cost: RunCostData | AggregatedCost): number {
  if ('totalInputTokens' in cost) return cost.totalInputTokens;
  if ('inputTokens' in cost) return cost.inputTokens;
  return 0;
}

/** Get total output tokens from either type */
function getOutputTokens(cost: RunCostData | AggregatedCost): number {
  if ('totalOutputTokens' in cost) return cost.totalOutputTokens;
  if ('outputTokens' in cost) return cost.outputTokens;
  return 0;
}

export function CostBadge({ cost, className, showTokens = false, size = 'sm' }: CostBadgeProps) {
  if (!cost) return null;

  const totalCost = getTotalCost(cost);
  if (totalCost === 0 && getInputTokens(cost) === 0) return null;

  const estimated = isEstimated(cost);
  const costStr = estimated ? `~${formatCost(totalCost)}` : formatCost(totalCost);

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 rounded-full font-mono',
        size === 'sm' ? 'px-1.5 py-0.5 text-[10px]' : 'px-2 py-0.5 text-xs',
        estimated
          ? 'bg-amber-500/10 text-amber-400 border border-amber-500/20'
          : 'bg-emerald-500/10 text-emerald-400 border border-emerald-500/20',
        className
      )}
      title={buildTooltip(cost, estimated)}
    >
      <svg xmlns="http://www.w3.org/2000/svg" width={size === 'sm' ? 10 : 12} height={size === 'sm' ? 10 : 12} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <line x1="12" y1="1" x2="12" y2="23" />
        <path d="M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6" />
      </svg>
      {costStr}
      {showTokens && (
        <span className="opacity-60">
          ({formatTokens(getInputTokens(cost) + getOutputTokens(cost))} tok)
        </span>
      )}
    </span>
  );
}

function buildTooltip(cost: RunCostData | AggregatedCost, estimated: boolean): string {
  const lines: string[] = [];

  if (estimated) {
    lines.push('Estimated cost (Cursor does not report token usage)');
    lines.push('');
  }

  const input = getInputTokens(cost);
  const output = getOutputTokens(cost);

  lines.push(`Input: ${formatTokens(input)} tokens`);
  lines.push(`Output: ${formatTokens(output)} tokens`);

  if ('cacheReadTokens' in cost && cost.cacheReadTokens > 0) {
    lines.push(`Cache read: ${formatTokens(cost.cacheReadTokens)} tokens`);
  }
  if ('totalCacheReadTokens' in cost && cost.totalCacheReadTokens > 0) {
    lines.push(`Cache read: ${formatTokens(cost.totalCacheReadTokens)} tokens`);
  }

  lines.push(`Total: ${formatCost(getTotalCost(cost))}`);

  // Model breakdown
  const modelUsage = 'modelUsage' in cost ? cost.modelUsage : 
    'modelTotals' in cost ? cost.modelTotals : {};
  
  if (Object.keys(modelUsage).length > 0) {
    lines.push('');
    lines.push('By model:');
    for (const [model, data] of Object.entries(modelUsage)) {
      lines.push(`  ${model}: ${formatCost(data.costUsd)}`);
    }
  }

  if ('runCount' in cost && cost.runCount > 1) {
    lines.push('');
    lines.push(`Across ${cost.runCount} runs`);
    if (cost.estimatedCount > 0) {
      lines.push(`(${cost.estimatedCount} estimated)`);
    }
  }

  return lines.join('\n');
}

/** Standalone cost summary for ticket modals */
export function CostSummary({ cost, className }: { cost: AggregatedCost | null | undefined; className?: string }) {
  if (!cost || (cost.totalCostUsd === 0 && cost.totalInputTokens === 0)) return null;

  const estimated = cost.estimatedCount > 0;

  return (
    <div className={cn('flex items-center gap-2 text-xs text-board-text-muted', className)}>
      <span className="font-medium">Total Cost:</span>
      <CostBadge cost={cost} size="md" showTokens />
      {cost.runCount > 0 && (
        <span className="opacity-60">
          across {cost.runCount} run{cost.runCount !== 1 ? 's' : ''}
          {estimated && ` (${cost.estimatedCount} estimated)`}
        </span>
      )}
    </div>
  );
}
