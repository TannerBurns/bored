import { useSettingsStore } from '../../stores/settingsStore';
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

const agentOptions = [
  { value: 'any', label: 'Any', description: 'No preference - use any available agent' },
  { value: 'cursor', label: 'Cursor', description: 'Prefer Cursor agent' },
  { value: 'claude', label: 'Claude', description: 'Prefer Claude Code agent' },
] as const;

const plannerModelOptions = [
  { value: 'default', label: 'Default', description: 'Use default model' },
  { value: 'opus', label: 'Opus', description: 'Most capable, higher cost' },
  { value: 'sonnet', label: 'Sonnet', description: 'Balanced capability and speed' },
] as const;

export function GeneralSettings() {
  const { 
    theme, 
    setTheme, 
    defaultAgentPref, 
    setDefaultAgentPref,
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

      {/* Default Agent Preference Section */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Default Agent Preference</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Set the default agent preference for new tickets.
          </p>
        </div>

        <div className="grid grid-cols-3 gap-2">
          {agentOptions.map((option) => {
            const isSelected = defaultAgentPref === option.value;
            return (
              <button
                key={option.value}
                onClick={() => setDefaultAgentPref(option.value)}
                className={cn(
                  'group relative flex flex-col items-center gap-1 px-3 py-2 rounded-lg transition-all duration-200',
                  isSelected
                    ? 'glass-intense ring-1 ring-board-accent'
                    : 'glass hover:glass-intense'
                )}
              >
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
              max={10}
              value={plannerTimeoutMinutes}
              onChange={(e) => setPlannerTimeoutMinutes(parseInt(e.target.value) || 5)}
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
          <div className="grid grid-cols-3 gap-2">
            {plannerModelOptions.map((option) => {
              const isSelected = plannerModel === option.value;
              return (
                <button
                  key={option.value}
                  onClick={() => setPlannerModel(option.value)}
                  className={cn(
                    'flex flex-col items-center gap-0.5 px-2 py-1.5 rounded-lg transition-all duration-200',
                    isSelected
                      ? 'glass-intense ring-1 ring-board-accent'
                      : 'glass hover:glass-intense'
                  )}
                >
                  <span className={cn(
                    'text-sm font-medium',
                    isSelected ? 'text-board-accent' : 'text-board-text'
                  )}>
                    {option.label}
                  </span>
                  <span className="text-xs text-board-text-muted">
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
    </div>
  );
}
