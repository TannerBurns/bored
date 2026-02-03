import { useState } from 'react';
import { WorkerPanel } from '../workers';
import { getTimeAgo, formatDuration } from '../../lib/utils';
import type { Ticket, AgentRunWithContext, RunStatus } from '../../types';

interface AgentsViewProps {
  tickets: Ticket[];
  recentRuns: AgentRunWithContext[];
}

type AgentsTab = 'workers' | 'runs';

const AGENTS_TABS = [
  { 
    id: 'workers' as const, 
    label: 'Workers',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
        <circle cx="9" cy="7" r="4" />
        <path d="M22 21v-2a4 4 0 0 0-3-3.87" />
        <path d="M16 3.13a4 4 0 0 1 0 7.75" />
      </svg>
    ),
  },
  { 
    id: 'runs' as const, 
    label: 'Runs',
    icon: (
      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
        <polygon points="5 3 19 12 5 21 5 3" />
      </svg>
    ),
  },
];

export function AgentsView({ tickets, recentRuns }: AgentsViewProps) {
  const [agentsTab, setAgentsTab] = useState<AgentsTab>('workers');
  
  const activeTicketCount = tickets.filter((t) => t.lockedByRunId).length;

  return (
    <div className="flex-1 overflow-hidden flex flex-col">
      <div className="flex gap-1 mb-3">
        {AGENTS_TABS.map((tab) => {
          const badge = tab.id === 'runs' && activeTicketCount > 0 ? activeTicketCount : undefined;
          return (
            <button
              key={tab.id}
              onClick={() => setAgentsTab(tab.id)}
              className={`px-3 py-1.5 text-sm font-medium rounded-lg transition-all duration-200 flex items-center gap-1.5 ${
                agentsTab === tab.id
                  ? 'bg-board-accent text-white shadow-sm'
                  : 'glass text-board-text-muted hover:text-board-text hover:bg-board-card-hover'
              }`}
            >
              {tab.icon}
              {tab.label}
              {badge && (
                <span className={`text-xs px-1.5 py-0.5 rounded-full ${
                  agentsTab === tab.id 
                    ? 'bg-white/20' 
                    : 'bg-status-warning/20 text-status-warning'
                }`}>
                  {badge}
                </span>
              )}
            </button>
          );
        })}
      </div>

      {agentsTab === 'workers' && (
        <div className="flex-1 overflow-auto glass rounded-lg">
          <WorkerPanel />
        </div>
      )}

      {agentsTab === 'runs' && (
        <RunsContent tickets={tickets} recentRuns={recentRuns} />
      )}
    </div>
  );
}

function RunsContent({ tickets, recentRuns }: { tickets: Ticket[]; recentRuns: AgentRunWithContext[] }) {
  const activeTickets = tickets.filter((t) => t.lockedByRunId);

  return (
    <div className="flex-1 overflow-auto glass rounded-lg p-4">
      {activeTickets.length > 0 && (
        <div className="mb-4">
          <h4 className="text-xs font-medium text-board-text-secondary uppercase tracking-wide mb-2 flex items-center gap-1.5">
            <span className="inline-block w-1.5 h-1.5 bg-status-warning rounded-full animate-pulse" />
            Active Runs
          </h4>
          <div className="space-y-1.5">
            {activeTickets.map((ticket) => (
              <div
                key={ticket.id}
                className="px-3 py-2 glass-intense rounded-lg flex items-center justify-between"
              >
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-sm text-board-text truncate">{ticket.title}</span>
                    <span className="text-xs text-board-text-muted font-mono shrink-0">
                      #{ticket.id.slice(0, 8)}
                    </span>
                  </div>
                  <span className="text-xs text-board-text-muted">
                    Running with {ticket.agentPref || 'agent'}
                  </span>
                </div>
                <span className="text-status-warning text-xs flex items-center gap-1">
                  <span className="inline-block w-1.5 h-1.5 bg-status-warning rounded-full animate-pulse" />
                  In Progress
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
      
      <div>
        <h4 className="text-xs font-medium text-board-text-secondary uppercase tracking-wide mb-2">
          Recent Runs
        </h4>
        <div className="space-y-1.5">
          {recentRuns.length === 0 ? (
            <div className="glass-subtle rounded-lg p-6 text-center">
              <svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className="mx-auto text-board-text-muted mb-2">
                <polygon points="5 3 19 12 5 21 5 3" />
              </svg>
              <p className="text-board-text-muted text-sm">No runs yet</p>
              <p className="text-board-text-muted text-xs mt-0.5">Start a run from a ticket to see activity</p>
            </div>
          ) : (
            recentRuns.map((run) => <RunItem key={run.id} run={run} />)
          )}
        </div>
      </div>
    </div>
  );
}

const STATUS_CONFIG: Record<RunStatus, { color: string; bg: string; label: string; pulse: boolean }> = {
  running: { color: 'text-status-warning', bg: 'bg-status-warning', label: 'Running', pulse: true },
  queued: { color: 'text-board-text-muted', bg: 'bg-board-text-muted', label: 'Queued', pulse: false },
  finished: { color: 'text-status-success', bg: 'bg-status-success', label: 'Completed', pulse: false },
  error: { color: 'text-status-error', bg: 'bg-status-error', label: 'Error', pulse: false },
  aborted: { color: 'text-board-text-muted', bg: 'bg-board-text-muted', label: 'Aborted', pulse: false },
  paused: { color: 'text-blue-400', bg: 'bg-blue-400', label: 'Paused', pulse: false },
};

function RunItem({ run }: { run: AgentRunWithContext }) {
  const status = STATUS_CONFIG[run.status] || STATUS_CONFIG.error;
  const startedAt = new Date(run.startedAt);
  const endedAt = run.endedAt ? new Date(run.endedAt) : null;
  const timeAgo = getTimeAgo(startedAt);
  const duration = endedAt ? formatDuration(startedAt, endedAt) : null;
  const stageInfo = run.totalStages > 0 
    ? `${run.currentStage || 'stage'} (${run.completedStages}/${run.totalStages})`
    : null;
  
  return (
    <div className="px-3 py-2 glass-intense rounded-lg flex items-center justify-between hover:bg-board-card-hover transition-colors">
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-xs text-board-text-muted bg-board-bg/50 px-1.5 py-0.5 rounded shrink-0">
            {run.boardName}
          </span>
          <span className="font-medium text-sm text-board-text truncate">
            {run.ticketTitle}
          </span>
          <span className="text-xs text-board-text-muted font-mono shrink-0">
            #{run.ticketId.slice(0, 8)}
          </span>
        </div>
        <div className="flex items-center gap-1 text-xs text-board-text-muted">
          {run.projectName && (
            <>
              <span className="text-board-accent">{run.projectName}</span>
              <span>·</span>
            </>
          )}
          <span>{run.agentType === 'cursor' ? 'Cursor' : 'Claude'}</span>
          <span>·</span>
          <span>{timeAgo}</span>
          {duration && (
            <>
              <span>·</span>
              <span>{duration}</span>
            </>
          )}
          {stageInfo && (
            <>
              <span>·</span>
              <span className="text-board-text-secondary">{stageInfo}</span>
            </>
          )}
        </div>
      </div>
      <span className={`${status.color} text-xs flex items-center gap-1 shrink-0`}>
        <span className={`inline-block w-1.5 h-1.5 ${status.bg} rounded-full ${status.pulse ? 'animate-pulse' : ''}`} />
        {status.label}
      </span>
    </div>
  );
}
