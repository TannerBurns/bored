import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { cn } from '../../../lib/utils';
import type { AgentRun, RunCostData, ModelCostData } from '../../../types';
import type { RunEvent, ImplementationTodoStatus } from './types';
import { getAgentIcon, getAgentDisplayName, getAgentBrandColor } from '../../common/AgentIcons';
import { CostBadge, getRunCost, getTotalCost } from '../../common/CostBadge';
import { SafetyCommitNotice } from '../../common/SafetyCommitNotice';
import { LogTimelineView } from './LogTimeline/LogTimelineView';
import { ImplementationChecklist } from './ImplementationChecklist';

function getWorkflowLabel(run: AgentRun): string {
  const mode = (run.metadata as Record<string, unknown> | undefined)?.workflow_mode;
  return mode === 'auto_pilot' ? 'Auto-Pilot' : 'Multi-Stage';
}

/** Aggregate cost data from multiple runs, deriving totalCostUsd from
 *  the per-model sums so the two can never diverge. */
function aggregateRunCosts(runs: AgentRun[]): RunCostData | null {
  let total = 0;
  let inputTokens = 0;
  let outputTokens = 0;
  let cacheRead = 0;
  let cacheWrite = 0;
  let anyEstimated = false;
  let found = false;
  const mergedModels: Record<string, ModelCostData> = {};

  for (const sr of runs) {
    const c = getRunCost(sr);
    if (!c) continue;
    found = true;
    total += c.totalCostUsd;
    inputTokens += c.inputTokens;
    outputTokens += c.outputTokens;
    cacheRead += c.cacheReadTokens;
    cacheWrite += c.cacheCreationTokens;
    if (c.isEstimated) anyEstimated = true;

    const models = c.modelUsage ?? {};
    if (Object.keys(models).length === 0) {
      if (c.totalCostUsd > 0 || c.inputTokens > 0 || c.outputTokens > 0
          || c.cacheReadTokens > 0 || c.cacheCreationTokens > 0) {
        const entry = mergedModels['other'] ??= { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd: 0 };
        entry.inputTokens += c.inputTokens;
        entry.outputTokens += c.outputTokens;
        entry.cacheReadTokens += c.cacheReadTokens;
        entry.cacheCreationTokens += c.cacheCreationTokens;
        entry.costUsd += c.totalCostUsd;
      }
    } else {
      for (const [model, data] of Object.entries(models)) {
        const entry = mergedModels[model] ??= { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd: 0 };
        entry.inputTokens += data.inputTokens;
        entry.outputTokens += data.outputTokens;
        entry.cacheReadTokens += data.cacheReadTokens;
        entry.cacheCreationTokens += data.cacheCreationTokens;
        entry.costUsd += data.costUsd;
      }
    }
  }

  if (!found) return null;

  const modelSum = Object.values(mergedModels).reduce((s, m) => s + m.costUsd, 0);

  return {
    totalCostUsd: modelSum > 0 ? modelSum : total,
    inputTokens,
    outputTokens,
    cacheReadTokens: cacheRead,
    cacheCreationTokens: cacheWrite,
    modelUsage: mergedModels,
    isEstimated: anyEstimated,
  };
}

/** For multi-stage parent runs, sum sub-run costs so the badge matches
 *  the backend aggregate (which excludes the parent). */
function getParentRunDisplayCost(run: AgentRun, subRuns: AgentRun[]): RunCostData | null {
  if (subRuns.length === 0) return getRunCost(run);
  return aggregateRunCosts(subRuns);
}

export interface RunsHistoryProps {
  agentRuns: AgentRun[];
  lockedByRunId?: string;
  expandedRunId: string | null;
  runEvents: RunEvent[];
  loadingEvents: boolean;
  handleRunClick: (runId: string) => void;
  implementationTodos?: ImplementationTodoStatus[];
}

export function RunsHistory({
  agentRuns,
  lockedByRunId,
  expandedRunId,
  runEvents,
  loadingEvents,
  handleRunClick,
  implementationTodos,
}: RunsHistoryProps) {
  if (agentRuns.length === 0) {
    return null;
  }

  return (
    <>
      {/* Current Run */}
      {lockedByRunId && (
        <CurrentRunSection
          agentRuns={agentRuns}
          lockedByRunId={lockedByRunId}
          expandedRunId={expandedRunId}
          runEvents={runEvents}
          loadingEvents={loadingEvents}
          handleRunClick={handleRunClick}
          implementationTodos={implementationTodos}
        />
      )}

      {/* Previous Runs */}
      <PreviousRunsSection
        agentRuns={agentRuns}
        lockedByRunId={lockedByRunId}
        expandedRunId={expandedRunId}
        runEvents={runEvents}
        loadingEvents={loadingEvents}
        handleRunClick={handleRunClick}
      />
    </>
  );
}

interface CurrentRunSectionProps {
  agentRuns: AgentRun[];
  lockedByRunId: string;
  expandedRunId: string | null;
  runEvents: RunEvent[];
  loadingEvents: boolean;
  handleRunClick: (runId: string) => void;
  implementationTodos?: ImplementationTodoStatus[];
}

function CurrentRunSection({
  agentRuns,
  lockedByRunId,
  expandedRunId,
  runEvents,
  loadingEvents,
  handleRunClick,
  implementationTodos,
}: CurrentRunSectionProps) {
  const currentRun = agentRuns.find(r => r.id === lockedByRunId);
  if (!currentRun) return null;
  
  const subRuns = agentRuns.filter(r => r.parentRunId === currentRun.id);
  const isMultiStage = subRuns.length > 0;

  return (
    <div>
      <h3 className="text-sm font-medium text-board-text-muted mb-2">
        Current Run
      </h3>
      <div className="bg-board-surface rounded-lg overflow-hidden border border-status-warning/30">
        <button
          onClick={() => handleRunClick(currentRun.id)}
          className="w-full flex items-center justify-between p-2 text-sm hover:bg-board-card-hover transition-colors"
        >
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full flex-shrink-0 bg-status-warning animate-pulse" />
            <span className="text-board-text-secondary flex items-center gap-1">
              {(() => {
                const Icon = getAgentIcon(currentRun.agentType);
                const brandColor = getAgentBrandColor(currentRun.agentType);
                return brandColor
                  ? <Icon size={14} style={{ color: brandColor }} />
                  : <Icon size={14} className="text-board-text-secondary" />;
              })()}
              {getAgentDisplayName(currentRun.agentType)}
              {isMultiStage && <span className="text-board-accent ml-1">({getWorkflowLabel(currentRun)})</span>}
            </span>
            <span className="text-board-text-muted text-xs">
              {new Date(currentRun.startedAt).toLocaleString()}
            </span>
            <span className="text-board-text-muted text-xs">
              {expandedRunId === currentRun.id ? '▼' : '▶'}
            </span>
          </div>
          <span className="text-xs px-2 py-0.5 rounded bg-status-warning/20 text-status-warning">
            {currentRun.status}
          </span>
        </button>
        
        {/* Expanded current run details */}
        {expandedRunId === currentRun.id && (
          <div className="px-3 pb-3 border-t border-board-border">
            <div className="mt-2 space-y-1 text-xs text-board-text-muted">
              <p><span className="font-medium">Run ID:</span> {currentRun.id}</p>
              <p><span className="font-medium">Started:</span> {new Date(currentRun.startedAt).toLocaleString()}</p>
            </div>

            <SafetyCommitNotice run={currentRun} className="mt-3" />
            <AutoPilotSelections run={currentRun} />
            
            {/* Sub-runs for multi-stage workflows */}
            {isMultiStage && subRuns.length > 0 && (
              <SubRunsList subRuns={subRuns} implementationTodos={implementationTodos} />
            )}

            {/* Logs */}
            <LogTimelineView events={runEvents} agentType={currentRun.agentType} loadingEvents={loadingEvents} />
          </div>
        )}
      </div>
    </div>
  );
}

interface PreviousRunsSectionProps {
  agentRuns: AgentRun[];
  lockedByRunId?: string;
  expandedRunId: string | null;
  runEvents: RunEvent[];
  loadingEvents: boolean;
  handleRunClick: (runId: string) => void;
}

function PreviousRunsSection({
  agentRuns,
  lockedByRunId,
  expandedRunId,
  runEvents,
  loadingEvents,
  handleRunClick,
}: PreviousRunsSectionProps) {
  const parentRuns = agentRuns.filter(r => !r.parentRunId && r.id !== lockedByRunId);
  const subRunsByParent = agentRuns.reduce((acc, run) => {
    if (run.parentRunId) {
      if (!acc[run.parentRunId]) acc[run.parentRunId] = [];
      acc[run.parentRunId].push(run);
    }
    return acc;
  }, {} as Record<string, AgentRun[]>);
  
  if (parentRuns.length === 0) return null;

  return (
    <div>
      <h3 className="text-sm font-medium text-board-text-muted mb-2">
        Previous Runs ({parentRuns.length})
      </h3>
      <div className="space-y-2">
        {parentRuns.map((run) => {
          const subRuns = subRunsByParent[run.id] || [];
          const isMultiStage = subRuns.length > 0;
          
          return (
            <div key={run.id} className="bg-board-surface rounded-lg overflow-hidden">
              <button
                onClick={() => handleRunClick(run.id)}
                className="w-full flex items-center justify-between p-2 text-sm hover:bg-board-card-hover transition-colors"
              >
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      'w-2 h-2 rounded-full flex-shrink-0',
                      run.status === 'finished' ? 'bg-status-success' :
                      run.status === 'running' ? 'bg-status-warning animate-pulse' :
                      run.status === 'error' ? 'bg-status-error' :
                      run.status === 'paused' ? 'bg-blue-400' :
                      'bg-board-text-muted'
                    )}
                  />
                  <span className="text-board-text-secondary flex items-center gap-1">
                    {(() => {
                      const Icon = getAgentIcon(run.agentType);
                      const brandColor = getAgentBrandColor(run.agentType);
                      return brandColor
                        ? <Icon size={14} style={{ color: brandColor }} />
                        : <Icon size={14} className="text-board-text-secondary" />;
                    })()}
                    {getAgentDisplayName(run.agentType)}
                    {isMultiStage && <span className="text-board-accent ml-1">({getWorkflowLabel(run)})</span>}
                    {run.resumedFromRunId && <span className="text-blue-400 ml-1">(Resumed)</span>}
                  </span>
                  <span className="text-board-text-muted text-xs">
                    {new Date(run.startedAt).toLocaleString()}
                  </span>
                  <span className="text-board-text-muted text-xs">
                    {expandedRunId === run.id ? '▼' : '▶'}
                  </span>
                </div>
                <div className="flex items-center gap-1.5">
                  <CostBadge cost={getParentRunDisplayCost(run, subRuns)} />
                  <span
                    className={cn(
                      'text-xs px-2 py-0.5 rounded',
                      run.status === 'finished' ? 'bg-status-success/20 text-status-success' :
                      run.status === 'running' ? 'bg-status-warning/20 text-status-warning' :
                      run.status === 'error' ? 'bg-status-error/20 text-status-error' :
                      run.status === 'paused' ? 'bg-blue-400/20 text-blue-400' :
                      'bg-board-surface text-board-text-muted'
                    )}
                  >
                    {run.status}
                  </span>
                </div>
              </button>
      
              {/* Expanded run details */}
              {expandedRunId === run.id && (
                <ExpandedRunDetails
                  run={run}
                  subRuns={subRuns}
                  isMultiStage={isMultiStage}
                  runEvents={runEvents}
                  loadingEvents={loadingEvents}
                />
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

interface SubRunsListProps {
  subRuns: AgentRun[];
  implementationTodos?: ImplementationTodoStatus[];
}

function SubRunsList({ subRuns, implementationTodos }: SubRunsListProps) {
  const [implExpanded, setImplExpanded] = useState(false);
  const sorted = [...subRuns].sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime());

  const hasTodos = implementationTodos && implementationTodos.length > 0;
  const implementSubRuns = hasTodos ? sorted.filter(r => r.stage === 'implement') : [];
  const completedImpl = hasTodos ? implementationTodos.filter(t => t.status === 'completed').length : 0;
  const totalImpl = hasTodos ? implementationTodos.length : 0;

  type DisplayRow = { type: 'single'; run: AgentRun; idx: number } | { type: 'grouped'; runs: AgentRun[] };
  const rows: DisplayRow[] = [];
  let implGroupInserted = false;

  sorted.forEach((subRun, idx) => {
    if (hasTodos && subRun.stage === 'implement') {
      if (!implGroupInserted) {
        rows.push({ type: 'grouped', runs: implementSubRuns });
        implGroupInserted = true;
      }
    } else {
      rows.push({ type: 'single', run: subRun, idx });
    }
  });

  const displayCount = hasTodos
    ? sorted.filter(r => r.stage !== 'implement').length + (implementSubRuns.length > 0 ? 1 : 0)
    : sorted.length;

  return (
    <div className="mt-3">
      <p className="text-xs font-medium text-board-text-muted mb-2">Stages ({displayCount}):</p>
      <div className="space-y-1 text-xs">
        {rows.map((row) => {
          if (row.type === 'grouped') {
            const anyRunning = row.runs.some(r => r.status === 'running');
            const anyError = row.runs.some(r => r.status === 'error');
            const allFinished = row.runs.every(r => r.status === 'finished');
            const groupStatus = anyRunning ? 'running' : anyError ? 'error' : allFinished ? 'finished' : 'pending';

            const groupCost = aggregateRunCosts(row.runs);
            const totalDuration = row.runs.reduce((sum, r) => {
              if (r.endedAt) {
                return sum + (new Date(r.endedAt).getTime() - new Date(r.startedAt).getTime()) / 1000;
              }
              return sum;
            }, 0);
            const progressPct = totalImpl > 0 ? (completedImpl / totalImpl) * 100 : 0;

            return (
              <div key="implement-group" className="rounded overflow-hidden">
                <button
                  onClick={() => setImplExpanded(!implExpanded)}
                  className="w-full flex items-center gap-1.5 py-1 px-2 bg-board-surface-raised hover:bg-board-card-hover transition-colors text-left"
                >
                  <span
                    className={cn(
                      'w-1.5 h-1.5 rounded-full flex-shrink-0',
                      groupStatus === 'finished' ? 'bg-status-success' :
                      groupStatus === 'running' ? 'bg-status-warning animate-pulse' :
                      groupStatus === 'error' ? 'bg-status-error' :
                      'bg-board-text-muted'
                    )}
                  />
                  <span className="text-board-text-secondary font-medium w-28 shrink-0 truncate">
                    implement ({completedImpl}/{totalImpl})
                  </span>
                  <span className={cn(
                    'text-xs w-14 shrink-0',
                    groupStatus === 'finished' ? 'text-status-success' :
                    groupStatus === 'running' ? 'text-status-warning' :
                    groupStatus === 'error' ? 'text-status-error' :
                    'text-board-text-muted'
                  )}>
                    {groupStatus}
                  </span>
                  <div className="flex-1 h-1 bg-board-bg/50 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-status-success rounded-full transition-all duration-300"
                      style={{ width: `${progressPct}%` }}
                    />
                  </div>
                  <CostBadge cost={groupCost} />
                  {totalDuration > 0 && (
                    <span className="text-board-text-muted w-10 text-right shrink-0">
                      {Math.round(totalDuration)}s
                    </span>
                  )}
                  <svg
                    className={cn(
                      'w-3 h-3 text-board-text-muted transition-transform flex-shrink-0',
                      implExpanded && 'rotate-90',
                    )}
                    viewBox="0 0 12 12"
                    fill="none"
                  >
                    <path d="M4 2l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
                  </svg>
                </button>
                {implExpanded && (
                  <div className="px-2 pb-2 pt-1">
                    <ImplementationChecklist
                      todos={implementationTodos!}
                      implementSubRuns={implementSubRuns}
                    />
                  </div>
                )}
              </div>
            );
          }

          const subRun = row.run;
          return (
            <div
              key={subRun.id}
              className="flex items-center gap-1.5 py-1 px-2 bg-board-surface-raised rounded"
            >
              <span
                className={cn(
                  'w-1.5 h-1.5 rounded-full flex-shrink-0',
                  subRun.status === 'finished' ? 'bg-status-success' :
                  subRun.status === 'running' ? 'bg-status-warning animate-pulse' :
                  subRun.status === 'error' ? 'bg-status-error' :
                  subRun.status === 'paused' ? 'bg-blue-400' :
                  'bg-board-text-muted'
                )}
              />
              <span className="text-board-text-secondary font-medium w-28 shrink-0 truncate">
                {subRun.stage || `Stage ${row.idx + 1}`}
              </span>
              <span className={cn(
                'text-xs w-14 shrink-0',
                subRun.status === 'finished' ? 'text-status-success' :
                subRun.status === 'running' ? 'text-status-warning' :
                subRun.status === 'error' ? 'text-status-error' :
                subRun.status === 'paused' ? 'text-blue-400' :
                'text-board-text-muted'
              )}>
                {subRun.status}
              </span>
              <span className="flex-1" />
              <CostBadge cost={getRunCost(subRun)} />
              {subRun.endedAt && (
                <span className="text-board-text-muted w-10 text-right shrink-0">
                  {Math.round((new Date(subRun.endedAt).getTime() - new Date(subRun.startedAt).getTime()) / 1000)}s
                </span>
              )}
              <span className="w-3 shrink-0" />
            </div>
          );
        })}
      </div>
    </div>
  );
}

function AutoPilotSelections({ run }: { run: AgentRun }) {
  const meta = run.metadata as Record<string, unknown> | undefined;
  if (meta?.workflow_mode !== 'auto_pilot') return null;

  const raw = meta.auto_pilot_selections;
  if (!Array.isArray(raw)) return null;
  const selections = raw as { command: string; model: string }[];

  return (
    <div className="mt-3">
      <p className="text-xs font-medium text-board-text-muted mb-2">
        Auto-Pilot Selected Commands ({selections.length}):
      </p>
      {selections.length === 0 ? (
        <p className="text-xs text-board-text-muted italic px-2">No commands selected</p>
      ) : (
        <div className="flex flex-wrap gap-1.5">
          {selections.map((s, i) => (
            <span
              key={i}
              className="inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded-full bg-board-accent/15 text-board-accent border border-board-accent/25"
            >
              <span className="font-medium">{String(s.command ?? '')}</span>
              <span className="text-board-text-muted">{String(s.model ?? '')}</span>
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

interface ExpandedRunDetailsProps {
  run: AgentRun;
  subRuns: AgentRun[];
  isMultiStage: boolean;
  runEvents: RunEvent[];
  loadingEvents: boolean;
}

function ExpandedRunDetails({
  run,
  subRuns,
  isMultiStage,
  runEvents,
  loadingEvents,
}: ExpandedRunDetailsProps) {
  const [savedTodos, setSavedTodos] = useState<ImplementationTodoStatus[]>([]);

  useEffect(() => {
    invoke<ImplementationTodoStatus[]>('get_implementation_todos', { runId: run.id })
      .then((todos) => setSavedTodos(todos))
      .catch(() => setSavedTodos([]));
  }, [run.id]);

  // Use the same model-derived cost as the badge so they always match.
  const displayCost = getParentRunDisplayCost(run, subRuns);
  const totalCost = displayCost ? getTotalCost(displayCost) : 0;

  return (
    <div className="px-3 pb-3 border-t border-board-border">
      {/* Run metadata */}
      <div className="mt-2 space-y-1 text-xs text-board-text-muted">
        <p><span className="font-medium">Run ID:</span> {run.id}</p>
        {run.endedAt && (
          <p><span className="font-medium">Duration:</span> {Math.round((new Date(run.endedAt).getTime() - new Date(run.startedAt).getTime()) / 1000)}s</p>
        )}
        {run.exitCode !== undefined && (
          <p><span className="font-medium">Exit code:</span> {run.exitCode}</p>
        )}
        {totalCost > 0 && (
          <p>
            <span className="font-medium">Total Cost:</span>{' '}
            <span className="text-emerald-400">
              ${totalCost < 0.01 ? totalCost.toFixed(4) : totalCost < 1 ? totalCost.toFixed(3) : totalCost.toFixed(2)}
            </span>
          </p>
        )}
      </div>

      <SafetyCommitNotice run={run} className="mt-3" />
      <AutoPilotSelections run={run} />

      {/* Sub-runs for multi-stage workflows */}
      {isMultiStage && subRuns.length > 0 && (
        <SubRunsList subRuns={subRuns} implementationTodos={savedTodos} />
      )}

      {/* Summary */}
      {run.summaryMd && (
        <div className="mt-2">
          <p className="text-xs font-medium text-board-text-muted mb-1">Summary:</p>
          <p className="text-xs text-board-text-secondary whitespace-pre-wrap bg-board-surface-raised p-2 rounded">
            {typeof run.summaryMd === 'string' ? run.summaryMd : String(run.summaryMd)}
          </p>
        </div>
      )}

      {/* Logs */}
      <LogTimelineView events={runEvents} agentType={run.agentType} loadingEvents={loadingEvents} />
    </div>
  );
}
