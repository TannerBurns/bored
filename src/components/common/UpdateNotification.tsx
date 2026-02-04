import { useEffect } from 'react';
import { useUpdater } from '../../hooks/useUpdater';
import { cn } from '../../lib/utils';

function DownloadIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1="12" y1="15" x2="12" y2="3" />
    </svg>
  );
}

function RefreshIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="23 4 23 10 17 10" />
      <polyline points="1 20 1 14 7 14" />
      <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
    </svg>
  );
}

function XIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </svg>
  );
}

function SparklesIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z" />
      <path d="M5 3v4" />
      <path d="M19 17v4" />
      <path d="M3 5h4" />
      <path d="M17 19h4" />
    </svg>
  );
}

export function UpdateNotification() {
  const {
    state,
    isDismissed,
    checkForUpdates,
    downloadAndInstall,
    handleRestart,
    dismissUpdate,
  } = useUpdater();

  useEffect(() => {
    // Check for updates on mount, with a small delay to not block app startup
    const timer = setTimeout(() => {
      checkForUpdates();
    }, 3000);

    return () => clearTimeout(timer);
  }, [checkForUpdates]);

  // Don't show notification popup for idle, checking, no-update, error, or dismissed states
  // Errors are only shown when user explicitly checks for updates in Settings
  if (
    state.status === 'idle' || 
    state.status === 'checking' || 
    state.status === 'no-update' ||
    state.status === 'error'
  ) {
    return null;
  }

  if (state.status === 'available' && isDismissed) {
    return null;
  }

  return (
    <div className="fixed bottom-4 right-4 z-50 animate-in slide-in-from-bottom-2 fade-in-0 duration-300">
      <div className="glass-intense rounded-xl shadow-2xl border border-board-border overflow-hidden max-w-sm">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-board-border/50">
          <div className="flex items-center gap-2">
            <SparklesIcon className="w-4 h-4 text-board-accent" />
            <span className="text-sm font-medium text-board-text">Update Available</span>
          </div>
          {state.status === 'available' && (
            <button
              onClick={dismissUpdate}
              className="p-1 rounded-lg text-board-text-muted hover:text-board-text hover:bg-board-card-hover transition-colors"
              aria-label="Dismiss"
            >
              <XIcon className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* Content */}
        <div className="px-4 py-3">
          {state.status === 'available' && (
            <div className="space-y-3">
              <p className="text-sm text-board-text-secondary">
                Version <span className="font-medium text-board-text">{state.update.version}</span> is ready to install.
              </p>
              <button
                onClick={() => downloadAndInstall()}
                className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-board-accent text-white text-sm font-medium rounded-lg hover:bg-board-accent-hover transition-colors shadow-sm"
              >
                <DownloadIcon className="w-4 h-4" />
                Download and Install
              </button>
            </div>
          )}

          {state.status === 'downloading' && (
            <div className="space-y-3">
              <div className="flex items-center justify-between text-sm">
                <span className="text-board-text-secondary">Downloading...</span>
                <span className="text-board-text font-medium">{state.progress}%</span>
              </div>
              <div className="h-2 bg-board-card rounded-full overflow-hidden">
                <div
                  className="h-full bg-board-accent transition-all duration-300 ease-out"
                  style={{ width: `${state.progress}%` }}
                />
              </div>
            </div>
          )}

          {state.status === 'ready' && (
            <div className="space-y-3">
              <p className="text-sm text-board-text-secondary">
                Update downloaded. Restart to apply version <span className="font-medium text-board-text">{state.update.version}</span>.
              </p>
              <button
                onClick={handleRestart}
                className="w-full flex items-center justify-center gap-2 px-4 py-2 bg-status-success text-white text-sm font-medium rounded-lg hover:opacity-90 transition-opacity shadow-sm"
              >
                <RefreshIcon className="w-4 h-4" />
                Restart Now
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
