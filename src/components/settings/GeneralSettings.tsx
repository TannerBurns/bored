import { useState, useEffect } from 'react';
import { getVersion } from '@tauri-apps/api/app';
import { useSettingsStore } from '../../stores/settingsStore';
import { useUpdater } from '../../hooks/useUpdater';
import { cn } from '../../lib/utils';

function SunIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41" />
    </svg>
  );
}

function MoonIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
    </svg>
  );
}

function MonitorIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="2" y="3" width="20" height="14" rx="2" ry="2" />
      <line x1="8" y1="21" x2="16" y2="21" />
      <line x1="12" y1="17" x2="12" y2="21" />
    </svg>
  );
}

const themeOptions = [
  { value: 'light', label: 'Light', description: 'Bright and clean', Icon: SunIcon },
  { value: 'dark', label: 'Dark', description: 'Easy on the eyes', Icon: MoonIcon },
  { value: 'system', label: 'System', description: 'Match your OS', Icon: MonitorIcon },
] as const;

const plannerModelOptions = [
  { value: 'opus-4.5', label: 'Opus 4.5', description: 'Most capable', isDefault: true },
  { value: 'opus', label: 'Opus 4.6', description: 'Latest generation', isDefault: false },
  { value: 'sonnet', label: 'Sonnet 4.5', description: 'Fast and capable', isDefault: false },
] as const;

function RefreshIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <polyline points="23 4 23 10 17 10" />
      <polyline points="1 20 1 14 7 14" />
      <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
    </svg>
  );
}

function CheckCircleIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
      <polyline points="22 4 12 14.01 9 11.01" />
    </svg>
  );
}

function DownloadIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1="12" y1="15" x2="12" y2="3" />
    </svg>
  );
}

function LoaderIcon({ className }: { className?: string }) {
  return (
    <svg className={cn(className, 'animate-spin')} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <line x1="12" y1="2" x2="12" y2="6" />
      <line x1="12" y1="18" x2="12" y2="22" />
      <line x1="4.93" y1="4.93" x2="7.76" y2="7.76" />
      <line x1="16.24" y1="16.24" x2="19.07" y2="19.07" />
      <line x1="2" y1="12" x2="6" y2="12" />
      <line x1="18" y1="12" x2="22" y2="12" />
      <line x1="4.93" y1="19.07" x2="7.76" y2="16.24" />
      <line x1="16.24" y1="7.76" x2="19.07" y2="4.93" />
    </svg>
  );
}

export function GeneralSettings() {
  const [appVersion, setAppVersion] = useState<string>('');
  const {
    state: updateState,
    isDismissed,
    checkForUpdates,
    downloadAndInstall,
    handleRestart,
    undoDismiss,
    reset,
  } = useUpdater();

  useEffect(() => {
    getVersion().then(setAppVersion).catch(() => setAppVersion('unknown'));
  }, []);

  const { 
    theme, 
    setTheme, 
    plannerAutoApprove,
    setPlannerAutoApprove,
    plannerModel,
    setPlannerModel,
    plannerMaxExplorations,
    setPlannerMaxExplorations,
    plannerTimeoutMinutes,
    setPlannerTimeoutMinutes,
    plannerMaxRetries,
    setPlannerMaxRetries,
    codeReviewMaxIterations,
    setCodeReviewMaxIterations,
    stageTimeoutMinutes,
    setStageTimeoutMinutes,
    stageMaxRetries,
    setStageMaxRetries,
  } = useSettingsStore();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">General</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Configure general application settings.
        </p>
      </div>

      {/* Spec Agent Settings Section */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Spec Agent Settings</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Configure how the AI spec agent explores codebases and generates work plans.
          </p>
        </div>

        {/* Auto-approve toggle */}
        <div className="flex items-center justify-between glass-subtle rounded-lg px-3 py-2">
          <div>
            <span className="text-sm font-medium text-board-text">Auto-approve Plans</span>
            <p className="text-xs text-board-text-muted">
              Automatically approve generated plans without manual review
            </p>
          </div>
          <button
            onClick={() => setPlannerAutoApprove(!plannerAutoApprove)}
            className={cn(
              'relative inline-flex h-5 w-9 flex-shrink-0 cursor-pointer rounded-full transition-colors duration-200 ease-in-out focus:outline-none focus:ring-1 focus:ring-board-accent',
              plannerAutoApprove ? 'bg-board-accent' : 'glass'
            )}
          >
            <span
              className={cn(
                'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
                plannerAutoApprove ? 'translate-x-4' : 'translate-x-0.5'
              )}
              style={{ marginTop: '2px' }}
            />
          </button>
        </div>

        {/* Max explorations */}
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">
            Max Exploration Queries
          </label>
          <input
            type="number"
            min={1}
            max={50}
            value={plannerMaxExplorations}
            onChange={(e) => setPlannerMaxExplorations(parseInt(e.target.value) || 10)}
            className="w-20 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
          />
          <p className="text-xs text-board-text-muted mt-0.5">
            Maximum exploration queries before generating a plan (1-50)
          </p>
        </div>

        {/* Spec agent timeout and retries */}
        <div className="grid grid-cols-2 gap-2">
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">
              Timeout (min)
            </label>
            <input
              type="number"
              min={1}
              max={30}
              value={plannerTimeoutMinutes}
              onChange={(e) => setPlannerTimeoutMinutes(parseInt(e.target.value) || 10)}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
            />
          </div>
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">
              Max Retries
            </label>
            <input
              type="number"
              min={0}
              max={5}
              value={plannerMaxRetries}
              onChange={(e) => setPlannerMaxRetries(parseInt(e.target.value) || 2)}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
            />
          </div>
        </div>

        {/* Spec agent model preference */}
        <div>
          <label className="block text-sm font-medium text-board-text mb-1.5">
            Spec Agent Model
          </label>
          <div className="grid grid-cols-3 gap-1.5">
            {plannerModelOptions.map((option) => {
              const isSelected = plannerModel === option.value;
              return (
                <button
                  key={option.value}
                  onClick={() => setPlannerModel(option.value)}
                  className={cn(
                    'flex flex-col items-center gap-0.5 px-1.5 py-1.5 rounded-lg transition-all duration-200',
                    isSelected
                      ? 'glass-intense ring-1 ring-board-accent'
                      : 'glass hover:glass-intense'
                  )}
                >
                  <span className="flex items-center gap-1">
                    <span className={cn(
                      'text-xs font-medium',
                      isSelected ? 'text-board-accent' : 'text-board-text'
                    )}>
                      {option.label}
                    </span>
                    {option.isDefault && (
                      <span className="text-[9px] font-medium px-1 py-0 rounded-full bg-board-accent/15 text-board-accent leading-relaxed">
                        default
                      </span>
                    )}
                  </span>
                  <span className="text-[11px] text-board-text-muted">
                    {option.description}
                  </span>
                </button>
              );
            })}
          </div>
        </div>
      </div>

      {/* Workflow Stage Settings Section */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Workflow Stage Settings</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Configure timeouts and retries for multi-stage workflow execution.
          </p>
        </div>

        {/* Stage timeout and retries */}
        <div className="grid grid-cols-2 gap-2">
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">
              Stage Timeout (min)
            </label>
            <input
              type="number"
              min={1}
              max={60}
              value={stageTimeoutMinutes}
              onChange={(e) => setStageTimeoutMinutes(parseInt(e.target.value) || 30)}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
            />
          </div>
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">
              Stage Max Retries
            </label>
            <input
              type="number"
              min={0}
              max={5}
              value={stageMaxRetries}
              onChange={(e) => setStageMaxRetries(parseInt(e.target.value) || 2)}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
            />
          </div>
        </div>

        {/* Code Review Max Iterations */}
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">
            Code Review Max Iterations
          </label>
          <input
            type="number"
            min={0}
            max={10}
            value={codeReviewMaxIterations}
            onChange={(e) => setCodeReviewMaxIterations(parseInt(e.target.value) || 3)}
            className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
          />
          <p className="text-xs text-board-text-muted mt-0.5">
            Max iterations before proceeding (0 to disable)
          </p>
        </div>
      </div>

      {/* Theme Section */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Theme</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Select your preferred color scheme.
          </p>
        </div>

        <div className="grid grid-cols-3 gap-2">
          {themeOptions.map((option) => {
            const isSelected = theme === option.value;
            return (
              <button
                key={option.value}
                onClick={() => setTheme(option.value)}
                className={cn(
                  'group relative flex flex-col items-center gap-1.5 px-3 py-2 rounded-lg transition-all duration-200',
                  isSelected
                    ? 'glass-intense ring-1 ring-board-accent'
                    : 'glass hover:glass-intense'
                )}
              >
                <div className={cn(
                  'p-2 rounded-full transition-all duration-200',
                  isSelected 
                    ? 'bg-board-accent text-white' 
                    : 'glass text-board-text-secondary group-hover:text-board-text'
                )}>
                  <option.Icon className="w-4 h-4" />
                </div>
                <div className="text-center">
                  <span className={cn(
                    'block text-sm font-medium',
                    isSelected ? 'text-board-accent' : 'text-board-text'
                  )}>
                    {option.label}
                  </span>
                  <span className="block text-xs text-board-text-muted">
                    {option.description}
                  </span>
                </div>
                {isSelected && (
                  <div className="absolute top-1.5 right-1.5">
                    <svg className="w-4 h-4 text-board-accent" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z" />
                    </svg>
                  </div>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {/* About Section */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">About</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Application version and updates.
          </p>
        </div>

        <div className="glass-subtle rounded-lg px-3 py-2 space-y-3">
          <div className="flex items-center justify-between">
            <div>
              <span className="text-sm font-medium text-board-text">Bored</span>
              <p className="text-xs text-board-text-muted">
                Version {appVersion || '...'}
              </p>
            </div>
            
            {updateState.status === 'idle' && (
              <button
                onClick={checkForUpdates}
                className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium glass rounded-lg text-board-text hover:glass-intense transition-all"
              >
                <RefreshIcon className="w-4 h-4" />
                Check for Updates
              </button>
            )}

            {updateState.status === 'checking' && (
              <div className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-board-text-muted">
                <LoaderIcon className="w-4 h-4" />
                Checking...
              </div>
            )}

            {updateState.status === 'no-update' && (
              <div className="flex items-center gap-2">
                <div className="flex items-center gap-1.5 px-3 py-1.5 text-sm text-status-success">
                  <CheckCircleIcon className="w-4 h-4" />
                  Up to date
                </div>
                <button
                  onClick={() => { reset(); checkForUpdates(); }}
                  className="px-2 py-1 text-xs glass rounded text-board-text hover:glass-intense transition-all"
                  title="Check again"
                >
                  <RefreshIcon className="w-3.5 h-3.5" />
                </button>
              </div>
            )}

            {updateState.status === 'available' && !isDismissed && (
              <button
                onClick={() => downloadAndInstall()}
                className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium bg-board-accent text-white rounded-lg hover:bg-board-accent-hover transition-all shadow-sm"
              >
                <DownloadIcon className="w-4 h-4" />
                Update to {updateState.update.version}
              </button>
            )}

            {updateState.status === 'available' && isDismissed && (
              <div className="flex items-center gap-2">
                <span className="text-xs text-board-text-muted">
                  v{updateState.update.version} dismissed
                </span>
                <button
                  onClick={undoDismiss}
                  className="px-2 py-1 text-xs glass rounded text-board-text hover:glass-intense transition-all"
                >
                  Show Update
                </button>
              </div>
            )}

            {updateState.status === 'downloading' && (
              <div className="flex items-center gap-2">
                <div className="w-24 h-2 bg-board-card rounded-full overflow-hidden">
                  <div
                    className="h-full bg-board-accent transition-all duration-300"
                    style={{ width: `${updateState.progress}%` }}
                  />
                </div>
                <span className="text-xs text-board-text-muted">{updateState.progress}%</span>
              </div>
            )}

            {updateState.status === 'ready' && (
              <button
                onClick={handleRestart}
                className="flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium bg-status-success text-white rounded-lg hover:opacity-90 transition-all shadow-sm"
              >
                <RefreshIcon className="w-4 h-4" />
                Restart to Update
              </button>
            )}

            {updateState.status === 'error' && (
              <div className="flex items-center gap-2">
                <span className="text-xs text-status-error">{updateState.message}</span>
                <button
                  onClick={checkForUpdates}
                  className="px-2 py-1 text-xs glass rounded text-board-text hover:glass-intense transition-all"
                >
                  Retry
                </button>
              </div>
            )}
          </div>

          {updateState.status === 'available' && !isDismissed && (
            <p className="text-xs text-board-text-muted">
              A new version is available. Click the button above to download and install.
            </p>
          )}

          {updateState.status === 'ready' && (
            <p className="text-xs text-board-text-muted">
              Update downloaded. Restart the application to apply the update.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
