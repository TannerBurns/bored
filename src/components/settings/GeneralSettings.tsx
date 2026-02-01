import { useSettingsStore } from '../../stores/settingsStore';

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
    codeReviewMaxIterations,
    setCodeReviewMaxIterations,
  } = useSettingsStore();

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-xl font-semibold text-board-text">General</h2>
        <p className="text-sm text-board-text-muted mt-1">
          Configure general application settings.
        </p>
      </div>

      {/* Default Agent Preference Section */}
      <div className="bg-board-surface rounded-xl p-5 space-y-4 border border-board-border">
        <div>
          <h3 className="font-medium text-board-text">Default Agent Preference</h3>
          <p className="text-sm text-board-text-muted mt-0.5">
            Set the default agent preference for new tickets.
          </p>
        </div>

        <div className="grid grid-cols-3 gap-3">
          {agentOptions.map((option) => {
            const isSelected = defaultAgentPref === option.value;
            return (
              <button
                key={option.value}
                onClick={() => setDefaultAgentPref(option.value)}
                className={`group relative flex flex-col items-center gap-3 p-5 rounded-xl border-2 transition-all duration-200 ${
                  isSelected
                    ? 'border-board-accent bg-board-accent-subtle shadow-sm'
                    : 'border-board-border hover:border-board-text-muted bg-board-surface-raised hover:bg-board-card-hover'
                }`}
              >
                <div className="text-center">
                  <span className={`block text-sm font-medium ${
                    isSelected ? 'text-board-accent' : 'text-board-text'
                  }`}>
                    {option.label}
                  </span>
                  <span className="block text-xs text-board-text-muted mt-0.5">
                    {option.description}
                  </span>
                </div>
                {isSelected && (
                  <div className="absolute top-2 right-2">
                    <svg className="w-5 h-5 text-board-accent" viewBox="0 0 24 24" fill="currentColor">
                      <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z" />
                    </svg>
                  </div>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {/* Planner Settings Section */}
      <div className="bg-board-surface rounded-xl p-5 space-y-4 border border-board-border">
        <div>
          <h3 className="font-medium text-board-text">Planner Agent Settings</h3>
          <p className="text-sm text-board-text-muted mt-0.5">
            Configure how the AI planner explores codebases and generates work plans.
          </p>
        </div>

        {/* Auto-approve toggle */}
        <div className="flex items-center justify-between">
          <div>
            <span className="text-sm font-medium text-board-text">Auto-approve Plans</span>
            <p className="text-xs text-board-text-muted mt-0.5">
              Automatically approve generated plans without manual review
            </p>
          </div>
          <button
            onClick={() => setPlannerAutoApprove(!plannerAutoApprove)}
            className={`relative inline-flex h-6 w-11 flex-shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none focus:ring-2 focus:ring-board-accent focus:ring-offset-2 ${
              plannerAutoApprove ? 'bg-board-accent' : 'bg-gray-300 dark:bg-gray-600'
            }`}
          >
            <span
              className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out ${
                plannerAutoApprove ? 'translate-x-5' : 'translate-x-0'
              }`}
            />
          </button>
        </div>

        {/* Max explorations */}
        <div>
          <label className="block text-sm font-medium text-board-text mb-2">
            Max Exploration Queries
          </label>
          <input
            type="number"
            min={1}
            max={50}
            value={plannerMaxExplorations}
            onChange={(e) => setPlannerMaxExplorations(parseInt(e.target.value) || 10)}
            className="w-24 px-3 py-1.5 text-sm border border-board-border rounded-lg bg-board-surface-raised text-board-text focus:ring-2 focus:ring-board-accent focus:border-board-accent"
          />
          <p className="text-xs text-board-text-muted mt-1">
            Maximum number of codebase exploration queries before generating a plan (1-50)
          </p>
        </div>

        {/* Code Review Max Iterations */}
        <div>
          <label className="block text-sm font-medium text-board-text mb-2">
            Code Review Max Iterations
          </label>
          <input
            type="number"
            min={1}
            max={10}
            value={codeReviewMaxIterations}
            onChange={(e) => setCodeReviewMaxIterations(parseInt(e.target.value) || 3)}
            className="w-24 px-3 py-1.5 text-sm border border-board-border rounded-lg bg-board-surface-raised text-board-text focus:ring-2 focus:ring-board-accent focus:border-board-accent"
          />
          <p className="text-xs text-board-text-muted mt-1">
            Maximum code review iterations before proceeding (1-10)
          </p>
        </div>

        {/* Planner model preference */}
        <div>
          <label className="block text-sm font-medium text-board-text mb-2">
            Planner Model
          </label>
          <div className="grid grid-cols-3 gap-3">
            {plannerModelOptions.map((option) => {
              const isSelected = plannerModel === option.value;
              return (
                <button
                  key={option.value}
                  onClick={() => setPlannerModel(option.value)}
                  className={`flex flex-col items-center gap-1 p-3 rounded-lg border-2 transition-all ${
                    isSelected
                      ? 'border-board-accent bg-board-accent-subtle'
                      : 'border-board-border hover:border-board-text-muted bg-board-surface-raised hover:bg-board-card-hover'
                  }`}
                >
                  <span className={`text-sm font-medium ${
                    isSelected ? 'text-board-accent' : 'text-board-text'
                  }`}>
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

      {/* Theme Section */}
      <div className="bg-board-surface rounded-xl p-5 space-y-4 border border-board-border">
        <div>
          <h3 className="font-medium text-board-text">Theme</h3>
          <p className="text-sm text-board-text-muted mt-0.5">
            Select your preferred color scheme.
          </p>
        </div>

        <div className="grid grid-cols-3 gap-3">
          {themeOptions.map((option) => {
            const isSelected = theme === option.value;
            return (
              <button
                key={option.value}
                onClick={() => setTheme(option.value)}
                className={`group relative flex flex-col items-center gap-3 p-5 rounded-xl border-2 transition-all duration-200 ${
                  isSelected
                    ? 'border-board-accent bg-board-accent-subtle shadow-sm'
                    : 'border-board-border hover:border-board-text-muted bg-board-surface-raised hover:bg-board-card-hover'
                }`}
              >
                <div className={`p-3 rounded-full transition-colors ${
                  isSelected 
                    ? 'bg-board-accent text-white' 
                    : 'bg-board-surface text-board-text-secondary group-hover:text-board-text'
                }`}>
                  <option.Icon className="w-6 h-6" />
                </div>
                <div className="text-center">
                  <span className={`block text-sm font-medium ${
                    isSelected ? 'text-board-accent' : 'text-board-text'
                  }`}>
                    {option.label}
                  </span>
                  <span className="block text-xs text-board-text-muted mt-0.5">
                    {option.description}
                  </span>
                </div>
                {isSelected && (
                  <div className="absolute top-2 right-2">
                    <svg className="w-5 h-5 text-board-accent" viewBox="0 0 24 24" fill="currentColor">
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
