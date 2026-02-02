import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import type { ScratchpadProgress, ScratchpadEta } from '../../types';

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
  progress: ScratchpadProgress;
  scratchpadId?: string;
  isWorking: boolean;
  isPaused?: boolean;
  isCompleted: boolean;
}

export function EpicProgressPanel({ progress, scratchpadId, isWorking, isPaused = false, isCompleted }: EpicProgressPanelProps) {
  const [expandedEpics, setExpandedEpics] = useState<Set<string>>(new Set());
  const [eta, setEta] = useState<ScratchpadEta | null>(null);
  
  // Load ETA when working
  useEffect(() => {
    if (!scratchpadId || (!isWorking && !isPaused)) {
      setEta(null);
      return;
    }
    
    const loadEta = async () => {
      try {
        const result = await invoke<ScratchpadEta>('get_scratchpad_eta', { scratchpadId });
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
  }, [scratchpadId, isWorking, isPaused]);
  
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
      case 'Done': return 'bg-green-500';
      case 'Ready': return 'bg-blue-500';
      case 'In Progress': return 'bg-yellow-500';
      case 'Review': return 'bg-purple-500';
      case 'Blocked': return 'bg-red-500';
      case 'Backlog': return 'bg-gray-500';
      default: return 'bg-gray-500';
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
      <div className="bg-gray-100 dark:bg-gray-800 rounded-lg p-4">
        <div className="flex items-center justify-between mb-3">
          <h3 className="font-semibold text-gray-900 dark:text-white">
            Epic Progress
          </h3>
          <div className="flex items-center gap-2">
            {isWorking && (
              <span className="flex items-center gap-1.5 text-sm text-green-600 dark:text-green-400">
                <span className="w-2 h-2 bg-green-500 rounded-full animate-pulse" />
                In Progress
              </span>
            )}
            {isPaused && (
              <span className="flex items-center gap-1.5 text-sm text-yellow-600 dark:text-yellow-400">
                <span className="w-2 h-2 bg-yellow-500 rounded-full" />
                Paused
              </span>
            )}
            {isCompleted && progress.done === progress.total && (
              <span className="flex items-center gap-1.5 text-sm text-green-600 dark:text-green-400">
                <span className="text-lg">✓</span>
                All Complete
              </span>
            )}
            {isCompleted && progress.done < progress.total && (
              <span className="flex items-center gap-1.5 text-sm text-yellow-600 dark:text-yellow-400">
                <span className="text-lg">⚠</span>
                Work Pending
              </span>
            )}
          </div>
        </div>
        
        {/* Progress bar */}
        <div className="mb-3">
          <div className="flex justify-between text-sm text-gray-600 dark:text-gray-400 mb-1">
            <span>{progress.done} of {progress.total} epics complete</span>
            <span>{percentComplete}%</span>
          </div>
          <div className="h-3 bg-gray-300 dark:bg-gray-700 rounded-full overflow-hidden">
            <div 
              className="h-full bg-green-500 transition-all duration-500"
              style={{ width: `${percentComplete}%` }}
            />
          </div>
        </div>
        
        {/* Stats */}
        <div className="flex flex-wrap gap-4 text-sm">
          <div className="flex items-center gap-1">
            <span className="w-3 h-3 bg-green-500 rounded-full" />
            <span className="text-gray-600 dark:text-gray-400">Done: {progress.done}</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="w-3 h-3 bg-blue-500 rounded-full" />
            <span className="text-gray-600 dark:text-gray-400">In Progress: {progress.inProgress}</span>
          </div>
          {progress.blocked > 0 && (
            <div className="flex items-center gap-1">
              <span className="w-3 h-3 bg-red-500 rounded-full" />
              <span className="text-gray-600 dark:text-gray-400">Blocked: {progress.blocked}</span>
            </div>
          )}
        </div>
        
        {/* ETA Display */}
        {eta && (isWorking || isPaused) && (
          <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-700">
            <div className="flex items-center justify-between text-sm">
              <div className="flex items-center gap-2">
                <span className="text-gray-500 dark:text-gray-400">Estimated completion:</span>
                <span className="font-medium text-gray-700 dark:text-gray-300">
                  {eta.estimatedSecondsRemaining != null ? formatDuration(eta.estimatedSecondsRemaining) : 'Calculating...'}
                </span>
              </div>
              <div className="flex items-center gap-1.5">
                <span className={`w-2 h-2 rounded-full ${
                  eta.confidence === 'high' ? 'bg-green-500' :
                  eta.confidence === 'medium' ? 'bg-yellow-500' : 'bg-gray-400'
                }`} />
                <span className="text-xs text-gray-500 dark:text-gray-400 capitalize">
                  {eta.confidence} confidence
                </span>
              </div>
            </div>
            {eta.avgSecondsPerTicket != null && (
              <div className="mt-1 text-xs text-gray-500 dark:text-gray-400">
                Based on avg. {formatDuration(eta.avgSecondsPerTicket)} per ticket 
                ({eta.completedTickets}/{eta.totalTickets} completed)
              </div>
            )}
          </div>
        )}
        
        {/* Execution Flow Info */}
        {isWorking && (
          <div className="mt-3 pt-3 border-t border-gray-200 dark:border-gray-700">
            <div className="text-xs text-gray-600 dark:text-gray-400 space-y-1">
              <div className="flex items-center gap-2">
                <span className="font-medium text-gray-700 dark:text-gray-300">Execution Flow:</span>
              </div>
              <div className="flex items-center gap-1">
                <span className="text-green-600 dark:text-green-400">▶</span>
                <span>{rootEpics.length} root epic{rootEpics.length !== 1 ? 's' : ''} (can start immediately{rootEpics.length > 1 ? ', in parallel' : ''})</span>
              </div>
              {waitingEpics.length > 0 && (
                <div className="flex items-center gap-1">
                  <span className="text-orange-500">⏳</span>
                  <span>{waitingEpics.length} epic{waitingEpics.length !== 1 ? 's' : ''} waiting on dependencies</span>
                </div>
              )}
              {dependentEpics.length > 0 && dependentEpics.length !== waitingEpics.length && (
                <div className="flex items-center gap-1">
                  <span className="text-blue-500">→</span>
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
          <h4 className="font-medium text-gray-900 dark:text-white">Epics</h4>
          <div className="flex gap-2 text-xs">
            <button 
              onClick={expandAll}
              className="text-blue-500 hover:text-blue-600"
            >
              Expand all
            </button>
            <span className="text-gray-400">|</span>
            <button 
              onClick={collapseAll}
              className="text-blue-500 hover:text-blue-600"
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
                className="bg-white dark:bg-gray-800 rounded-lg border border-gray-200 dark:border-gray-700 overflow-hidden"
              >
                {/* Epic header */}
                <button
                  onClick={() => toggleEpic(epic.id)}
                  className="w-full flex items-center justify-between p-3 hover:bg-gray-50 dark:hover:bg-gray-700/30 transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <span className={`w-8 h-8 flex items-center justify-center rounded-full text-white text-sm ${getColumnColor(epic.column)}`}>
                      {getColumnIcon(epic.column)}
                    </span>
                    <div className="text-left">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-gray-900 dark:text-white">
                          {epic.title}
                        </span>
                        {epic.tickets.length > 0 && (
                          <span className="text-xs text-gray-500">
                            ({ticketsDone}/{epic.tickets.length} tickets)
                          </span>
                        )}
                        {epic.dependsOnIds.length === 0 && (
                          <span className="px-1.5 py-0.5 text-xs bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 rounded">
                            root
                          </span>
                        )}
                      </div>
                      {epic.dependsOnTitles.length > 0 && (
                        <div className="flex items-center gap-1 text-xs text-orange-600 dark:text-orange-400 mt-0.5">
                          <span>↳</span>
                          <span>waits for: {epic.dependsOnTitles.join(', ')}</span>
                          {epic.column === 'Backlog' && (
                            <span className="text-gray-500">(blocked)</span>
                          )}
                        </div>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className={`px-2 py-1 text-xs rounded-full text-white ${getColumnColor(epic.column)}`}>
                      {epic.column}
                    </span>
                    <span className="text-gray-400 transition-transform" style={{ transform: isExpanded ? 'rotate(180deg)' : 'rotate(0deg)' }}>
                      ▼
                    </span>
                  </div>
                </button>
                
                {/* Ticket list (expandable) */}
                {isExpanded && epic.tickets.length > 0 && (
                  <div className="border-t border-gray-200 dark:border-gray-600 bg-gray-50 dark:bg-gray-700/50">
                    {epic.tickets.map((ticket, idx) => (
                      <div 
                        key={ticket.id}
                        className={`flex items-center justify-between px-4 py-2 ${idx !== epic.tickets.length - 1 ? 'border-b border-gray-200 dark:border-gray-600' : ''}`}
                      >
                        <div className="flex items-center gap-2">
                          <span className={`w-5 h-5 flex items-center justify-center rounded text-white text-xs ${getColumnColor(ticket.column)}`}>
                            {getColumnIcon(ticket.column)}
                          </span>
                          <span className="text-sm text-gray-700 dark:text-gray-200">
                            {ticket.title}
                          </span>
                        </div>
                        <span className={`px-1.5 py-0.5 text-xs rounded text-white ${getColumnColor(ticket.column)}`}>
                          {ticket.column}
                        </span>
                      </div>
                    ))}
                  </div>
                )}
                
                {/* Empty state for epics with no tickets */}
                {isExpanded && epic.tickets.length === 0 && (
                  <div className="border-t border-gray-200 dark:border-gray-600 bg-gray-50 dark:bg-gray-700/50 px-4 py-3 text-sm text-gray-500 dark:text-gray-400 text-center">
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
