import {
  useSettingsStore,
  CLAUDE_MODEL_OPTIONS,
  WORKFLOW_STAGE_INFO,
  REQUIRED_STAGE_KEYS,
  type AIModel,
} from '../../stores/settingsStore';
import { cn } from '../../lib/utils';

export function AgentWorkflowSettings() {
  const config = useSettingsStore((s) => s.agentConfigs['claude']);
  const catalog = useSettingsStore((s) => s.commandsCatalog);
  const setStage = useSettingsStore((s) => s.setAgentConfigStage);
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);

  const workflowStages = config.workflowStages;
  const codeReviewMaxIterations = config.codeReviewMaxIterations;
  const stageTimeoutHours = config.stageTimeoutHours;
  const stageMaxRetries = config.stageMaxRetries;

  const allStageInfo = [
    ...WORKFLOW_STAGE_INFO,
    ...catalog
      .filter((c) => c.enabled)
      .map((c) => ({
        key: c.id,
        label: c.name,
        description: c.description,
        required: false,
      })),
  ];

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">Agent Workflow</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Configure which workflow stages run and which AI model to use for each stage.
        </p>
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Stage Configuration</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Toggle stages on/off and choose the AI model for each. Required stages cannot be disabled.
          </p>
        </div>

        <div className="space-y-1">
          <div className="grid grid-cols-[40px_1fr_130px] gap-2 px-2 py-1">
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">On</span>
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">Stage</span>
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">Model</span>
          </div>

          {config.stageOrder.map((key) => {
            const stage = allStageInfo.find((s) => s.key === key);
            if (!stage) return null;
            const stageConfig = workflowStages[key];
            if (!stageConfig) return null;
            const isEnabled = stageConfig.enabled;
            const isRequired = REQUIRED_STAGE_KEYS.has(key);

            return (
              <div
                key={key}
                className={cn(
                  'grid grid-cols-[40px_1fr_130px] gap-2 items-center px-2 py-1.5 rounded-lg transition-all duration-150',
                  isEnabled ? 'glass-subtle' : 'opacity-50'
                )}
              >
                <button
                  onClick={() => {
                    if (!isRequired) {
                      setStage('claude', key, { enabled: !isEnabled });
                    }
                  }}
                  disabled={isRequired}
                  className={cn(
                    'relative inline-flex h-5 w-9 flex-shrink-0 rounded-full transition-colors duration-200 ease-in-out focus:outline-none focus:ring-1 focus:ring-board-accent',
                    isRequired ? 'cursor-not-allowed' : 'cursor-pointer',
                    isEnabled ? 'bg-board-accent' : 'glass'
                  )}
                  title={isRequired ? 'Required stage — cannot be disabled' : `${isEnabled ? 'Disable' : 'Enable'} ${stage.label}`}
                >
                  <span
                    className={cn(
                      'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
                      isEnabled ? 'translate-x-4' : 'translate-x-0.5'
                    )}
                    style={{ marginTop: '2px' }}
                  />
                </button>

                <div className="min-w-0">
                  <span className="text-sm font-medium text-board-text">{stage.label}</span>
                  {isRequired && (
                    <span className="ml-1.5 text-[9px] font-medium px-1 py-0 rounded-full bg-board-accent/15 text-board-accent leading-relaxed">
                      required
                    </span>
                  )}
                  <p className="text-[11px] text-board-text-muted truncate">{stage.description}</p>
                </div>

                <select
                  value={stageConfig.model}
                  onChange={(e) => setStage('claude', key, { model: e.target.value as AIModel })}
                  disabled={!isEnabled}
                  className="w-full px-2 py-1 text-xs glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  {CLAUDE_MODEL_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>
            );
          })}
        </div>
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Workflow Stage Settings</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Configure timeouts and retries for multi-stage workflow execution.
          </p>
        </div>

        <div className="grid grid-cols-2 gap-2">
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">
              Stage Timeout (hours)
            </label>
            <input
              type="number"
              min={1}
              step={1}
              value={stageTimeoutHours}
              onChange={(e) => updateConfig('claude', { stageTimeoutHours: parseInt(e.target.value) || 1 })}
              className="w-20 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
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
              onChange={(e) => updateConfig('claude', { stageMaxRetries: parseInt(e.target.value) || 2 })}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
            />
          </div>
        </div>

        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">
            Code Review Max Iterations
          </label>
          <input
            type="number"
            min={0}
            max={10}
            value={codeReviewMaxIterations}
            onChange={(e) => updateConfig('claude', { codeReviewMaxIterations: parseInt(e.target.value) || 3 })}
            className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all"
          />
          <p className="text-xs text-board-text-muted mt-0.5">
            Max iterations before proceeding (0 to disable)
          </p>
        </div>
      </div>
    </div>
  );
}
