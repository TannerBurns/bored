import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { cn } from '../../lib/utils';
import type { SpecProgress, SpecEta } from '../../types';

function formatDuration(seconds: number): string {
  if (seconds < 60) {
    return `${Math.round(seconds)}s`;
  } else if (seconds < 3600) {
    const mins = Math.floor(seconds / 60);
    const secs = Math.round(seconds % 60);
    return secs > 0 ? `${mins}m ${secs}s` : `${mins}m`;
  } else {
    const hours = Math.floor(seconds / 3600);
    const mins = Math.floor((seconds % 3600) / 60);
    return mins > 0 ? `${hours}h ${mins}m` : `${hours}h`;
  }
}

interface EpicProgressPanelProps {
  progress: SpecProgress;
  specId?: string;
  isWorking: boolean;
  isPaused?: boolean;
  isCompleted: boolean;
}

export function EpicProgressPanel({ progress, specId, isWorking, isPaused = false, isCompleted }: EpicProgressPanelProps) {
  const [expandedEpics, setExpandedEpics] = useState<Set<string>>(new Set());
  const [eta, setEta] = useState<SpecEta | null>(null);
  
  // Load ETA when working
  useEffect(() => {
    if (!specId || (!isWorking && !isPaused)) {
      setEta(null);
      return;
    }
    
    const loadEta = async () => {
      try {
        const result = await invoke<SpecEta>('get_spec_eta', { specId });
        setEta(result);
      } catch {
        // ETA calculation can fail gracefully - it's informational
        setEta(null);
      }
    };
    
    loadEta();
    
    // Poll for ETA updates when working
    if (isWorking) {
      const interval = setInterval(loadEta, 30000); // Update every 30s
      return () => clearInterval(interval);
    }
  }, [specId, isWorking, isPaused]);
  
  const toggleEpic = (epicId: string) => {
    setExpandedEpics(prev => {
      const next = new Set(prev);
      if (next.has(epicId)) {
        next.delete(epicId);
      } else {
        next.add(epicId);
      }
      return next;
    });
  };
  
  const expandAll = () => {
    setExpandedEpics(new Set(progress.epics.map(e => e.id)));
  };
  
  const collapseAll = () => {
    setExpandedEpics(new Set());
  };
  
  // Calculate execution flow info
  const rootEpics = progress.epics.filter(e => e.dependsOnIds.length === 0);
  const dependentEpics = progress.epics.filter(e => e.dependsOnIds.length > 0);
  const waitingEpics = dependentEpics.filter(e => e.column === 'Backlog');
  
  const getColumnColor = (column: string) => {
    switch (column) {
      case 'Done': return 'bg-status-success';
      case 'Ready': return 'bg-status-info';
      case 'In Progress': return 'bg-status-warning';
      case 'Review': return 'bg-purple-500';
      case 'Blocked': return 'bg-status-error';
      case 'Backlog': return 'bg-board-text-muted';
      default: return 'bg-board-text-muted';
    }
  };
  
  const getColumnGlow = (column: string) => {
    switch (column) {
      case 'Done': return 'glow-success';
      case 'Ready': return '';
      case 'In Progress': return 'glow-warning';
      case 'Blocked': return 'glow-error';
      default: return '';
    }
  };
  
  const getColumnIcon = (column: string) => {
    switch (column) {
      case 'Done': return '✓';
      case 'Ready': return '▶';
      case 'In Progress': return '⚡';
      case 'Review': return '👁';
      case 'Blocked': return '⚠';
      case 'Backlog': return '📋';
      default: return '•';
    }
  };
  
  const percentComplete = progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0;
  
  return (
    <div className="space-y-6">
      {/* Summary */}
      <div className="glass rounded-xl p-5">
        <div className="flex items-center justify-between mb-4">
          <h3 className="font-semibold text-board-text">
            Epic Progress
          </h3>
          <div className="flex items-center gap-2">
            {isWorking && (
              <span className="flex items-center gap-1.5 text-sm text-status-success glass-subtle px-2 py-1 rounded-lg">
                <span className="w-2 h-2 bg-status-success rounded-full animate-pulse" />
                In Progress
              </span>
            )}
            {isPaused && (
              <span className="flex items-center gap-1.5 text-sm text-status-warning glass-subtle px-2 py-1 rounded-lg">
                <span className="w-2 h-2 bg-status-warning rounded-full" />
                Paused
              </span>
            )}
            {isCompleted && progress.done === progress.total && (
              <span className="flex items-center gap-1.5 text-sm text-status-success glass-subtle px-2 py-1 rounded-lg">
                <span className="text-lg">✓</span>
                All Complete
              </span>
            )}
            {isCompleted && progress.done < progress.total && (
              <span className="flex items-center gap-1.5 text-sm text-status-warning glass-subtle px-2 py-1 rounded-lg">
                <span className="text-lg">⚠</span>
                Work Pending
              </span>
            )}
          </div>
        </div>
        
        {/* Progress bar */}
        <div className="mb-4">
          <div className="flex justify-between text-sm text-board-text-secondary mb-2">
            <span>{progress.done} of {progress.total} epics complete</span>
            <span className="font-medium">{percentComplete}%</span>
          </div>
          <div className="h-3 glass-subtle rounded-full overflow-hidden">
            <div 
              className="h-full bg-board-accent transition-all duration-500"
              style={{ width: `${percentComplete}%` }}
            />
          </div>
        </div>
        
        {/* Stats */}
        <div className="flex flex-wrap gap-4 text-sm">
          <div className="flex items-center gap-2 glass-subtle px-3 py-1.5 rounded-lg">
            <span className="w-3 h-3 bg-status-success rounded-full" />
            <span className="text-board-text-secondary">Done: <span className="font-medium text-board-text">{progress.done}</span></span>
          </div>
          <div className="flex items-center gap-2 glass-subtle px-3 py-1.5 rounded-lg">
            <span className="w-3 h-3 bg-status-info rounded-full" />
            <span className="text-board-text-secondary">In Progress: <span className="font-medium text-board-text">{progress.inProgress}</span></span>
          </div>
          {progress.blocked > 0 && (
            <div className="flex items-center gap-2 glass-subtle px-3 py-1.5 rounded-lg">
              <span className="w-3 h-3 bg-status-error rounded-full" />
              <span className="text-board-text-secondary">Blocked: <span className="font-medium text-board-text">{progress.blocked}</span></span>
            </div>
          )}
        </div>
        
        {/* ETA Display */}
        {eta && (isWorking || isPaused) && (
          <div className="mt-4 pt-4 border-t border-board-border">
            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2">
                <span className="text-board-text-muted">Estimated completion:</span>
                <span className="font-medium text-board-text">
                  {eta.estimatedSecondsRemaining != null ? formatDuration(eta.estimatedSecondsRemaining) : 'Calculating...'}
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className={cn(
                  'w-2 h-2 rounded-full',
                  eta.confidence === 'high' ? 'bg-status-success' :
                  eta.confidence === 'medium' ? 'bg-status-warning' : 'bg-board-text-muted'
                )} />
                <span className="text-xs text-board-text-muted capitalize">
                  {eta.confidence} confidence
                </span>
              </div>
            </div>
            {eta.avgSecondsPerTicket != null && (
              <div className="mt-1 text-xs text-board-text-muted">
                Based on avg. {formatDuration(eta.avgSecondsPerTicket)} per ticket 
                ({eta.completedTickets}/{eta.totalTickets} completed)
              </div>
            )}
          </div>
        )}
        
        {/* Execution Flow Info */}
        {isWorking && (
          <div className="mt-4 pt-4 border-t border-board-border">
            <div className="text-xs space-y-1.5">
              <div className="font-medium text-board-text mb-2">Execution Flow</div>
              <div className="flex items-center gap-2 text-board-text-secondary">
                <span className="text-status-success">▶</span>
                <span>{rootEpics.length} root epic{rootEpics.length !== 1 ? 's' : ''} (can start immediately{rootEpics.length > 1 ? ', in parallel' : ''})</span>
              </div>
              {waitingEpics.length > 0 && (
                <div className="flex items-center gap-2 text-board-text-secondary">
                  <span className="text-status-warning">⏳</span>
                  <span>{waitingEpics.length} epic{waitingEpics.length !== 1 ? 's' : ''} waiting on dependencies</span>
                </div>
              )}
              {dependentEpics.length > 0 && dependentEpics.length !== waitingEpics.length && (
                <div className="flex items-center gap-2 text-board-text-secondary">
                  <span className="text-status-info">→</span>
                  <span>{dependentEpics.length - waitingEpics.length} dependent epic{(dependentEpics.length - waitingEpics.length) !== 1 ? 's' : ''} already started/done</span>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
      
      {/* Epic list */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <h4 className="font-medium text-board-text">Epics</h4>
          <div className="flex gap-2 text-xs">
            <button 
              onClick={expandAll}
              className="text-board-accent hover:text-board-accent-hover transition-colors"
            >
              Expand all
            </button>
            <span className="text-board-text-muted">|</span>
            <button 
              onClick={collapseAll}
              className="text-board-accent hover:text-board-accent-hover transition-colors"
            >
              Collapse all
            </button>
          </div>
        </div>
        <div className="space-y-2">
          {progress.epics.map((epic) => {
            const isExpanded = expandedEpics.has(epic.id);
            const ticketsDone = epic.tickets.filter(t => t.column === 'Done').length;
            
            return (
              <div 
                key={epic.id}
                className="glass rounded-xl overflow-hidden"
              >
                {/* Epic header */}
                <button
                  onClick={() => toggleEpic(epic.id)}
                  className="w-full flex items-center justify-between p-3 hover:bg-board-card-hover transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <span className={cn(
                      'w-8 h-8 flex items-center justify-center rounded-full text-white text-sm shadow-sm',
                      getColumnColor(epic.column),
                      getColumnGlow(epic.column)
                    )}>
                      {getColumnIcon(epic.column)}
                    </span>
                    <div className="text-left">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-board-text">
                          {epic.title}
                        </span>
                        {epic.tickets.length > 0 && (
                          <span className="text-xs text-board-text-muted glass-subtle px-1.5 py-0.5 rounded">
                            {ticketsDone}/{epic.tickets.length}
                          </span>
                        )}
                        {epic.dependsOnIds.length === 0 && (
                          <span className="px-1.5 py-0.5 text-xs bg-status-success/20 text-status-success rounded-full">
                            root
                          </span>
                        )}
                      </div>
                      {epic.dependsOnTitles.length > 0 && (
                        <div className="flex items-center gap-1 text-xs text-status-warning mt-0.5">
                          <span>↳</span>
                          <span>waits for: {epic.dependsOnTitles.join(', ')}</span>
                          {epic.column === 'Backlog' && (
                            <span className="text-board-text-muted">(blocked)</span>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className={cn(
                      'px-2.5 py-1 text-xs rounded-full text-white shadow-sm',
                      getColumnColor(epic.column)
                    )}>
                      {epic.column}
                    </span>
                    <span 
                      className="text-board-text-muted transition-transform duration-200" 
                      style={{ transform: isExpanded ? 'rotate(180deg)' : 'rotate(0deg)' }}
                    >
                      ▼
                    </span>
                  </div>
                </button>
                
                {/* Ticket list (expandable) */}
                {isExpanded && epic.tickets.length > 0 && (
                  <div className="border-t border-board-border glass-subtle">
                    {epic.tickets.map((ticket, idx) => (
                      <div 
                        key={ticket.id}
                        className={cn(
                          'flex items-center justify-between px-4 py-2',
                          idx !== epic.tickets.length - 1 && 'border-b border-board-border'
                        )}
                      >
                        <div className="flex items-center gap-2">
                          <span className={cn(
                            'w-5 h-5 flex items-center justify-center rounded text-white text-xs',
                            getColumnColor(ticket.column)
                          )}>
                            {getColumnIcon(ticket.column)}
                          </span>
                          <span className="text-sm text-board-text-secondary">
                            {ticket.title}
                          </span>
                        </div>
                        <span className={cn(
                          'px-1.5 py-0.5 text-xs rounded text-white',
                          getColumnColor(ticket.column)
                        )}>
                          {ticket.column}
                        </span>
                      </div>
                    ))}
                  </div>
                )}
                
                {/* Empty state for epics with no tickets */}
                {isExpanded && epic.tickets.length === 0 && (
                  <div className="border-t border-board-border glass-subtle px-4 py-3 text-sm text-board-text-muted text-center">
                    No child tickets
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
