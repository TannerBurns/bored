import { useMemo, useCallback } from 'react';
import { getAgentIcon, getAgentBrandColor } from '../common/AgentIcons';
import { StatusSection, AlertMessages, useAgentSettings } from './shared';
import { getAgentStatus, setAgentSettings as setAgentSettingsBackend } from '../../lib/tauri';
import {
  useSettingsStore,
  WORKFLOW_PRESETS,
  WORKFLOW_STAGE_INFO,
  MODEL_OPTIONS,
  CODEX_MODEL_OPTIONS,
  type AIModel,
  type WorkflowPreset,
  type AgentConfig,
} from '../../stores/settingsStore';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';
import { cn } from '../../lib/utils';
import type { AgentModelOption } from '../../types';

const PRESET_KEYS = Object.keys(WORKFLOW_PRESETS) as Exclude<WorkflowPreset, 'custom'>[];

function fetchAgentStatus(agentId: string) {
  return async () => {
    const status = await getAgentStatus(agentId);
    return {
      isAvailable: status.isAvailable,
      version: status.version ?? undefined,
    };
  };
}

function getModelOptions(agentId: string, availableModels?: AgentModelOption[]): { value: AIModel; label: string }[] {
  if (availableModels && availableModels.length > 0) {
    return availableModels.map((m) => ({ value: m.value as AIModel, label: m.label }));
  }
  if (agentId === 'codex') return CODEX_MODEL_OPTIONS;
  return MODEL_OPTIONS;
}

function ClaudeSpecificSettings({ agentId }: { agentId: string }) {
  const settings = useSettingsStore((s) => s.getAgentSettings(agentId));
  const setAgentSetting = useSettingsStore((s) => s.setAgentSetting);

  const thinkingEnabled = (settings.thinkingEnabled as boolean) ?? true;
  const extendedContext = (settings.extendedContext as boolean) ?? false;
  const chromeEnabled = (settings.chromeEnabled as boolean) ?? false;

  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-board-text">CLI Options</h3>
        <p className="text-xs text-board-text-muted">Agent-specific options saved automatically.</p>
      </div>
      <ToggleRow
        label="Thinking" description="Enable extended thinking for better reasoning."
        enabled={thinkingEnabled}
        onChange={(v) => setAgentSetting(agentId, 'thinkingEnabled', v)}
      />
      <ToggleRow
        label="Extended Context" description="Enable 1M token context window."
        enabled={extendedContext}
        onChange={(v) => setAgentSetting(agentId, 'extendedContext', v)}
      />
      <ToggleRow
        label="Chrome" description="Enable Chrome browser access."
        enabled={chromeEnabled}
        onChange={(v) => setAgentSetting(agentId, 'chromeEnabled', v)}
      />
    </div>
  );
}

function CursorSpecificSettings({ agentId }: { agentId: string }) {
  const settings = useSettingsStore((s) => s.getAgentSettings(agentId));
  const setAgentSetting = useSettingsStore((s) => s.setAgentSetting);
  const thinkingEnabled = (settings.thinkingEnabled as boolean) ?? true;

  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-board-text">CLI Options</h3>
        <p className="text-xs text-board-text-muted">Agent-specific options saved automatically.</p>
      </div>
      <ToggleRow
        label="Thinking" description='Appends "-thinking" to the model name sent to Cursor.'
        enabled={thinkingEnabled}
        onChange={(v) => setAgentSetting(agentId, 'thinkingEnabled', v)}
      />
    </div>
  );
}

function CodexSpecificSettings({ agentId }: { agentId: string }) {
  const settings = useSettingsStore((s) => s.getAgentSettings(agentId));
  const setAgentSetting = useSettingsStore((s) => s.setAgentSetting);

  const ossEnabled = (settings.ossEnabled as boolean) ?? false;
  const localProvider = (settings.localProvider as string) ?? 'ollama';
  const modelOverride = (settings.modelOverride as string) ?? '';

  const updateSetting = useCallback((key: string, value: unknown) => {
    setAgentSetting(agentId, key, value);
    const current = useSettingsStore.getState().getAgentSettings(agentId);
    setAgentSettingsBackend(agentId, { ...current, [key]: value })
      .catch((err) => console.warn('[codex] Failed to sync settings to backend:', err));
  }, [agentId, setAgentSetting]);

  return (
    <div className="glass rounded-lg p-3 space-y-3">
      <div>
        <h3 className="text-sm font-medium text-board-text">Local Models (OSS)</h3>
        <p className="text-xs text-board-text-muted">Run Codex against a local inference server instead of the OpenAI API.</p>
      </div>
      <ToggleRow
        label="Use Local Provider"
        description="Enable open-source mode (--oss) for local model inference."
        enabled={ossEnabled}
        onChange={(v) => updateSetting('ossEnabled', v)}
      />
      {ossEnabled && (
        <>
          <div className="glass-subtle rounded-lg px-3 py-2 space-y-1.5">
            <label className="block text-sm font-medium text-board-text">Provider</label>
            <div className="flex gap-1.5">
              {[
                { value: 'ollama', label: 'Ollama' },
                { value: 'lmstudio', label: 'LM Studio' },
              ].map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => updateSetting('localProvider', opt.value)}
                  className={cn(
                    'px-3 py-1.5 text-xs font-medium rounded-lg transition-all duration-200',
                    localProvider === opt.value
                      ? 'glass-intense ring-1 ring-board-accent text-board-accent'
                      : 'glass text-board-text-muted hover:text-board-text hover:glass-intense'
                  )}
                >
                  {opt.label}
                </button>
              ))}
            </div>
          </div>
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Model</label>
            <input
              type="text"
              placeholder="e.g., llama3.2, codestral, deepseek-coder"
              value={modelOverride}
              onChange={(e) => updateSetting('modelOverride', e.target.value)}
              className="w-full px-2 py-1.5 bg-board-surface-raised rounded-lg border border-board-border focus:border-board-accent focus:outline-none font-mono text-xs text-board-text"
            />
            <p className="text-xs text-board-text-muted mt-1">The model name your local server should use. Overrides stage model selection.</p>
          </div>
        </>
      )}
    </div>
  );
}

const AGENT_SPECIFIC_SECTIONS: Record<string, React.ComponentType<{ agentId: string }>> = {
  claude: ClaudeSpecificSettings,
  cursor: CursorSpecificSettings,
  codex: CodexSpecificSettings,
};

function ToggleRow({ label, description, enabled, onChange, disabled }: {
  label: string;
  description: string;
  enabled: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-center justify-between glass-subtle rounded-lg px-3 py-2">
      <div className="mr-3">
        <span className="text-sm font-medium text-board-text">{label}</span>
        <p className="text-xs text-board-text-muted">{description}</p>
      </div>
      <button
        onClick={() => onChange(!enabled)}
        disabled={disabled}
        className={cn(
          'relative inline-flex h-5 w-9 flex-shrink-0 cursor-pointer rounded-full transition-colors duration-200 ease-in-out focus:outline-none focus:ring-1 focus:ring-board-accent',
          enabled ? 'bg-board-accent' : 'glass',
          disabled && 'opacity-50 cursor-not-allowed'
        )}
      >
        <span className={cn(
          'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow ring-0 transition duration-200 ease-in-out',
          enabled ? 'translate-x-4' : 'translate-x-0.5'
        )} style={{ marginTop: '2px' }} />
      </button>
    </div>
  );
}

function WorkflowSection({ agentId, config, models }: { agentId: string; config: AgentConfig; models: { value: AIModel; label: string }[] }) {
  const setPreset = useSettingsStore((s) => s.setAgentConfigWorkflowPreset);
  const setStage = useSettingsStore((s) => s.setAgentConfigStage);
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-board-text">Workflow</h3>
        <p className="text-xs text-board-text-muted mt-0.5">Configure workflow stages and models.</p>
      </div>

      {/* Preset selector */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h4 className="text-sm font-medium text-board-text">Preset</h4>
          <p className="text-xs text-board-text-muted mt-0.5">Manual changes switch to Custom.</p>
        </div>
        <div className="grid grid-cols-2 gap-1.5">
          {PRESET_KEYS.map((key) => {
            const preset = WORKFLOW_PRESETS[key];
            const isSelected = config.workflowPreset === key;
            return (
              <button
                key={key}
                onClick={() => setPreset(agentId, key)}
                className={cn(
                  'flex flex-col items-start gap-0.5 px-2.5 py-2 rounded-lg transition-all duration-200 text-left',
                  isSelected ? 'glass-intense ring-1 ring-board-accent' : 'glass hover:glass-intense'
                )}
              >
                <span className={cn('text-xs font-medium', isSelected ? 'text-board-accent' : 'text-board-text')}>
                  {preset.label}
                </span>
                <span className="text-[11px] text-board-text-muted leading-snug">{preset.description}</span>
              </button>
            );
          })}
          {config.workflowPreset === 'custom' && (
            <div className="flex flex-col items-start gap-0.5 px-2.5 py-2 rounded-lg glass-intense ring-1 ring-board-accent text-left">
              <span className="text-xs font-medium text-board-accent">Custom</span>
              <span className="text-[11px] text-board-text-muted leading-snug">Manually configured stages and models</span>
            </div>
          )}
        </div>
      </div>

      {/* Per-stage table */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h4 className="text-sm font-medium text-board-text">Stage Configuration</h4>
          <p className="text-xs text-board-text-muted mt-0.5">Toggle stages and choose models. Required stages cannot be disabled.</p>
        </div>
        <div className="space-y-1">
          <div className="grid grid-cols-[40px_1fr_130px] gap-2 px-2 py-1">
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">On</span>
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">Stage</span>
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">Model</span>
          </div>
          {WORKFLOW_STAGE_INFO.map((stage) => {
            const stageConfig = config.workflowStages[stage.key];
            return (
              <div key={stage.key} className={cn(
                'grid grid-cols-[40px_1fr_130px] gap-2 items-center px-2 py-1.5 rounded-lg transition-all duration-150',
                stageConfig.enabled ? 'glass-subtle' : 'opacity-50'
              )}>
                <button
                  onClick={() => { if (!stage.required) setStage(agentId, stage.key, { enabled: !stageConfig.enabled }); }}
                  disabled={stage.required}
                  className={cn(
                    'relative inline-flex h-5 w-9 flex-shrink-0 rounded-full transition-colors duration-200',
                    stage.required ? 'cursor-not-allowed' : 'cursor-pointer',
                    stageConfig.enabled ? 'bg-board-accent' : 'glass'
                  )}
                  title={stage.required ? 'Required stage' : `${stageConfig.enabled ? 'Disable' : 'Enable'} ${stage.label}`}
                >
                  <span className={cn(
                    'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow transition duration-200',
                    stageConfig.enabled ? 'translate-x-4' : 'translate-x-0.5'
                  )} style={{ marginTop: '2px' }} />
                </button>
                <div className="min-w-0">
                  <span className="text-sm font-medium text-board-text">{stage.label}</span>
                  {stage.required && (
                    <span className="ml-1.5 text-[9px] font-medium px-1 py-0 rounded-full bg-board-accent/15 text-board-accent leading-relaxed">required</span>
                  )}
                  <p className="text-[11px] text-board-text-muted truncate">{stage.description}</p>
                </div>
                <select
                  value={stageConfig.model}
                  onChange={(e) => setStage(agentId, stage.key, { model: e.target.value as AIModel })}
                  disabled={!stageConfig.enabled}
                  className="w-full px-2 py-1 text-xs glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                >
                  {models.map((opt) => (
                    <option key={opt.value} value={opt.value}>{opt.label}</option>
                  ))}
                </select>
              </div>
            );
          })}
        </div>
      </div>

      {/* Timeouts / retries */}
      <div className="glass rounded-lg p-3 space-y-3">
        <div className="grid grid-cols-3 gap-2">
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Stage Timeout (hrs)</label>
            <input type="number" min={1} step={1} value={config.stageTimeoutHours}
              onChange={(e) => updateConfig(agentId, { stageTimeoutHours: parseInt(e.target.value) || 1 })}
              className="w-20 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
          </div>
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Max Retries</label>
            <input type="number" min={0} max={5} value={config.stageMaxRetries}
              onChange={(e) => updateConfig(agentId, { stageMaxRetries: parseInt(e.target.value) || 2 })}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
          </div>
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Review Iterations</label>
            <input type="number" min={0} max={10} value={config.codeReviewMaxIterations}
              onChange={(e) => updateConfig(agentId, { codeReviewMaxIterations: parseInt(e.target.value) || 3 })}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
          </div>
        </div>
      </div>
    </div>
  );
}

function SpecAgentSection({ agentId, config, models }: { agentId: string; config: AgentConfig; models: { value: AIModel; label: string }[] }) {
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-board-text">Spec Agent</h3>
        <p className="text-xs text-board-text-muted mt-0.5">Configure how the AI spec agent generates plans.</p>
      </div>
      <div className="glass rounded-lg p-3 space-y-3">
        <ToggleRow
          label="Auto-approve Plans"
          description="Automatically approve generated plans without manual review"
          enabled={config.plannerAutoApprove}
          onChange={(v) => updateConfig(agentId, { plannerAutoApprove: v })}
        />
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">Max Exploration Queries</label>
          <input type="number" min={1} max={50} value={config.plannerMaxExplorations}
            onChange={(e) => updateConfig(agentId, { plannerMaxExplorations: parseInt(e.target.value) || 10 })}
            className="w-20 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
          <p className="text-xs text-board-text-muted mt-0.5">Maximum exploration queries before generating a plan (1-50)</p>
        </div>
        <div className="grid grid-cols-2 gap-2">
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Timeout (min)</label>
            <input type="number" min={1} max={30} value={config.plannerTimeoutMinutes}
              onChange={(e) => updateConfig(agentId, { plannerTimeoutMinutes: parseInt(e.target.value) || 10 })}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
          </div>
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Max Retries</label>
            <input type="number" min={0} max={5} value={config.plannerMaxRetries}
              onChange={(e) => updateConfig(agentId, { plannerMaxRetries: parseInt(e.target.value) || 2 })}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
          </div>
        </div>
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">Model</label>
          <select value={config.plannerModel}
            onChange={(e) => updateConfig(agentId, { plannerModel: e.target.value as AIModel })}
            className="w-full max-w-[180px] px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent">
            {models.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
          </select>
        </div>
      </div>
    </div>
  );
}

function ValidationSection({ agentId, config, models }: { agentId: string; config: AgentConfig; models: { value: AIModel; label: string }[] }) {
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-board-text">Validation Agent</h3>
        <p className="text-xs text-board-text-muted mt-0.5">AI agent for ticket validation chat.</p>
      </div>
      <div className="glass rounded-lg p-3 space-y-3">
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">Model</label>
          <select value={config.validationModel}
            onChange={(e) => updateConfig(agentId, { validationModel: e.target.value as AIModel })}
            className="w-full max-w-[180px] px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent">
            {models.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
          </select>
        </div>
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">Timeout (minutes)</label>
          <input type="number" min={1} max={120} value={config.validationTimeoutMinutes}
            onChange={(e) => updateConfig(agentId, { validationTimeoutMinutes: Math.max(1, Math.min(120, parseInt(e.target.value) || 10)) })}
            className="w-20 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
        </div>
      </div>
    </div>
  );
}

function DiagnosticSection({ agentId, config, models }: { agentId: string; config: AgentConfig; models: { value: AIModel; label: string }[] }) {
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-board-text">Diagnostic Agent</h3>
        <p className="text-xs text-board-text-muted mt-0.5">AI agent for diagnosing worktree and git failures.</p>
      </div>
      <div className="glass rounded-lg p-3 space-y-3">
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">Model</label>
          <select value={config.diagnosticModel}
            onChange={(e) => updateConfig(agentId, { diagnosticModel: e.target.value as AIModel })}
            className="w-full max-w-[180px] px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent">
            {models.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
          </select>
        </div>
      </div>
    </div>
  );
}

interface AgentSettingsPageProps {
  agentId: string;
}

export function AgentSettingsPage({ agentId }: AgentSettingsPageProps) {
  const agentConfig = useMemo(() => ({
    agentType: agentId,
    getStatus: fetchAgentStatus(agentId),
  }), [agentId]);
  const base = useAgentSettings(agentConfig);
  const agent = useAgentRegistryStore((s) => s.agents.find((a) => a.id === agentId));
  const config = useSettingsStore((s) => s.agentConfigs[agentId] ?? s.getAgentConfig(agentId));

  const Icon = getAgentIcon(agentId);
  const brandColor = getAgentBrandColor(agentId, agent?.brandColor);
  const displayName = agent?.displayName ?? agentId.charAt(0).toUpperCase() + agentId.slice(1);
  const models = getModelOptions(agentId, agent?.availableModels);

  const AgentSpecific = AGENT_SPECIFIC_SECTIONS[agentId];

  if (base.loading) {
    return <div className="text-board-text-muted text-center py-8">Loading {displayName} status...</div>;
  }

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-semibold text-board-text flex items-center gap-2">
        <Icon size={20} style={brandColor ? { color: brandColor } : undefined} />
        {displayName}
      </h2>

      <AlertMessages error={base.error} success={base.success} />

      <StatusSection
        isAvailable={base.status?.isAvailable ?? false}
        version={base.status?.version}
      />

      {AgentSpecific && (
        <>
          <AgentSpecific agentId={agentId} />
          <hr className="border-board-border/30" />
        </>
      )}

      <WorkflowSection agentId={agentId} config={config} models={models} />
      <hr className="border-board-border/30" />
      <SpecAgentSection agentId={agentId} config={config} models={models} />
      <hr className="border-board-border/30" />
      <ValidationSection agentId={agentId} config={config} models={models} />
      <hr className="border-board-border/30" />
      <DiagnosticSection agentId={agentId} config={config} models={models} />
    </div>
  );
}
