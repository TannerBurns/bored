import { useSettingsStore } from '../../stores/settingsStore';
import { cn } from '../../lib/utils';

const plannerModelOptions = [
  { value: 'opus-4.5', label: 'Opus 4.5', description: 'Most capable', isDefault: true },
  { value: 'opus-4.6', label: 'Opus 4.6', description: 'Latest generation', isDefault: false },
  { value: 'sonnet-4.5', label: 'Sonnet 4.5', description: 'Fast and capable', isDefault: false },
] as const;

export function SpecAgentSettings() {
  const {
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
  } = useSettingsStore();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">Spec Agent</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Configure how the AI spec agent explores codebases and generates work plans.
        </p>
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
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

        {/* Timeout and retries */}
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

        {/* Model preference */}
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
    </div>
  );
}
