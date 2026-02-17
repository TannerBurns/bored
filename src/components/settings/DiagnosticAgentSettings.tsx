import {
  useSettingsStore,
  MODEL_OPTIONS,
  type AIModel,
} from '../../stores/settingsStore';

export function DiagnosticAgentSettings() {
  const {
    diagnosticModel,
    setDiagnosticModel,
  } = useSettingsStore();

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
            value={diagnosticModel}
            onChange={(e) => setDiagnosticModel(e.target.value as AIModel)}
            className="w-full max-w-[180px] px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
          >
            {MODEL_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
          <p className="text-xs text-board-text-muted mt-0.5">
            AI model used for diagnosing errors when tickets get blocked
          </p>
        </div>
      </div>
    </div>
  );
}
