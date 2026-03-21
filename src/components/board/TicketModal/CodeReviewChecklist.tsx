import { useState, useMemo } from 'react';
import { cn } from '../../../lib/utils';
import type { AgentRun, CodeReviewIssue } from '../../../types';
import { CostBadge, aggregateRunCosts } from '../../common/CostBadge';

export interface CodeReviewIteration {
  iteration: number;
  issuesFound: number | null;
  issuesSection: string;
  issues: CodeReviewIssue[];
  status: 'running' | 'finished' | 'error' | 'pending';
  reviewSubRun?: AgentRun;
  fixSubRun?: AgentRun;
}

interface CodeReviewChecklistProps {
  iterations: CodeReviewIteration[];
}

function isFixRunning(iteration: CodeReviewIteration): boolean {
  return !!iteration.fixSubRun && iteration.fixSubRun.status === 'running';
}

function IterationStatusIcon({ iteration }: { iteration: CodeReviewIteration }) {
  if (iteration.status === 'running' || isFixRunning(iteration)) {
    return (
      <svg className="w-4 h-4 text-status-warning flex-shrink-0 animate-spin" viewBox="0 0 16 16" fill="none">
        <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" strokeDasharray="12 32" />
      </svg>
    );
  }
  if (iteration.status === 'error') {
    return (
      <svg className="w-4 h-4 text-status-error flex-shrink-0" viewBox="0 0 16 16" fill="none">
        <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" fill="currentColor" fillOpacity="0.15" />
        <path d="M6 6l4 4M10 6l-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </svg>
    );
  }
  if (iteration.issuesFound != null && iteration.issuesFound > 0) {
    return (
      <svg className="w-4 h-4 text-amber-400 flex-shrink-0" viewBox="0 0 16 16" fill="none">
        <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" fill="currentColor" fillOpacity="0.15" />
        <path d="M8 5v3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        <circle cx="8" cy="11" r="0.75" fill="currentColor" />
      </svg>
    );
  }
  if (iteration.issuesFound === 0) {
    return (
      <svg className="w-4 h-4 text-status-success flex-shrink-0" viewBox="0 0 16 16" fill="none">
        <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" fill="currentColor" fillOpacity="0.15" />
        <path d="M5 8l2 2 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      </svg>
    );
  }
  if (iteration.status === 'finished' && iteration.issuesFound == null) {
    return (
      <svg className="w-4 h-4 text-board-text-muted flex-shrink-0" viewBox="0 0 16 16" fill="none">
        <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" fill="currentColor" fillOpacity="0.1" />
        <path d="M8 5v3.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
        <circle cx="8" cy="11.5" r="0.75" fill="currentColor" />
      </svg>
    );
  }
  return (
    <svg className="w-4 h-4 text-board-text-muted flex-shrink-0" viewBox="0 0 16 16" fill="none">
      <circle cx="8" cy="8" r="7" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

function getIssuesBadge(iteration: CodeReviewIteration): string {
  if (iteration.issuesFound === 0) return 'clean';
  if (isFixRunning(iteration)) return 'fixing...';
  if (iteration.issuesFound != null && iteration.issuesFound > 0) {
    const label = `${iteration.issuesFound} issue${iteration.issuesFound === 1 ? '' : 's'}`;
    if (iteration.fixSubRun?.status === 'finished') return `${label} fixed`;
    return label;
  }
  if (iteration.status === 'running') return 'reviewing...';
  if (iteration.status === 'finished') return 'reviewed';
  return 'pending';
}

function SeverityBadge({ severity }: { severity: string }) {
  const s = severity.toLowerCase();
  return (
    <span className={cn(
      'text-[10px] font-medium px-1.5 py-0.5 rounded-full leading-none',
      s === 'high' ? 'bg-status-error/15 text-status-error' :
      s === 'medium' ? 'bg-amber-400/15 text-amber-400' :
      s === 'low' ? 'bg-sky-400/15 text-sky-400' :
      'bg-board-surface text-board-text-muted',
    )}>
      {severity}
    </span>
  );
}

function IssuesList({ issues }: { issues: CodeReviewIssue[] }) {
  return (
    <div className="space-y-1">
      {issues.map((issue, i) => (
        <div key={i} className="bg-board-surface rounded px-2.5 py-2 space-y-1">
          <div className="flex items-start gap-1.5">
            <span className="text-xs font-medium text-board-text flex-1">{issue.title}</span>
            {issue.severity && <SeverityBadge severity={issue.severity} />}
          </div>
          {issue.file && (
            <div className="flex items-center gap-1.5 text-[11px] text-board-text-muted">
              <code className="bg-board-bg/50 px-1 py-0.5 rounded font-mono">{issue.file}</code>
              {issue.lines && <span className="text-board-text-muted/60">L{issue.lines}</span>}
            </div>
          )}
          {issue.description && (
            <p className="text-[11px] text-board-text-muted leading-relaxed">{issue.description}</p>
          )}
        </div>
      ))}
    </div>
  );
}

export function CodeReviewChecklist({ iterations }: CodeReviewChecklistProps) {
  const [expandedIndex, setExpandedIndex] = useState<number | null>(null);

  const sortedIterations = useMemo(
    () => [...iterations].sort((a, b) => a.iteration - b.iteration),
    [iterations],
  );

  if (sortedIterations.length === 0) return null;

  return (
    <div className="space-y-0.5">
      {sortedIterations.map((iter) => {
        const idx = iter.iteration - 1;
        const iterRuns = [iter.reviewSubRun, iter.fixSubRun].filter(
          (r): r is AgentRun => r != null,
        );
        const combinedCost = aggregateRunCosts(iterRuns);

        return (
          <div key={iter.iteration}>
            <button
              onClick={() => setExpandedIndex(expandedIndex === idx ? null : idx)}
              className={cn(
                'w-full flex items-center gap-2 py-1.5 px-2 rounded text-left transition-colors',
                'hover:bg-board-card-hover',
                expandedIndex === idx && 'bg-board-card-hover',
              )}
            >
              <IterationStatusIcon iteration={iter} />
              <span className={cn(
                'text-xs flex-1',
                iter.issuesFound === 0 ? 'text-board-text-muted' : 'text-board-text-secondary',
                iter.status === 'running' && 'font-medium text-board-text-primary',
              )}>
                Iteration {iter.iteration}
              </span>
              <span className={cn(
                'text-xs px-1.5 py-0.5 rounded-full',
                iter.issuesFound === 0
                  ? 'bg-status-success/15 text-status-success'
                  : isFixRunning(iter)
                    ? 'bg-sky-400/15 text-sky-400'
                    : iter.issuesFound != null && iter.issuesFound > 0
                      ? 'bg-amber-400/15 text-amber-400'
                      : iter.status === 'running'
                        ? 'bg-status-warning/15 text-status-warning'
                        : 'bg-board-surface text-board-text-muted',
              )}>
                {getIssuesBadge(iter)}
              </span>
              {combinedCost && <CostBadge cost={combinedCost} />}
              <svg
                className={cn(
                  'w-3 h-3 text-board-text-muted transition-transform flex-shrink-0',
                  expandedIndex === idx && 'rotate-90',
                )}
                viewBox="0 0 12 12"
                fill="none"
              >
                <path d="M4 2l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>
            {expandedIndex === idx && (
              <div className="ml-6 mr-2 mb-1 max-h-72 overflow-y-auto">
                {iter.issues.length > 0 ? (
                  <IssuesList issues={iter.issues} />
                ) : iter.issuesSection ? (
                  <div className="px-2 py-1.5 text-xs text-board-text-muted whitespace-pre-wrap bg-board-surface rounded">
                    {iter.issuesSection}
                  </div>
                ) : iter.issuesFound === 0 ? (
                  <div className="px-2 py-1.5 text-xs text-status-success/70 bg-board-surface rounded">
                    No issues found — code is clean.
                  </div>
                ) : null}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
