import { cn } from '../../../lib/utils';
import type { AgentRun, RunCostData } from '../../../types';
import type { RunEvent } from './types';
import { ClaudeIcon, CursorIcon } from '../../common/AgentIcons';
import { CostBadge } from '../../common/CostBadge';

/** Extract cost data from a run's metadata field */
function getRunCost(run: AgentRun): RunCostData | null {
  if (!run.metadata) return null;
  const cost = (run.metadata as Record<string, unknown>).cost;
  if (!cost || typeof cost !== 'object') return null;
  return cost as RunCostData;
}

/** Normalize eventType which can be string or {custom: "value"} */
function getEventTypeString(eventType: unknown): string {
  if (typeof eventType === 'string') return eventType;
  if (typeof eventType === 'object' && eventType !== null) {
    const obj = eventType as Record<string, unknown>;
    if ('custom' in obj) return String(obj.custom);
    const keys = Object.keys(obj);
    if (keys.length === 1) return String(obj[keys[0]]);
    return JSON.stringify(eventType);
  }
  return String(eventType);
}

export interface RunsHistoryProps {
  agentRuns: AgentRun[];
  lockedByRunId?: string;
  expandedRunId: string | null;
  runEvents: RunEvent[];
  loadingEvents: boolean;
  handleRunClick: (runId: string) => Promise<void>;
}

export function RunsHistory({
  agentRuns,
  lockedByRunId,
  expandedRunId,
  runEvents,
  loadingEvents,
  handleRunClick,
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
  handleRunClick: (runId: string) => Promise<void>;
}

function CurrentRunSection({
  agentRuns,
  lockedByRunId,
  expandedRunId,
  runEvents,
  loadingEvents,
  handleRunClick,
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
              {currentRun.agentType === 'cursor' ? (
                <CursorIcon size={14} className="text-board-text-secondary" />
              ) : (
                <ClaudeIcon size={14} className="text-[#da7756]" />
              )}
              {currentRun.agentType === 'cursor' ? 'Cursor' : 'Claude'}
              {isMultiStage && <span className="text-board-accent ml-1">(Multi-Stage)</span>}
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
            
            {/* Sub-runs for multi-stage workflows */}
            {isMultiStage && subRuns.length > 0 && (
              <SubRunsList subRuns={subRuns} />
            )}
            
            {/* Events and Raw Logs */}
            <RunEventsDisplay runEvents={runEvents} loadingEvents={loadingEvents} />
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
  handleRunClick: (runId: string) => Promise<void>;
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
        {parentRuns.slice(0, 5).map((run) => {
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
                    {run.agentType === 'cursor' ? (
                      <CursorIcon size={14} className="text-board-text-secondary" />
                    ) : (
                      <ClaudeIcon size={14} className="text-[#da7756]" />
                    )}
                    {run.agentType === 'cursor' ? 'Cursor' : 'Claude'}
                    {isMultiStage && <span className="text-board-accent ml-1">(Multi-Stage)</span>}
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
                  <CostBadge cost={getRunCost(run)} />
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
}

function SubRunsList({ subRuns }: SubRunsListProps) {
  return (
    <div className="mt-3">
      <p className="text-xs font-medium text-board-text-muted mb-2">Stages ({subRuns.length}):</p>
      <div className="space-y-1 text-xs">
        {subRuns.sort((a, b) => new Date(a.startedAt).getTime() - new Date(b.startedAt).getTime()).map((subRun, idx) => (
          <div 
            key={subRun.id} 
            className="flex items-center gap-2 py-1 px-2 bg-board-surface-raised rounded"
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
            <span className="text-board-text-secondary font-medium w-24">
              {subRun.stage || `Stage ${idx + 1}`}
            </span>
            <span className={cn(
              'text-xs',
              subRun.status === 'finished' ? 'text-status-success' :
              subRun.status === 'running' ? 'text-status-warning' :
              subRun.status === 'error' ? 'text-status-error' :
              subRun.status === 'paused' ? 'text-blue-400' :
              'text-board-text-muted'
            )}>
              {subRun.status}
            </span>
            <span className="ml-auto flex items-center gap-1.5">
              <CostBadge cost={getRunCost(subRun)} />
              {subRun.endedAt && (
                <span className="text-board-text-muted">
                  {Math.round((new Date(subRun.endedAt).getTime() - new Date(subRun.startedAt).getTime()) / 1000)}s
                </span>
              )}
            </span>
          </div>
        ))}
      </div>
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
  // Aggregate cost from sub-runs if multi-stage, otherwise use run cost
  const totalCost = isMultiStage
    ? subRuns.reduce((sum, sr) => {
        const c = getRunCost(sr);
        return c ? sum + c.totalCostUsd : sum;
      }, 0)
    : getRunCost(run)?.totalCostUsd ?? 0;
  const hasEstimated = isMultiStage
    ? subRuns.some(sr => getRunCost(sr)?.isEstimated)
    : getRunCost(run)?.isEstimated ?? false;

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
            <span className={hasEstimated ? 'text-amber-400' : 'text-emerald-400'}>
              {hasEstimated ? '~' : ''}${totalCost < 0.01 ? totalCost.toFixed(4) : totalCost < 1 ? totalCost.toFixed(3) : totalCost.toFixed(2)}
            </span>
          </p>
        )}
      </div>

      {/* Sub-runs for multi-stage workflows */}
      {isMultiStage && subRuns.length > 0 && (
        <SubRunsList subRuns={subRuns} />
      )}
      
      {/* Summary */}
      {run.summaryMd && (
        <div className="mt-2">
          <p className="text-xs font-medium text-board-text-muted mb-1">Summary:</p>
          <p className="text-xs text-board-text-secondary whitespace-pre-wrap bg-board-surface-raised p-2 rounded">
            {run.summaryMd}
          </p>
        </div>
      )}

      {/* Events and Raw Logs */}
      <RunEventsDisplay runEvents={runEvents} loadingEvents={loadingEvents} />
    </div>
  );
}

interface RunEventsDisplayProps {
  runEvents: RunEvent[];
  loadingEvents: boolean;
}

function RunEventsDisplay({ runEvents, loadingEvents }: RunEventsDisplayProps) {
  const logEvents = runEvents.filter(e => {
    const type = getEventTypeString(e.eventType);
    return type === 'log_stdout' || type === 'log_stderr';
  });
  const structuredEvents = runEvents.filter(e => {
    const type = getEventTypeString(e.eventType);
    return type !== 'log_stdout' && type !== 'log_stderr';
  });

  return (
    <>
      {/* Events (from hooks - file edits, commands, etc.) */}
      <div className="mt-2">
        <p className="text-xs font-medium text-board-text-muted mb-1">
          Events ({loadingEvents ? '...' : structuredEvents.length}):
        </p>
        {loadingEvents ? (
          <p className="text-xs text-board-text-muted">Loading...</p>
        ) : structuredEvents.length === 0 ? (
          <p className="text-xs text-board-text-muted italic">No hook events recorded (hooks may not be installed)</p>
        ) : (
          <div className="bg-board-surface-raised rounded p-2 max-h-40 overflow-y-auto font-mono text-xs">
            {structuredEvents.map((event) => (
              <div key={event.id} className="text-board-text-secondary py-0.5 border-b border-board-border last:border-0">
                <span className="text-board-text-muted">{new Date(event.createdAt).toLocaleTimeString()}</span>
                {' '}
                <span className="text-board-accent">[{getEventTypeString(event.eventType)}]</span>
                {' '}
                {typeof event.payload === 'object' 
                  ? JSON.stringify(event.payload).substring(0, 100) + (JSON.stringify(event.payload).length > 100 ? '...' : '')
                  : String(event.payload)}
              </div>
            ))}
          </div>
        )}
      </div>
      
      {/* Raw Logs (stdout/stderr from agent process) */}
      <div className="mt-2">
        <p className="text-xs font-medium text-board-text-muted mb-1">
          Raw Logs ({loadingEvents ? '...' : logEvents.length} lines):
        </p>
        {loadingEvents ? (
          <p className="text-xs text-board-text-muted">Loading...</p>
        ) : logEvents.length === 0 ? (
          <p className="text-xs text-board-text-muted italic">No output logs recorded</p>
        ) : (
          <div className="bg-black/80 rounded p-2 max-h-60 overflow-y-auto font-mono text-xs">
            {logEvents.map((event) => {
              const payload = event.payload as { raw?: string } | null;
              const content = payload?.raw || '';
              const eventTypeStr = getEventTypeString(event.eventType);
              const isStderr = eventTypeStr === 'log_stderr';
              return (
                <div 
                  key={event.id} 
                  className={cn(
                    'whitespace-pre-wrap break-all',
                    isStderr ? 'text-red-400' : 'text-green-400'
                  )}
                >
                  {content}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </>
  );
}
