import { cn } from '../../lib/utils';
import type { AgentRun } from '../../types';

interface SafetyCommit {
  commit_hash?: string;
  created_at?: string;
  target_branch?: string;
  detour_branch?: string;
  merged_to_target?: boolean;
}

export function SafetyCommitNotice({ run, className }: { run: AgentRun; className?: string }) {
  const meta = run.metadata as Record<string, unknown> | undefined;
  if (!meta?.safety_commit) return null;

  const sc = meta.safety_commit as SafetyCommit;
  const isDetour = !!sc.target_branch;
  const hasCommitHash = !!sc.commit_hash;
  const isCleanDetourMerge = isDetour && sc.merged_to_target === true && !hasCommitHash;

  const accentBg = isCleanDetourMerge ? 'bg-emerald-500/10' : 'bg-amber-500/10';
  const accentBorder = isCleanDetourMerge ? 'border-emerald-500/25' : 'border-amber-500/25';
  const accentText = isCleanDetourMerge ? 'text-emerald-400' : 'text-amber-400';
  const accentTextMuted = isCleanDetourMerge ? 'text-emerald-400/80' : 'text-amber-400/80';
  const codeBg = isCleanDetourMerge
    ? 'bg-emerald-500/15 text-emerald-300'
    : 'bg-amber-500/15 text-amber-300';

  return (
    <div className={cn('p-2.5 rounded-lg border', accentBg, accentBorder, className)}>
      <p className={cn('text-xs font-medium flex items-center gap-1.5', accentText)}>
        <svg className="w-3.5 h-3.5 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          {isCleanDetourMerge ? (
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 13l4 4L19 7" />
          ) : (
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5Z" />
          )}
        </svg>
        {isCleanDetourMerge ? 'Merged to target' : 'Changes auto-saved'}
      </p>
      <p className={cn('text-xs mt-1', accentTextMuted)}>
        {isDetour && sc.merged_to_target ? (
          <>
            {hasCommitHash ? 'Changes auto-saved and merged into' : "Agent's work merged into"}{' '}
            <code className={cn('px-1 py-0.5 rounded', codeBg)}>{sc.target_branch}</code>.
          </>
        ) : isDetour && sc.merged_to_target === false ? (
          <>
            {hasCommitHash ? 'Changes auto-saved to branch' : "Agent's work is on branch"}{' '}
            <code className="bg-amber-500/15 px-1 py-0.5 rounded text-amber-300">{sc.detour_branch}</code>.
            {' '}Merge into{' '}
            <code className="bg-amber-500/15 px-1 py-0.5 rounded text-amber-300">{sc.target_branch}</code>
            {' '}manually with{' '}
            <code className="bg-amber-500/15 px-1 py-0.5 rounded text-amber-300">git merge {sc.detour_branch}</code>.
          </>
        ) : (
          <>
            Some changes were not committed by the agent and were automatically saved.
          </>
        )}
        {sc.commit_hash && (
          <span className="ml-1">
            Commit: <code className="bg-amber-500/15 px-1 py-0.5 rounded text-amber-300">{sc.commit_hash}</code>
          </span>
        )}
      </p>
    </div>
  );
}
