import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { useSpecStore } from '../../stores/specStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { Button } from '../common/Button';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { PlanViewer } from './PlanViewer';
import { LiveLogPanel } from './LiveLogPanel';
import { EpicProgressPanel } from './EpicProgressPanel';
import { logger } from '../../lib/logger';
import { cn } from '../../lib/utils';
import type { Spec, Exploration, SpecStatus, SpecProgress } from '../../types';

interface SpecDetailProps {
  spec: Spec;
  onClose: () => void;
}

const statusMessages: Record<string, { title: string; subtitle: string; variant?: 'info' | 'error' | 'warning' }> = {
  exploring: {
    title: 'Analyzing codebase...',
    subtitle: 'The agent is exploring your project to understand its structure',
  },
  planning: {
    title: 'Generating work plan...',
    subtitle: 'Creating a structured plan with epics and tickets',
  },
  executing: {
    title: 'Creating epics and tickets...',
    subtitle: 'Setting up your work items based on the approved plan',
  },
  working: {
    title: 'Work in progress...',
    subtitle: 'Agents are working on the epics. Track progress in the Progress tab.',
  },
  paused: {
    title: 'Work paused',
    subtitle: 'Work has been paused. Resume when ready to continue.',
    variant: 'warning',
  },
  halted: {
    title: 'Work halted',
    subtitle: 'Work has been halted. Start again when ready.',
    variant: 'warning',
  },
  failed: {
    title: 'Exploration failed',
    subtitle: 'The agent encountered an error. Check the logs for details and try again.',
    variant: 'error',
  },
};

function ProgressIndicator({ status }: { status: SpecStatus }) {
  const message = statusMessages[status];
  if (!message) return null;

  const isError = message.variant === 'error';
  const isWarning = message.variant === 'warning';

  return (
    <div className={cn(
      'mx-4 mt-4 flex items-center gap-3 p-4 rounded-xl glass',
      isError && 'ring-1 ring-status-error/50 glow-error',
      isWarning && 'ring-1 ring-status-warning/50 glow-warning',
      !isError && !isWarning && 'ring-1 ring-status-info/50'
    )}>
      {isError ? (
        <div className="h-5 w-5 flex-shrink-0 text-status-error">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
          </svg>
        </div>
      ) : isWarning ? (
        <div className="h-5 w-5 flex-shrink-0 text-status-warning">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zM9 9a1 1 0 112 0v4a1 1 0 11-2 0V9zm1-4a1 1 0 100 2 1 1 0 000-2z" clipRule="evenodd" />
          </svg>
        </div>
      ) : (
        <div className="animate-spin h-5 w-5 border-2 border-status-info border-t-transparent rounded-full flex-shrink-0" />
      )}
      <div>
        <p className={cn(
          'font-medium',
          isError && 'text-status-error',
          isWarning && 'text-status-warning',
          !isError && !isWarning && 'text-status-info'
        )}>
          {message.title}
        </p>
        <p className="text-sm text-board-text-muted">
          {message.subtitle}
        </p>
      </div>
    </div>
  );
}

function ExplorationLog({ explorations }: { explorations: Exploration[] }) {
  if (explorations.length === 0) {
    return (
      <div className="text-board-text-muted text-center py-8 glass-subtle rounded-xl">
        No explorations yet
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {explorations.map((exploration, idx) => (
        <div key={idx} className="glass rounded-xl overflow-hidden">
          <div className="glass-subtle px-4 py-3 border-b border-board-border">
            <h4 className="font-medium text-board-text">
              Query {idx + 1}
            </h4>
            <p className="text-sm text-board-text-muted mt-1">
              {exploration.query}
            </p>
          </div>
          <div className="p-4">
            <MarkdownViewer content={exploration.response} />
          </div>
        </div>
      ))}
    </div>
  );
}

export function SpecDetail({ spec, onClose }: SpecDetailProps) {
  const { approvePlan, deleteSpec, getSpec, setCurrentSpec, setStatus, liveLogs, pauseWork, resumeWork, haltWork } = useSpecStore();
  const { plannerAutoApprove, plannerMaxExplorations, plannerModel, plannerTimeoutMinutes, plannerMaxRetries } = useSettingsStore();
  const [activeTab, setActiveTab] = useState<'input' | 'exploration' | 'logs' | 'plan' | 'progress'>('input');
  
  // Filter logs for this spec
  const specLogs = liveLogs.filter(log => log.specId === spec.id);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  const [isStartingWork, setIsStartingWork] = useState(false);
  const [isPausing, setIsPausing] = useState(false);
  const [isResuming, setIsResuming] = useState(false);
  const [isHalting, setIsHalting] = useState(false);
  const [progress, setProgress] = useState<SpecProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  
  // Load progress when status is working, paused, halted, or completed
  useEffect(() => {
    const loadProgress = async () => {
      if (['working', 'paused', 'halted', 'completed', 'executed'].includes(spec.status)) {
        try {
          const prog = await invoke<SpecProgress>('get_spec_progress', { specId: spec.id });
          setProgress(prog);
          
          // Auto-correct status if marked as 'completed' but epics aren't done
          if (spec.status === 'completed' && prog.total > 0 && prog.done < prog.total) {
            logger.info('Auto-correcting spec status from completed to executed', { 
              specId: spec.id, 
              done: prog.done, 
              total: prog.total 
            });
            await setStatus(spec.id, 'executed');
          }
          
          // Auto-correct status if all epics are done but status is not 'completed'
          // This handles edge cases where the backend completion check wasn't triggered
          if (prog.total > 0 && prog.done === prog.total && 
              ['working', 'paused', 'halted', 'executed'].includes(spec.status)) {
            logger.info('Auto-correcting spec status to completed - all epics done', { 
              specId: spec.id, 
              done: prog.done, 
              total: prog.total,
              previousStatus: spec.status
            });
            await setStatus(spec.id, 'completed');
          }
        } catch (err) {
          logger.error('Failed to load progress', err);
        }
      }
    };
    loadProgress();
    
    // Poll for progress updates when working
    if (spec.status === 'working') {
      const interval = setInterval(loadProgress, 5000);
      return () => clearInterval(interval);
    }
  }, [spec.id, spec.status, setStatus]);

  const handleStartPlanner = async () => {
    setIsStarting(true);
    setError(null);
    try {
      const model = spec.model 
        || (plannerModel === 'default' ? undefined : plannerModel);
      const agentKind = spec.agentPref || undefined;
      
      logger.info('Starting planner', { 
        specId: spec.id, 
        agentKind,
        model,
      });
      
      await invoke('start_planner', {
        input: {
          specId: spec.id,
          agentKind,
          maxExplorations: plannerMaxExplorations,
          autoApprove: plannerAutoApprove,
          model,
          timeoutMinutes: plannerTimeoutMinutes,
          maxRetries: plannerMaxRetries,
        },
      });
      
      const updated = await getSpec(spec.id);
      setCurrentSpec(updated);
      logger.info('Planner started successfully', { specId: spec.id });
    } catch (err) {
      logger.error('Failed to start planner', err);
      setError(String(err));
    } finally {
      setIsStarting(false);
    }
  };

  const handleExecutePlan = async () => {
    setIsExecuting(true);
    setError(null);
    try {
      await invoke('execute_plan', { specId: spec.id });
      const updated = await getSpec(spec.id);
      setCurrentSpec(updated);
      logger.info('Plan executed', { specId: spec.id });
    } catch (err) {
      logger.error('Failed to execute plan', err);
      setError(String(err));
    } finally {
      setIsExecuting(false);
    }
  };

  const handleStartWork = async () => {
    setIsStartingWork(true);
    setError(null);
    try {
      await invoke('start_spec_work', { specId: spec.id });
      const updated = await getSpec(spec.id);
      setCurrentSpec(updated);
      logger.info('Work started', { specId: spec.id });
    } catch (err) {
      logger.error('Failed to start work', err);
      setError(String(err));
    } finally {
      setIsStartingWork(false);
    }
  };

  const handleApprove = async () => {
    try {
      await approvePlan(spec.id);
    } catch (err) {
      logger.error('Failed to approve plan:', err);
      setError(String(err));
    }
  };

  const handleRetry = async () => {
    // Reset status to draft so we can start again
    try {
      await setStatus(spec.id, 'draft');
      const updated = await getSpec(spec.id);
      setCurrentSpec(updated);
      setError(null);
      // Now start the planner again
      await handleStartPlanner();
    } catch (err) {
      logger.error('Failed to retry', err);
      setError(String(err));
    }
  };

  const handleDelete = async (deleteTickets = false) => {
    const ticketCount = progress?.totalTickets || 0;
    const message = deleteTickets && ticketCount > 0
      ? `Are you sure you want to delete this spec AND all ${ticketCount} associated tickets (epics and their children)? This cannot be undone.`
      : 'Are you sure you want to delete this spec? The tickets created from it will remain.';
    
    if (!confirm(message)) return;
    
    setIsDeleting(true);
    try {
      await deleteSpec(spec.id, deleteTickets);
      onClose();
    } catch (err) {
      logger.error('Failed to delete spec:', err);
      setError(String(err));
    } finally {
      setIsDeleting(false);
    }
  };

  const handlePause = async () => {
    setIsPausing(true);
    setError(null);
    try {
      await pauseWork(spec.id);
      const updated = await getSpec(spec.id);
      setCurrentSpec(updated);
      logger.info('Work paused', { specId: spec.id });
    } catch (err) {
      logger.error('Failed to pause work', err);
      setError(String(err));
    } finally {
      setIsPausing(false);
    }
  };

  const handleResume = async () => {
    setIsResuming(true);
    setError(null);
    try {
      await resumeWork(spec.id);
      const updated = await getSpec(spec.id);
      setCurrentSpec(updated);
      logger.info('Work resumed', { specId: spec.id });
    } catch (err) {
      logger.error('Failed to resume work', err);
      setError(String(err));
    } finally {
      setIsResuming(false);
    }
  };

  const handleHalt = async () => {
    if (!confirm('Are you sure you want to halt all work? This will stop all active runs and reset tickets to their initial state.')) {
      return;
    }
    setIsHalting(true);
    setError(null);
    try {
      await haltWork(spec.id);
      const updated = await getSpec(spec.id);
      setCurrentSpec(updated);
      logger.info('Work halted', { specId: spec.id });
    } catch (err) {
      logger.error('Failed to halt work', err);
      setError(String(err));
    } finally {
      setIsHalting(false);
    }
  };

  const canStart = spec.status === 'draft';
  const canRetry = spec.status === 'failed';
  const canApprove = spec.status === 'awaiting_approval' && spec.planMarkdown;
  const canExecute = spec.status === 'approved' && spec.planJson;
  const canStartWork = spec.status === 'executed' 
    || spec.status === 'halted'
    || (spec.status === 'completed' && progress !== null && progress.done < progress.total);
  const isWorking = spec.status === 'working';
  const isPaused = spec.status === 'paused';
  const isHalted = spec.status === 'halted';
  const isCompleted = spec.status === 'completed';
  const isProcessing = ['exploring', 'planning', 'executing'].includes(spec.status);
  
  // Pause/resume controls
  const canPause = isWorking;
  const canResume = isPaused;
  const canHalt = isWorking || isPaused;
  
  // Auto-switch to logs tab when processing starts
  useEffect(() => {
    if (isProcessing) {
      setActiveTab('logs');
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isProcessing]);

  const tabs: { id: string; label: string; badge?: string | number; pulse?: boolean }[] = [
    { id: 'input', label: 'User Input' },
    { id: 'logs', label: 'Live Logs', badge: specLogs.length > 0 ? specLogs.length : undefined, pulse: isProcessing },
    { id: 'exploration', label: `Exploration (${spec.explorationLog?.length || 0})` },
    { id: 'plan', label: 'Plan' },
  ];
  
  if ((isWorking || isPaused || isCompleted || canStartWork) && progress) {
    tabs.push({
      id: 'progress',
      label: 'Progress',
      badge: `${progress.done}/${progress.total}`,
      pulse: isWorking,
    });
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-board-border glass-subtle">
        <div>
          <h2 className="text-lg font-semibold text-board-text">
            {spec.name}
          </h2>
          <p className="text-sm text-board-text-muted capitalize flex items-center gap-2">
            Status: 
            <span className="glass-subtle px-2 py-0.5 rounded-full text-xs">
              {spec.status.replace('_', ' ')}
            </span>
          </p>
        </div>
        <div className="flex gap-2">
          {canStart && (
            <Button 
              onClick={handleStartPlanner} 
              variant="primary"
              disabled={isStarting}
            >
              {isStarting ? 'Starting...' : 'Start Exploring'}
            </Button>
          )}
          {canRetry && (
            <Button 
              onClick={handleRetry} 
              variant="primary"
              disabled={isStarting}
            >
              {isStarting ? 'Retrying...' : 'Retry'}
            </Button>
          )}
          {canApprove && (
            <Button onClick={handleApprove} variant="primary">
              Approve Plan
            </Button>
          )}
          {canExecute && (
            <Button 
              onClick={handleExecutePlan} 
              variant="primary"
              disabled={isExecuting}
            >
              {isExecuting ? 'Executing...' : 'Execute Plan'}
            </Button>
          )}
          {canStartWork && (
            <Button 
              onClick={handleStartWork} 
              variant="primary"
              disabled={isStartingWork}
            >
              {isStartingWork ? 'Starting...' : isHalted ? 'Restart Work' : 'Start Work'}
            </Button>
          )}
          {/* Pause/Resume/Halt controls */}
          {canPause && (
            <Button
              onClick={handlePause}
              variant="secondary"
              disabled={isPausing}
            >
              {isPausing ? 'Pausing...' : 'Pause'}
            </Button>
          )}
          {canResume && (
            <Button
              onClick={handleResume}
              variant="primary"
              disabled={isResuming}
            >
              {isResuming ? 'Resuming...' : 'Resume'}
            </Button>
          )}
          {canHalt && (
            <Button
              onClick={handleHalt}
              variant="danger"
              disabled={isHalting}
            >
              {isHalting ? 'Halting...' : 'Halt'}
            </Button>
          )}
          {/* Delete dropdown */}
          <div className="relative group">
            <Button 
              onClick={() => handleDelete(false)} 
              variant="danger" 
              disabled={isDeleting || isProcessing}
            >
              {isDeleting ? 'Deleting...' : 'Delete'}
            </Button>
            {progress && progress.totalTickets > 0 && !isDeleting && !isProcessing && (
              <div className="absolute right-0 top-full mt-1 w-48 glass-intense rounded-xl shadow-lg opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-10 overflow-hidden">
                <button
                  onClick={() => handleDelete(false)}
                  className="w-full px-3 py-2 text-left text-sm text-board-text hover:bg-board-card-hover transition-colors"
                >
                  Delete spec only
                </button>
                <button
                  onClick={() => handleDelete(true)}
                  className="w-full px-3 py-2 text-left text-sm text-status-error hover:bg-status-error/10 transition-colors border-t border-board-border"
                >
                  Delete with {progress.totalTickets} tickets
                </button>
              </div>
            )}
          </div>
          <Button onClick={onClose} variant="secondary">
            Close
          </Button>
        </div>
      </div>

      {/* Error Message */}
      {error && (
        <div className="mx-4 mt-4 p-3 glass rounded-xl ring-1 ring-status-error/50 glow-error">
          <p className="text-sm text-status-error">{error}</p>
        </div>
      )}

      {/* Progress Indicator */}
      <ProgressIndicator status={spec.status} />

      {/* Tabs with glass styling */}
      <div className="flex border-b border-board-border px-4 gap-1">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setActiveTab(tab.id as typeof activeTab)}
            className={cn(
              'px-4 py-2.5 text-sm font-medium transition-all duration-200 relative flex items-center gap-2 rounded-t-lg',
              activeTab === tab.id
                ? 'text-board-accent'
                : 'text-board-text-muted hover:text-board-text hover:bg-board-card-hover'
            )}
          >
            {tab.label}
            {tab.pulse && (
              <span className="inline-block w-2 h-2 bg-status-success rounded-full animate-pulse" />
            )}
            {tab.badge && (
              <span className="text-xs glass-subtle px-1.5 py-0.5 rounded-full">
                {tab.badge}
              </span>
            )}
            {activeTab === tab.id && (
              <div 
                className="absolute bottom-0 left-0 right-0 h-0.5"
                style={{ background: 'var(--app-accent-gradient)' }}
              />
            )}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === 'input' && (
          <div className="glass rounded-xl p-6">
            <h3 className="text-lg font-medium text-board-text mb-4">Original Request</h3>
            <p className="whitespace-pre-wrap text-board-text-secondary">{spec.userInput}</p>
          </div>
        )}
        
        {activeTab === 'logs' && (
          <LiveLogPanel 
            logs={specLogs} 
            isProcessing={isProcessing}
            currentPhase={
              spec.status === 'exploring' ? 'exploration' :
              spec.status === 'planning' ? 'planning' : undefined
            }
          />
        )}

        {activeTab === 'exploration' && (
          <ExplorationLog explorations={spec.explorationLog || []} />
        )}

        {activeTab === 'plan' && (
          spec.planMarkdown ? (
            <PlanViewer
              markdown={spec.planMarkdown}
              planJson={spec.planJson}
            />
          ) : (
            <div className="text-board-text-muted text-center py-8 glass-subtle rounded-xl">
              No plan generated yet
            </div>
          )
        )}
        
        {activeTab === 'progress' && progress && (
          <EpicProgressPanel 
            progress={progress}
            specId={spec.id}
            isWorking={isWorking}
            isPaused={isPaused}
            isCompleted={isCompleted}
          />
        )}
      </div>
    </div>
  );
}
