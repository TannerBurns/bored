import { useState } from 'react';
import { useSettingsStore } from '../../stores/settingsStore';
import { useBoardStore } from '../../stores/boardStore';
import { isTauri } from '../../lib/utils';
import { cleanupStaleRuns } from '../../lib/tauri';

function TrashIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="3 6 5 6 21 6" />
      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
      <line x1="10" y1="11" x2="10" y2="17" />
      <line x1="14" y1="11" x2="14" y2="17" />
    </svg>
  );
}

function AlertIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z" />
      <line x1="12" y1="9" x2="12" y2="13" />
      <line x1="12" y1="17" x2="12.01" y2="17" />
    </svg>
  );
}

function CheckIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  );
}

function RefreshIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21.5 2v6h-6M2.5 22v-6h6M2 11.5a10 10 0 0 1 18.8-4.3M22 12.5a10 10 0 0 1-18.8 4.3"/>
    </svg>
  );
}

export function DataSettings() {
  const [showConfirm, setShowConfirm] = useState(false);
  const [isResetting, setIsResetting] = useState(false);
  const [resetComplete, setResetComplete] = useState(false);
  const [confirmText, setConfirmText] = useState('');
  
  // Stale run cleanup state
  const [isCleaningRuns, setIsCleaningRuns] = useState(false);
  const [cleanupResult, setCleanupResult] = useState<{ count: number; error?: string } | null>(null);
  
  const handleCleanupStaleRuns = async () => {
    if (!isTauri()) return;
    
    setIsCleaningRuns(true);
    setCleanupResult(null);
    
    try {
      const count = await cleanupStaleRuns();
      setCleanupResult({ count });
    } catch (error) {
      console.error('Failed to cleanup stale runs:', error);
      setCleanupResult({ count: 0, error: String(error) });
    } finally {
      setIsCleaningRuns(false);
    }
  };

  const handleFactoryReset = async () => {
    if (confirmText !== 'RESET') return;
    
    setIsResetting(true);
    
    try {
      // Clear localStorage for settings
      localStorage.removeItem('agent-kanban-settings');
      
      // Reset the settings store to defaults
      useSettingsStore.setState({
        theme: 'dark',
        defaultAgentPref: 'any',
      });
      
      // Reset the board store to empty state
      useBoardStore.setState({
        boards: [],
        currentBoard: null,
        columns: [],
        tickets: [],
        selectedTicket: null,
        comments: [],
        isLoading: false,
        error: null,
        isTicketModalOpen: false,
        isCreateModalOpen: false,
      });
      
      // If running in Tauri, we could call a backend command to clear the database
      // For now, we just clear the frontend state
      if (isTauri()) {
        // TODO: Add Tauri command to clear database
        // await invoke('factory_reset');
      }
      
      setResetComplete(true);
      setShowConfirm(false);
      setConfirmText('');
      
      // Reload the page after a brief delay to show success state
      setTimeout(() => {
        window.location.reload();
      }, 1500);
    } catch (error) {
      console.error('Factory reset failed:', error);
    } finally {
      setIsResetting(false);
    }
  };

  const handleCancel = () => {
    setShowConfirm(false);
    setConfirmText('');
  };

  if (resetComplete) {
    return (
      <div className="space-y-6">
        <div>
          <h2 className="text-xl font-semibold text-board-text">Data Management</h2>
          <p className="text-sm text-board-text-muted mt-1">
            Manage your application data and storage.
          </p>
        </div>

        <div className="bg-status-success/10 border border-status-success/30 rounded-xl p-6 flex items-center gap-4">
          <div className="p-3 bg-status-success/20 rounded-full">
            <CheckIcon className="w-6 h-6 text-status-success" />
          </div>
          <div>
            <h3 className="font-medium text-status-success">Reset Complete</h3>
            <p className="text-sm text-board-text-secondary mt-0.5">
              Application data has been cleared. Reloading...
            </p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-board-text">Data Management</h2>
        <p className="text-sm text-board-text-muted mt-1">
          Manage your application data and storage.
        </p>
      </div>

      {/* Storage Info */}
      <div className="bg-board-surface rounded-xl p-5 space-y-4 border border-board-border">
        <div>
          <h3 className="font-medium text-board-text">Local Storage</h3>
          <p className="text-sm text-board-text-muted mt-0.5">
            Settings and preferences are stored locally in your browser.
          </p>
        </div>

        <div className="grid grid-cols-2 gap-4">
          <div className="bg-board-surface-raised rounded-lg p-4 border border-board-border">
            <p className="text-xs text-board-text-muted uppercase tracking-wide">Settings</p>
            <p className="text-lg font-semibold text-board-text mt-1">Stored</p>
          </div>
          <div className="bg-board-surface-raised rounded-lg p-4 border border-board-border">
            <p className="text-xs text-board-text-muted uppercase tracking-wide">Environment</p>
            <p className="text-lg font-semibold text-board-text mt-1">
              {isTauri() ? 'Desktop App' : 'Web Browser'}
            </p>
          </div>
        </div>
      </div>

      {/* Cleanup Stale Runs */}
      {isTauri() && (
        <div className="bg-board-surface rounded-xl p-5 space-y-4 border border-board-border">
          <div className="flex items-start gap-3">
            <div className="p-2 bg-status-warning/10 rounded-lg">
              <RefreshIcon className="w-5 h-5 text-status-warning" />
            </div>
            <div>
              <h3 className="font-medium text-board-text">Cleanup Stale Runs</h3>
              <p className="text-sm text-board-text-muted mt-0.5">
                Mark any stuck "Running" or "Queued" agent runs as aborted. Use this if you have 
                runs that are stuck and never completed.
              </p>
            </div>
          </div>

          {cleanupResult && (
            <div className={`rounded-lg p-3 ${cleanupResult.error ? 'bg-status-error/10 border border-status-error/30' : 'bg-status-success/10 border border-status-success/30'}`}>
              {cleanupResult.error ? (
                <p className="text-sm text-status-error">{cleanupResult.error}</p>
              ) : (
                <p className="text-sm text-status-success">
                  {cleanupResult.count === 0 
                    ? 'No stale runs found.' 
                    : `Cleaned up ${cleanupResult.count} stale run${cleanupResult.count === 1 ? '' : 's'}.`}
                </p>
              )}
            </div>
          )}

          <button
            onClick={handleCleanupStaleRuns}
            disabled={isCleaningRuns}
            className="px-4 py-2 bg-status-warning/10 text-status-warning border border-status-warning/30 rounded-lg hover:bg-status-warning/20 disabled:opacity-50 transition-colors font-medium"
          >
            {isCleaningRuns ? 'Cleaning...' : 'Cleanup Stale Runs'}
          </button>
        </div>
      )}

      {/* Factory Reset */}
      <div className="bg-board-surface rounded-xl p-5 space-y-4 border border-status-error/30">
        <div className="flex items-start gap-3">
          <div className="p-2 bg-status-error/10 rounded-lg">
            <TrashIcon className="w-5 h-5 text-status-error" />
          </div>
          <div>
            <h3 className="font-medium text-board-text">Factory Reset</h3>
            <p className="text-sm text-board-text-muted mt-0.5">
              Clear all application data and restore default settings. This will remove all boards, 
              tickets, and preferences. This action cannot be undone.
            </p>
          </div>
        </div>

        {!showConfirm ? (
          <button
            onClick={() => setShowConfirm(true)}
            className="px-4 py-2 bg-status-error/10 text-status-error border border-status-error/30 rounded-lg hover:bg-status-error/20 transition-colors font-medium"
          >
            Reset All Data
          </button>
        ) : (
          <div className="bg-status-error/5 border border-status-error/20 rounded-lg p-4 space-y-4">
            <div className="flex items-start gap-3">
              <AlertIcon className="w-5 h-5 text-status-error flex-shrink-0 mt-0.5" />
              <div>
                <p className="font-medium text-status-error">Are you absolutely sure?</p>
                <p className="text-sm text-board-text-secondary mt-1">
                  This will permanently delete all your data. Type <span className="font-mono font-bold">RESET</span> to confirm.
                </p>
              </div>
            </div>
            
            <input
              type="text"
              value={confirmText}
              onChange={(e) => setConfirmText(e.target.value)}
              placeholder="Type RESET to confirm"
              className="w-full px-3 py-2.5 bg-board-surface-raised rounded-lg text-board-text placeholder-board-text-muted focus:outline-none focus:ring-2 focus:ring-status-error border border-board-border font-mono"
              autoFocus
            />

            <div className="flex gap-2">
              <button
                onClick={handleCancel}
                className="px-4 py-2 text-board-text-muted hover:text-board-text transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleFactoryReset}
                disabled={confirmText !== 'RESET' || isResetting}
                className="px-4 py-2 bg-status-error text-white rounded-lg hover:bg-status-error/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors font-medium"
              >
                {isResetting ? 'Resetting...' : 'Confirm Reset'}
              </button>
            </div>
          </div>
        )}
      </div>

    </div>
  );
}
