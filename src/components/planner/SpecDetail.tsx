import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useSpecStore } from '../../stores/specStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { Button } from '../common/Button';
import { ConfirmModal } from '../common/ConfirmModal';
import { VersionsList } from './VersionsList';
import { logger } from '../../lib/logger';
import { cn } from '../../lib/utils';
import type { SpecWithVersion, SpecVersionStatus } from '../../types';

interface SpecDetailProps {
  spec: SpecWithVersion;
  onClose: () => void;
  onOpenChat?: (specId: string) => void;
}

const statusMessages: Record<string, { title: string; subtitle: string; variant?: 'info' | 'error' | 'warning' }> = {
  conversing: {
    title: 'Spec discovery session active',
    subtitle: 'Chat with the AI to refine your requirements before exploration',
  },
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

function ProgressIndicator({ status }: { status: SpecVersionStatus }) {
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

export function SpecDetail({ spec, onClose, onOpenChat }: SpecDetailProps) {
  const deleteSpec = useSpecStore((s) => s.deleteSpec);
  const getSpec = useSpecStore((s) => s.getSpec);
  const setCurrentSpec = useSpecStore((s) => s.setCurrentSpec);
  const setStatus = useSpecStore((s) => s.setStatus);
  const agentConfig = useSettingsStore((s) => s.agentConfigs['claude'] ?? s.getAgentConfig('claude'));
  
  const version = spec.latestVersion;
  const status = version?.status ?? 'conversing';
  const [isDeleting, setIsDeleting] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleRefresh = async () => {
    const updated = await getSpec(spec.id);
    setCurrentSpec(updated);
  };

  const handleStartPlanner = async () => {
    setIsStarting(true);
    setError(null);
    try {
      const model = spec.model || agentConfig.plannerModel;
      
      logger.info('Starting planner', { 
        specId: spec.id, 
        model,
      });
      
      await invoke('start_planner', {
        input: {
          specId: spec.id,
          autoApprove: agentConfig.plannerAutoApprove,
          model,
          timeoutMinutes: agentConfig.plannerTimeoutMinutes,
          maxRetries: agentConfig.plannerMaxRetries,
        },
      });
      
      await handleRefresh();
      logger.info('Planner started successfully', { specId: spec.id });
    } catch (err) {
      logger.error('Failed to start planner', err);
      setError(String(err));
    } finally {
      setIsStarting(false);
    }
  };

  const handleRetry = async () => {
    try {
      await setStatus(spec.id, 'draft');
      await handleRefresh();
      setError(null);
      await handleStartPlanner();
    } catch (err) {
      logger.error('Failed to retry', err);
      setError(String(err));
    }
  };

  const handleDeleteConfirm = async () => {
    setIsDeleting(true);
    try {
      await deleteSpec(spec.id);
      onClose();
    } catch (err) {
      logger.error('Failed to delete spec:', err);
      setError(String(err));
    } finally {
      setIsDeleting(false);
    }
  };

  const canRetry = status === 'failed';
  const isProcessing = ['exploring', 'planning', 'executing'].includes(status);

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between p-4 border-b border-board-border glass-subtle">
        <div>
          <h2 className="text-lg font-semibold text-board-text">
            {spec.name}
          </h2>
          <p className="text-sm text-board-text-muted capitalize flex items-center gap-2">
            Status: 
            <span className="glass-subtle px-2 py-0.5 rounded-full text-xs">
              {status.replace('_', ' ')}
            </span>
          </p>
        </div>
        <div className="flex gap-2">
          {onOpenChat && (
            <Button
              onClick={() => onOpenChat(spec.id)}
              variant="secondary"
            >
              Open Chat
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
          <Button 
            onClick={() => setShowDeleteConfirm(true)} 
            variant="danger" 
            disabled={isDeleting || isProcessing}
          >
            {isDeleting ? 'Deleting...' : 'Delete'}
          </Button>
          <Button onClick={onClose} variant="secondary">
            Close
          </Button>
        </div>
      </div>

      {error && (
        <div className="mx-4 mt-4 p-3 glass rounded-xl ring-1 ring-status-error/50 glow-error">
          <p className="text-sm text-status-error">{error}</p>
        </div>
      )}

      <ProgressIndicator status={status} />

      <div className="flex-1 overflow-hidden">
        <div className="h-full p-4">
          <VersionsList specId={spec.id} userInput={spec.userInput} onRefresh={handleRefresh} />
        </div>
      </div>

      <ConfirmModal
        open={showDeleteConfirm}
        onOpenChange={setShowDeleteConfirm}
        title="Delete Spec"
        message="Are you sure you want to delete this spec? This cannot be undone."
        confirmLabel="Delete"
        variant="danger"
        onConfirm={handleDeleteConfirm}
        onCancel={() => setShowDeleteConfirm(false)}
      />
    </div>
  );
}
