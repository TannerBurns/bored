import {
  useSettingsStore,
  MODEL_OPTIONS,
  WORKFLOW_PRESETS,
  WORKFLOW_STAGE_INFO,
  type AIModel,
  type WorkflowPreset,
} from '../../stores/settingsStore';
import { cn } from '../../lib/utils';

const PRESET_KEYS = Object.keys(WORKFLOW_PRESETS) as Exclude<WorkflowPreset, 'custom'>[];

export function AgentWorkflowSettings() {
  const {
    workflowPreset,
    workflowStages,
    setWorkflowPreset,
    setWorkflowStageConfig,
    codeReviewMaxIterations,
    setCodeReviewMaxIterations,
    stageTimeoutHours,
    setStageTimeoutHours,
    stageMaxRetries,
    setStageMaxRetries,
  } = useSettingsStore();

  return (
    <div className="space-y-4">
      <div>
        <h2 className="text-lg font-semibold text-board-text">Agent Workflow</h2>
        <p className="text-xs text-board-text-muted mt-0.5">
          Configure which workflow stages run and which AI model to use for each stage.
        </p>
      </div>

      {/* Preset Selector */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Workflow Preset</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Choose a preset to quickly configure stages and models. Manual changes switch to Custom.
          </p>
        </div>

        <div className="grid grid-cols-2 gap-1.5">
          {PRESET_KEYS.map((key) => {
            const preset = WORKFLOW_PRESETS[key];
            const isSelected = workflowPreset === key;
            return (
              <button
                key={key}
                onClick={() => setWorkflowPreset(key)}
                className={cn(
                  'flex flex-col items-start gap-0.5 px-2.5 py-2 rounded-lg transition-all duration-200 text-left',
                  isSelected
                    ? 'glass-intense ring-1 ring-board-accent'
                    : 'glass hover:glass-intense'
                )}
              >
                <span className={cn(
                  'text-xs font-medium',
                  isSelected ? 'text-board-accent' : 'text-board-text'
                )}>
                  {preset.label}
                </span>
                <span className="text-[11px] text-board-text-muted leading-snug">
                  {preset.description}
                </span>
              </button>
            );
          })}

          {/* Custom indicator */}
          {workflowPreset === 'custom' && (
            <div className="flex flex-col items-start gap-0.5 px-2.5 py-2 rounded-lg glass-intense ring-1 ring-board-accent text-left">
              <span className="text-xs font-medium text-board-accent">Custom</span>
              <span className="text-[11px] text-board-text-muted leading-snug">
                Manually configured stages and models
              </span>
            </div>
          )}
        </div>
      </div>

      {/* Per-Stage Configuration Table */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h3 className="text-sm font-medium text-board-text">Stage Configuration</h3>
          <p className="text-xs text-board-text-muted mt-0.5">
            Toggle stages on/off and choose the AI model for each. Required stages cannot be disabled.
          </p>
        </div>

        <div className="space-y-1">
          {/* Header */}
          <div className="grid grid-cols-[40px_1fr_130px] gap-2 px-2 py-1">
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">On</span>
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">Stage</span>
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">Model</span>
          </div>

          {WORKFLOW_STAGE_INFO.map((stage) => {
            const config = workflowStages[stage.key];
            const isEnabled = config.enabled;
            const isRequired = stage.required;

            return (
              <div
                key={stage.key}
                className={cn(
                  'grid grid-cols-[40px_1fr_130px] gap-2 items-center px-2 py-1.5 rounded-lg transition-all duration-150',
                  isEnabled ? 'glass-subtle' : 'opacity-50'
                )}
              >
                {/* Toggle */}
                <button
                  onClick={() => {
                    if (!isRequired) {
                      setWorkflowStageConfig(stage.key, { enabled: !isEnabled });
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

                {/* Stage name + description */}
                <div className="min-w-0">
                  <span className="text-sm font-medium text-board-text">{stage.label}</span>
                  {isRequired && (
                    <span className="ml-1.5 text-[9px] font-medium px-1 py-0 rounded-full bg-board-accent/15 text-board-accent leading-relaxed">
                      required
                    </span>
                  )}
                  <p className="text-[11px] text-board-text-muted truncate">{stage.description}</p>
                </div>

                {/* Model dropdown */}
                <select
                  value={config.model}
                  onChange={(e) => setWorkflowStageConfig(stage.key, { model: e.target.value as AIModel })}
                  disabled={!isEnabled}
                  className="w-full px-2 py-1 text-xs glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  {MODEL_OPTIONS.map((opt) => (
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

      {/* Workflow Stage Settings */}
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
              Stage Timeout (hours)
            </label>
            <input
              type="number"
              min={1}
              step={1}
              value={stageTimeoutHours}
              onChange={(e) => setStageTimeoutHours(parseInt(e.target.value) || 1)}
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
    </div>
  );
}
