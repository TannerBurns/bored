import {
  useSettingsStore,
  CLAUDE_MODEL_OPTIONS,
  type AIModel,
} from '../../stores/settingsStore';
import { cn } from '../../lib/utils';

export function SpecAgentSettings() {
  const config = useSettingsStore((s) => s.agentConfigs['claude']);
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);

  const plannerAutoApprove = config.plannerAutoApprove;
  const plannerModel = config.plannerModel;
  const plannerTimeoutMinutes = config.plannerTimeoutMinutes;
  const plannerMaxRetries = config.plannerMaxRetries;

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">Spec Agent</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Configure how the AI spec agent explores codebases and generates work plans.
        </p>
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div className="flex items-center justify-between glass-subtle rounded-lg px-3 py-2">
          <div>
            <span className="text-sm font-medium text-board-text">Auto-approve Plans</span>
            <p className="text-xs text-board-text-muted">
              Automatically approve generated plans without manual review
            </p>
          </div>
          <button
            onClick={() => updateConfig('claude', { plannerAutoApprove: !plannerAutoApprove })}
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

        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">
            Model
          </label>
          <select
            value={plannerModel}
            onChange={(e) => updateConfig('claude', { plannerModel: e.target.value as AIModel })}
            className="w-full max-w-[180px] px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
          >
            {CLAUDE_MODEL_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">
              Timeout (min)
            </label>
            <input
              type="number"
              min={1}
              max={120}
              value={plannerTimeoutMinutes}
              onChange={(e) => updateConfig('claude', { plannerTimeoutMinutes: Math.max(1, Math.min(120, parseInt(e.target.value) || 10)) })}
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
              onChange={(e) => updateConfig('claude', { plannerMaxRetries: Math.max(0, Math.min(5, parseInt(e.target.value) || 0)) })}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
            />
          </div>
        </div>
      </div>
    </div>
  );
}
