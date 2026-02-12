import {
  useSettingsStore,
  type AIModel,
} from '../../stores/settingsStore';

const MODEL_OPTIONS: { value: AIModel; label: string }[] = [
  { value: 'opus-4.6', label: 'Opus 4.6' },
  { value: 'opus-4.5', label: 'Opus 4.5' },
  { value: 'sonnet-4.5', label: 'Sonnet 4.5' },
];

export function ValidationAgentSettings() {
  const {
    validationModel,
    setValidationModel,
    validationTimeoutMinutes,
    setValidationTimeoutMinutes,
  } = useSettingsStore();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">Validation Agent</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Configure the AI agent used for ticket validation chat (review diff, run app, test).
        </p>
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">
            Model
          </label>
          <select
            value={validationModel}
            onChange={(e) => setValidationModel(e.target.value as AIModel)}
            className="w-full max-w-[180px] px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
          >
            {MODEL_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
          <p className="text-xs text-board-text-muted mt-0.5">
            AI model used for validation chat
          </p>
        </div>

        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">
            Timeout (minutes)
          </label>
          <input
            type="number"
            min={1}
            max={120}
            value={validationTimeoutMinutes}
            onChange={(e) =>
              setValidationTimeoutMinutes(Math.max(1, Math.min(120, parseInt(e.target.value) || 10)))
            }
            className="w-20 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
          />
          <p className="text-xs text-board-text-muted mt-0.5">
            Maximum time for the validation agent per request (1–120)
          </p>
        </div>
      </div>
    </div>
  );
}
