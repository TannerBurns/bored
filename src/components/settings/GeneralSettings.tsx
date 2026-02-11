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

  const { theme, setTheme } = useSettingsStore();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">General</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Configure general application settings.
        </p>
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
