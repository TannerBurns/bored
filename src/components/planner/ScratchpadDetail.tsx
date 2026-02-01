import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { usePlannerStore } from '../../stores/plannerStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { Button } from '../common/Button';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { PlanViewer } from './PlanViewer';
import { LiveLogPanel } from './LiveLogPanel';
import { EpicProgressPanel } from './EpicProgressPanel';
import { logger } from '../../lib/logger';
import type { Scratchpad, Exploration, ScratchpadStatus, ScratchpadProgress } from '../../types';

interface ScratchpadDetailProps {
  scratchpad: Scratchpad;
  onClose: () => void;
}

const statusMessages: Record<string, { title: string; subtitle: string; variant?: 'info' | 'error' }> = {
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
  failed: {
    title: 'Exploration failed',
    subtitle: 'The agent encountered an error. Check the logs for details and try again.',
    variant: 'error',
  },
};

function ProgressIndicator({ status }: { status: ScratchpadStatus }) {
  const message = statusMessages[status];
  if (!message) return null;

  const isError = message.variant === 'error';

  return (
    <div className={`mx-4 mt-4 flex items-center gap-3 p-4 rounded-lg border ${
      isError 
        ? 'bg-red-50 dark:bg-red-900/20 border-red-200 dark:border-red-800' 
        : 'bg-blue-50 dark:bg-blue-900/20 border-blue-200 dark:border-blue-800'
    }`}>
      {isError ? (
        <div className="h-5 w-5 flex-shrink-0 text-red-500">
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor">
            <path fillRule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clipRule="evenodd" />
          </svg>
        </div>
      ) : (
        <div className="animate-spin h-5 w-5 border-2 border-blue-500 border-t-transparent rounded-full flex-shrink-0" />
      )}
      <div>
        <p className={`font-medium ${isError ? 'text-red-700 dark:text-red-300' : 'text-blue-700 dark:text-blue-300'}`}>
          {message.title}
        </p>
        <p className={`text-sm ${isError ? 'text-red-600 dark:text-red-400' : 'text-blue-600 dark:text-blue-400'}`}>
          {message.subtitle}
        </p>
      </div>
    </div>
  );
}

function ExplorationLog({ explorations }: { explorations: Exploration[] }) {
  if (explorations.length === 0) {
    return (
      <div className="text-gray-500 text-center py-8">
        No explorations yet
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {explorations.map((exploration, idx) => (
        <div key={idx} className="border dark:border-gray-700 rounded-lg overflow-hidden">
          <div className="bg-gray-100 dark:bg-gray-800 px-4 py-2">
            <h4 className="font-medium text-gray-900 dark:text-white">
              Query {idx + 1}
            </h4>
            <p className="text-sm text-gray-600 dark:text-gray-300 mt-1">
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

export function ScratchpadDetail({ scratchpad, onClose }: ScratchpadDetailProps) {
  const { approvePlan, deleteScratchpad, getScratchpad, setCurrentScratchpad, setStatus, liveLogs } = usePlannerStore();
  const { plannerAutoApprove, plannerMaxExplorations, plannerModel } = useSettingsStore();
  const [activeTab, setActiveTab] = useState<'input' | 'exploration' | 'logs' | 'plan' | 'progress'>('input');
  
  // Filter logs for this scratchpad
  const scratchpadLogs = liveLogs.filter(log => log.scratchpadId === scratchpad.id);
  const [isDeleting, setIsDeleting] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isExecuting, setIsExecuting] = useState(false);
  const [isStartingWork, setIsStartingWork] = useState(false);
  const [progress, setProgress] = useState<ScratchpadProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  
  // Load progress when status is working or completed
  useEffect(() => {
    const loadProgress = async () => {
      if (scratchpad.status === 'working' || scratchpad.status === 'completed' || scratchpad.status === 'executed') {
        try {
          const prog = await invoke<ScratchpadProgress>('get_scratchpad_progress', { scratchpadId: scratchpad.id });
          setProgress(prog);
          
          // Auto-correct status if marked as 'completed' but epics aren't done
          if (scratchpad.status === 'completed' && prog.total > 0 && prog.done < prog.total) {
            logger.info('Auto-correcting scratchpad status from completed to executed', { 
              scratchpadId: scratchpad.id, 
              done: prog.done, 
              total: prog.total 
            });
            await setStatus(scratchpad.id, 'executed');
          }
        } catch (err) {
          logger.error('Failed to load progress', err);
        }
      }
    };
    loadProgress();
    
    // Poll for progress updates when working
    if (scratchpad.status === 'working') {
      const interval = setInterval(loadProgress, 5000);
      return () => clearInterval(interval);
    }
  }, [scratchpad.id, scratchpad.status, setStatus]);

  const handleStartPlanner = async () => {
    setIsStarting(true);
    setError(null);
    try {
      const model = scratchpad.model 
        || (plannerModel === 'default' ? undefined : plannerModel);
      const agentKind = scratchpad.agentPref || undefined;
      
      logger.info('Starting planner', { 
        scratchpadId: scratchpad.id, 
        agentKind,
        model,
      });
      
      await invoke('start_planner', {
        input: {
          scratchpadId: scratchpad.id,
          agentKind,
          maxExplorations: plannerMaxExplorations,
          autoApprove: plannerAutoApprove,
          model,
        },
      });
      
      const updated = await getScratchpad(scratchpad.id);
      setCurrentScratchpad(updated);
      logger.info('Planner started successfully', { scratchpadId: scratchpad.id });
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
      await invoke('execute_plan', { scratchpadId: scratchpad.id });
      const updated = await getScratchpad(scratchpad.id);
      setCurrentScratchpad(updated);
      logger.info('Plan executed', { scratchpadId: scratchpad.id });
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
      await invoke('start_scratchpad_work', { scratchpadId: scratchpad.id });
      const updated = await getScratchpad(scratchpad.id);
      setCurrentScratchpad(updated);
      logger.info('Work started', { scratchpadId: scratchpad.id });
    } catch (err) {
      logger.error('Failed to start work', err);
      setError(String(err));
    } finally {
      setIsStartingWork(false);
    }
  };

  const handleApprove = async () => {
    try {
      await approvePlan(scratchpad.id);
    } catch (err) {
      logger.error('Failed to approve plan:', err);
      setError(String(err));
    }
  };

  const handleRetry = async () => {
    // Reset status to draft so we can start again
    try {
      await setStatus(scratchpad.id, 'draft');
      const updated = await getScratchpad(scratchpad.id);
      setCurrentScratchpad(updated);
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
      ? `Are you sure you want to delete this scratchpad AND all ${ticketCount} associated tickets (epics and their children)? This cannot be undone.`
      : 'Are you sure you want to delete this scratchpad? The tickets created from it will remain.';
    
    if (!confirm(message)) return;
    
    setIsDeleting(true);
    try {
      await deleteScratchpad(scratchpad.id, deleteTickets);
      onClose();
    } catch (err) {
      logger.error('Failed to delete scratchpad:', err);
      setError(String(err));
    } finally {
      setIsDeleting(false);
    }
  };

  const canStart = scratchpad.status === 'draft';
  const canRetry = scratchpad.status === 'failed';
  const canApprove = scratchpad.status === 'awaiting_approval' && scratchpad.planMarkdown;
  const canExecute = scratchpad.status === 'approved' && scratchpad.planJson;
  const canStartWork = scratchpad.status === 'executed' 
    || (scratchpad.status === 'completed' && progress !== null && progress.done < progress.total);
  const isWorking = scratchpad.status === 'working';
  const isCompleted = scratchpad.status === 'completed';
  const isProcessing = ['exploring', 'planning', 'executing'].includes(scratchpad.status);
  
  // Auto-switch to logs tab when processing starts
  useEffect(() => {
    if (isProcessing) {
      setActiveTab('logs');
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isProcessing]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b dark:border-gray-700">
        <div>
          <h2 className="text-lg font-semibold text-gray-900 dark:text-white">
            {scratchpad.name}
          </h2>
          <p className="text-sm text-gray-500 capitalize">
            Status: {scratchpad.status.replace('_', ' ')}
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
              {isStartingWork ? 'Starting...' : 'Start Work'}
            </Button>
          )}
          {/* Delete dropdown */}
          <div className="relative group">
            <Button 
              onClick={() => handleDelete(false)} 
              variant="secondary" 
              disabled={isDeleting || isProcessing}
              className="text-red-500 hover:text-red-600 border-red-300 hover:border-red-400"
            >
              {isDeleting ? 'Deleting...' : 'Delete'}
            </Button>
            {progress && progress.totalTickets > 0 && !isDeleting && !isProcessing && (
              <div className="absolute right-0 top-full mt-1 w-48 bg-white dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg shadow-lg opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-10">
                <button
                  onClick={() => handleDelete(false)}
                  className="w-full px-3 py-2 text-left text-sm hover:bg-gray-100 dark:hover:bg-gray-700 rounded-t-lg"
                >
                  Delete scratchpad only
                </button>
                <button
                  onClick={() => handleDelete(true)}
                  className="w-full px-3 py-2 text-left text-sm text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20 rounded-b-lg border-t border-gray-200 dark:border-gray-700"
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
        <div className="mx-4 mt-4 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
          <p className="text-sm text-red-700 dark:text-red-300">{error}</p>
        </div>
      )}

      {/* Progress Indicator */}
      <ProgressIndicator status={scratchpad.status} />

      {/* Tabs */}
      <div className="flex border-b dark:border-gray-700">
        <button
          onClick={() => setActiveTab('input')}
          className={`px-4 py-2 text-sm font-medium ${
            activeTab === 'input'
              ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
              : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
          }`}
        >
          User Input
        </button>
        <button
          onClick={() => setActiveTab('logs')}
          className={`px-4 py-2 text-sm font-medium flex items-center gap-1.5 ${
            activeTab === 'logs'
              ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
              : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
          }`}
        >
          Live Logs
          {isProcessing && (
            <span className="inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
          )}
          {scratchpadLogs.length > 0 && (
            <span className="text-xs bg-gray-200 dark:bg-gray-700 px-1.5 rounded">
              {scratchpadLogs.length}
            </span>
          )}
        </button>
        <button
          onClick={() => setActiveTab('exploration')}
          className={`px-4 py-2 text-sm font-medium ${
            activeTab === 'exploration'
              ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
              : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
          }`}
        >
          Exploration ({scratchpad.explorationLog?.length || 0})
        </button>
        <button
          onClick={() => setActiveTab('plan')}
          className={`px-4 py-2 text-sm font-medium ${
            activeTab === 'plan'
              ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
              : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
          }`}
        >
          Plan
        </button>
        {(isWorking || isCompleted || canStartWork) && progress && (
          <button
            onClick={() => setActiveTab('progress')}
            className={`px-4 py-2 text-sm font-medium flex items-center gap-1.5 ${
              activeTab === 'progress'
                ? 'border-b-2 border-blue-500 text-blue-600 dark:text-blue-400'
                : 'text-gray-500 hover:text-gray-700 dark:hover:text-gray-300'
            }`}
          >
            Progress
            {isWorking && (
              <span className="inline-block w-2 h-2 bg-green-500 rounded-full animate-pulse" />
            )}
            <span className="text-xs bg-gray-200 dark:bg-gray-700 px-1.5 rounded">
              {progress.done}/{progress.total}
            </span>
          </button>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === 'input' && (
          <div className="prose dark:prose-invert max-w-none">
            <h3>Original Request</h3>
            <p className="whitespace-pre-wrap">{scratchpad.userInput}</p>
          </div>
        )}
        
        {activeTab === 'logs' && (
          <LiveLogPanel 
            logs={scratchpadLogs} 
            isProcessing={isProcessing}
            currentPhase={
              scratchpad.status === 'exploring' ? 'exploration' :
              scratchpad.status === 'planning' ? 'planning' : undefined
            }
          />
        )}

        {activeTab === 'exploration' && (
          <ExplorationLog explorations={scratchpad.explorationLog || []} />
        )}

        {activeTab === 'plan' && (
          scratchpad.planMarkdown ? (
            <PlanViewer
              markdown={scratchpad.planMarkdown}
              planJson={scratchpad.planJson}
            />
          ) : (
            <div className="text-gray-500 text-center py-8">
              No plan generated yet
            </div>
          )
        )}
        
        {activeTab === 'progress' && progress && (
          <EpicProgressPanel 
            progress={progress}
            isWorking={isWorking}
            isCompleted={isCompleted}
          />
        )}
      </div>
    </div>
  );
}
