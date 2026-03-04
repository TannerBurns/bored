import { useEffect, useState, useMemo, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useSpecStore } from '../../stores/specStore';
import { cn } from '../../lib/utils';
import { MarkdownViewer } from '../common/MarkdownViewer';
import { Button } from '../common/Button';
import { ConfirmModal } from '../common/ConfirmModal';
import { PlanViewer } from './PlanViewer';
import { EpicProgressPanel } from './EpicProgressPanel';
import { logger } from '../../lib/logger';
import type { SpecVersion, SpecProgress } from '../../types';

/** Split user input into original request and refined spec */
function parseUserInput(userInput: string): { originalRequest: string; refinedSpec: string | null } {
  const separator = '\n\n---\n';
  const sepIndex = userInput.indexOf(separator);
  
  if (sepIndex === -1) {
    // No separator found - it's just the original request
    return { originalRequest: userInput, refinedSpec: null };
  }
  
  return {
    originalRequest: userInput.substring(0, sepIndex).trim(),
    refinedSpec: userInput.substring(sepIndex + separator.length).trim(),
  };
}

interface VersionsListProps {
  specId: string;
  userInput: string;
  onRefresh: () => Promise<void>;
}

const statusLabels: Record<string, { label: string; color: string }> = {
  conversing: { label: 'Spec Discovery', color: 'text-purple-400' },
  exploring: { label: 'Exploring', color: 'text-blue-400' },
  planning: { label: 'Planning', color: 'text-yellow-400' },
  awaiting_approval: { label: 'Awaiting Approval', color: 'text-orange-400' },
  approved: { label: 'Approved', color: 'text-green-400' },
  executing: { label: 'Executing', color: 'text-blue-400' },
  executed: { label: 'Executed', color: 'text-green-400' },
  working: { label: 'Working', color: 'text-blue-400' },
  paused: { label: 'Paused', color: 'text-yellow-400' },
  halted: { label: 'Halted', color: 'text-red-400' },
  completed: { label: 'Completed', color: 'text-green-400' },
  failed: { label: 'Failed', color: 'text-red-400' },
};

function VersionCard({ version, isSelected, onSelect }: { 
  version: SpecVersion; 
  isSelected: boolean; 
  onSelect: () => void;
}) {
  const status = statusLabels[version.status] || { label: version.status, color: 'text-board-text-muted' };
  
  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full text-left p-4 rounded-xl transition-all',
        isSelected 
          ? 'glass-intense ring-1 ring-board-accent/50' 
          : 'glass-subtle hover:glass'
      )}
    >
      <div className="flex items-center justify-between mb-2">
        <span className="text-lg font-semibold text-board-text">
          Version {version.versionNumber}
        </span>
        <span className={cn('text-sm font-medium', status.color)}>
          {status.label}
        </span>
      </div>
      <div className="text-sm text-board-text-muted">
        {new Date(version.createdAt).toLocaleDateString(undefined, {
          year: 'numeric',
          month: 'short',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit',
        })}
      </div>
      {version.planMarkdown && (
        <div className="mt-2 text-xs text-board-text-muted">
          Plan available
        </div>
      )}
    </button>
  );
}

export function VersionsList({ specId, userInput, onRefresh }: VersionsListProps) {
  const { 
    currentVersions, 
    selectedVersion, 
    loadVersions, 
    selectVersion,
  } = useSpecStore();

  useEffect(() => {
    loadVersions(specId);
  }, [specId, loadVersions]);

  if (currentVersions.length === 0) {
    return (
      <div className="text-board-text-muted text-center py-8 glass-subtle rounded-xl">
        No versions yet
      </div>
    );
  }

  return (
    <div className="flex gap-4 h-full">
      {/* Version list sidebar */}
      <div className="w-64 flex-shrink-0 space-y-2 overflow-y-auto">
        {currentVersions
          .slice()
          .sort((a, b) => b.versionNumber - a.versionNumber)
          .map((version) => (
            <VersionCard
              key={version.id}
              version={version}
              isSelected={selectedVersion?.id === version.id}
              onSelect={() => selectVersion(version)}
            />
          ))}
      </div>

      {/* Version detail */}
      <div className="flex-1 overflow-y-auto">
        {selectedVersion ? (
          <VersionDetail 
            version={selectedVersion} 
            specId={specId} 
            userInput={userInput}
            onRefresh={onRefresh}
          />
        ) : (
          <div className="text-board-text-muted text-center py-8 glass-subtle rounded-xl">
            Select a version to view details
          </div>
        )}
      </div>
    </div>
  );
}

function VersionDetail({ version, specId, userInput, onRefresh }: { 
  version: SpecVersion; 
  specId: string; 
  userInput: string;
  onRefresh: () => Promise<void>;
}) {
  const { approvePlan, pauseWork, resumeWork, haltWork, loadVersions, selectVersionById, scrollToProgress, setScrollToProgress } = useSpecStore();
  
  const progressRef = useRef<HTMLDivElement>(null);

  // Refresh both spec and versions list, then re-select current version
  const handleRefreshAll = async () => {
    await onRefresh();
    await loadVersions(specId);
    // Re-select the current version to get updated data
    selectVersionById(version.id);
  };
  const statusInfo = statusLabels[version.status] || { label: version.status, color: 'text-board-text-muted' };
  const [progress, setProgress] = useState<SpecProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  
  // Action loading states
  const [isExecuting, setIsExecuting] = useState(false);
  const [isStartingWork, setIsStartingWork] = useState(false);
  const [isPausing, setIsPausing] = useState(false);
  const [isResuming, setIsResuming] = useState(false);
  const [isHalting, setIsHalting] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  
  const [showResetConfirm, setShowResetConfirm] = useState(false);
  const [showHaltConfirm, setShowHaltConfirm] = useState(false);
  
  // Parse user input into original request and refined spec
  const { originalRequest, refinedSpec } = useMemo(() => parseUserInput(userInput), [userInput]);
  
  // Status flags
  const status = version.status;
  const isWorking = status === 'working';
  const isPaused = status === 'paused';
  const isCompleted = status === 'completed';
  const isPlanning = status === 'planning';
  const isHalted = status === 'halted';
  const showProgress = ['working', 'paused', 'halted', 'completed', 'executed'].includes(status);
  
  // Action availability
  const canApprove = status === 'awaiting_approval' && version.planMarkdown;
  const canExecute = status === 'approved' && version.planJson;
  const canStartWork = status === 'executed' 
    || status === 'halted'
    || (status === 'completed' && progress !== null && progress.done < progress.total);
  const canPause = isWorking;
  const canResume = isPaused;
  const canHalt = isWorking || isPaused;
  const canReset = ['executed', 'working', 'paused', 'halted', 'completed'].includes(status);

  // Load progress for this specific version
  useEffect(() => {
    const loadProgress = async () => {
      if (showProgress) {
        try {
          // Use version-specific progress command to get correct version's tickets
          const prog = await invoke<SpecProgress>('get_version_progress', { versionId: version.id });
          setProgress(prog);
        } catch (err) {
          console.error('Failed to load progress', err);
        }
      }
    };
    loadProgress();
    
    // Poll for progress updates when working
    if (isWorking) {
      const interval = setInterval(loadProgress, 5000);
      return () => clearInterval(interval);
    }
  }, [version.id, status, showProgress, isWorking]);

  useEffect(() => {
    if (scrollToProgress && progressRef.current && progress) {
      progressRef.current.scrollIntoView({ behavior: 'smooth' });
      setScrollToProgress(false);
    }
  }, [scrollToProgress, progress, setScrollToProgress]);

  // Action handlers
  const handleApprove = async () => {
    setError(null);
    try {
      await approvePlan(specId);
      await handleRefreshAll();
    } catch (err) {
      logger.error('Failed to approve plan:', err);
      setError(String(err));
    }
  };

  const handleExecutePlan = async () => {
    setIsExecuting(true);
    setError(null);
    try {
      await invoke('execute_plan', { specId });
      await handleRefreshAll();
      logger.info('Plan executed', { specId });
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
      await invoke('start_spec_work', { specId });
      await handleRefreshAll();
      logger.info('Work started', { specId });
    } catch (err) {
      logger.error('Failed to start work', err);
      setError(String(err));
    } finally {
      setIsStartingWork(false);
    }
  };

  const handlePause = async () => {
    setIsPausing(true);
    setError(null);
    try {
      await pauseWork(specId);
      await handleRefreshAll();
      logger.info('Work paused', { specId });
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
      await resumeWork(specId);
      await handleRefreshAll();
      logger.info('Work resumed', { specId });
    } catch (err) {
      logger.error('Failed to resume work', err);
      setError(String(err));
    } finally {
      setIsResuming(false);
    }
  };

  const handleHalt = () => {
    setShowHaltConfirm(true);
  };

  const handleHaltConfirm = async () => {
    setIsHalting(true);
    setError(null);
    try {
      await haltWork(specId);
      await handleRefreshAll();
      logger.info('Work halted', { specId });
    } catch (err) {
      logger.error('Failed to halt work', err);
      setError(String(err));
    } finally {
      setIsHalting(false);
    }
  };

  const handleReset = () => {
    setShowResetConfirm(true);
  };

  const handleResetConfirm = async () => {
    setIsResetting(true);
    setError(null);
    try {
      const deletedCount = await invoke<number>('reset_plan_execution', { specId, versionId: version.id });
      await handleRefreshAll();
      logger.info('Plan execution reset', { specId, versionId: version.id, deletedCount });
    } catch (err) {
      logger.error('Failed to reset plan execution', err);
      setError(String(err));
    } finally {
      setIsResetting(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Header with actions */}
      <div className="glass rounded-xl p-4">
        <div className="flex items-center justify-between mb-2">
          <h3 className="text-lg font-semibold text-board-text">
            Version {version.versionNumber}
          </h3>
          <span className={cn('text-sm font-medium px-3 py-1 rounded-full glass-subtle', statusInfo.color)}>
            {statusInfo.label}
          </span>
        </div>
        <p className="text-sm text-board-text-muted mb-3">
          Created {new Date(version.createdAt).toLocaleString()}
        </p>
        
        {/* Action buttons */}
        <div className="flex flex-wrap gap-2">
          {canApprove && (
            <Button onClick={handleApprove} variant="primary" size="sm">
              Approve Plan
            </Button>
          )}
          {canExecute && (
            <Button 
              onClick={handleExecutePlan} 
              variant="primary"
              size="sm"
              disabled={isExecuting}
            >
              {isExecuting ? 'Executing...' : 'Execute Plan'}
            </Button>
          )}
          {canStartWork && (
            <Button 
              onClick={handleStartWork} 
              variant="primary"
              size="sm"
              disabled={isStartingWork}
            >
              {isStartingWork ? 'Starting...' : isHalted ? 'Restart Work' : 'Start Work'}
            </Button>
          )}
          {canPause && (
            <Button
              onClick={handlePause}
              variant="secondary"
              size="sm"
              disabled={isPausing}
            >
              {isPausing ? 'Pausing...' : 'Pause'}
            </Button>
          )}
          {canResume && (
            <Button
              onClick={handleResume}
              variant="primary"
              size="sm"
              disabled={isResuming}
            >
              {isResuming ? 'Resuming...' : 'Resume'}
            </Button>
          )}
          {canHalt && (
            <Button
              onClick={handleHalt}
              variant="danger"
              size="sm"
              disabled={isHalting}
            >
              {isHalting ? 'Halting...' : 'Halt'}
            </Button>
          )}
          {canReset && (
            <Button
              onClick={handleReset}
              variant="secondary"
              size="sm"
              disabled={isResetting}
              title="Delete all tickets and reset to approved status"
            >
              {isResetting ? 'Resetting...' : 'Reset Tickets'}
            </Button>
          )}
        </div>
        
        {/* Error display */}
        {error && (
          <div className="mt-3 p-2 rounded-lg bg-status-error/10 border border-status-error/30">
            <p className="text-sm text-status-error">{error}</p>
          </div>
        )}
      </div>

      {/* Original Request - collapsed by default */}
      <details className="glass rounded-xl overflow-hidden">
        <summary className="glass-subtle px-4 py-3 cursor-pointer hover:bg-board-card-hover transition-colors">
          <span className="font-medium text-board-text">Original Request</span>
        </summary>
        <div className="p-4">
          <MarkdownViewer content={originalRequest} />
        </div>
      </details>

      {/* Refined Spec - collapsed by default, only show if exists */}
      {refinedSpec && (
        <details className="glass rounded-xl overflow-hidden">
          <summary className="glass-subtle px-4 py-3 cursor-pointer hover:bg-board-card-hover transition-colors">
            <span className="font-medium text-board-text">Refined Specification</span>
          </summary>
          <div className="p-4">
            <MarkdownViewer content={refinedSpec} />
          </div>
        </details>
      )}

      {/* Plan - check planMarkdown exists AND we're not still planning */}
      {version.planMarkdown && !isPlanning ? (
        <details className="glass rounded-xl overflow-hidden">
          <summary className="glass-subtle px-4 py-3 cursor-pointer hover:bg-board-card-hover transition-colors">
            <span className="font-medium text-board-text">Work Plan</span>
          </summary>
          <div className="p-4">
            <PlanViewer
              markdown={version.planMarkdown}
              planJson={version.planJson}
            />
          </div>
        </details>
      ) : (
        <div className="text-board-text-muted text-center py-8 glass-subtle rounded-xl">
          {isPlanning ? (
            <div className="flex items-center justify-center gap-2">
              <div className="animate-spin h-5 w-5 border-2 border-board-accent border-t-transparent rounded-full" />
              <span>Generating plan...</span>
            </div>
          ) : version.status === 'conversing' ? (
            'Plan will be generated after spec discovery completes'
          ) : (
            'No plan available'
          )}
        </div>
      )}

      {/* Progress */}
      {showProgress && progress && (
        <div ref={progressRef}>
          <EpicProgressPanel
            progress={progress}
            specId={specId}
            isWorking={isWorking}
            isPaused={isPaused}
            isCompleted={isCompleted}
          />
        </div>
      )}

      <ConfirmModal
        open={showResetConfirm}
        onOpenChange={setShowResetConfirm}
        title="Reset Tickets"
        message="Are you sure you want to delete all tickets and reset to approved status? You can then re-execute the plan to recreate tickets."
        confirmLabel="Reset"
        variant="danger"
        onConfirm={handleResetConfirm}
        onCancel={() => setShowResetConfirm(false)}
      />

      <ConfirmModal
        open={showHaltConfirm}
        onOpenChange={setShowHaltConfirm}
        title="Halt Work"
        message="Are you sure you want to halt all work? This will stop all active runs and reset tickets to their initial state."
        confirmLabel="Halt"
        variant="danger"
        onConfirm={handleHaltConfirm}
        onCancel={() => setShowHaltConfirm(false)}
      />
    </div>
  );
}
