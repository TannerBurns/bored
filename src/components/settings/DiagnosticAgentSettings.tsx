import {
  useSettingsStore,
  CLAUDE_MODEL_OPTIONS,
  type AIModel,
} from '../../stores/settingsStore';

export function DiagnosticAgentSettings() {
  const config = useSettingsStore((s) => s.agentConfigs['claude']);
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">Diagnostic Agent</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Configure the AI agent used for diagnosing worktree and git failures.
        </p>
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">
            Model
          </label>
          <select
            value={config.diagnosticModel}
            onChange={(e) => updateConfig('claude', { diagnosticModel: e.target.value as AIModel })}
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
              value={config.diagnosticTimeoutMinutes}
              onChange={(e) =>
                updateConfig('claude', { diagnosticTimeoutMinutes: Math.max(1, Math.min(120, parseInt(e.target.value) || 10)) })
              }
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
              value={config.diagnosticMaxRetries}
              onChange={(e) =>
                updateConfig('claude', { diagnosticMaxRetries: Math.max(0, Math.min(5, parseInt(e.target.value) || 0)) })
              }
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
            />
          </div>
        </div>
      </div>
    </div>
  );
}
