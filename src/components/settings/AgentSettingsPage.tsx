import { useCallback, useMemo } from 'react';
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core';
import { SortableContext, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { restrictToVerticalAxis, restrictToParentElement } from '@dnd-kit/modifiers';
import { getAgentIcon, getAgentBrandColor } from '../common/AgentIcons';
import { StatusSection, AlertMessages, useAgentSettings } from './shared';
import { ToggleRow, AGENT_SPECIFIC_SECTIONS } from './AgentSpecificSettings';
import { getAgentStatus } from '../../lib/tauri';
import {
  useSettingsStore,
  WORKFLOW_STAGE_INFO,
  REQUIRED_STAGE_KEYS,
  MODEL_OPTIONS,
  CODEX_MODEL_OPTIONS,
  type AIModel,
  type AgentConfig,
  type CatalogCommand,
} from '../../stores/settingsStore';
import { useAgentRegistryStore } from '../../stores/agentRegistryStore';
import { cn } from '../../lib/utils';
import type { AgentModelOption } from '../../types';

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

const REQUIRED_STAGE_INFO = new Map(WORKFLOW_STAGE_INFO.map((s) => [s.key, s]));

function GripIcon({ className }: { className?: string }) {
  return (
    <svg className={className} width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
      <circle cx="4" cy="2" r="1" /><circle cx="8" cy="2" r="1" />
      <circle cx="4" cy="6" r="1" /><circle cx="8" cy="6" r="1" />
      <circle cx="4" cy="10" r="1" /><circle cx="8" cy="10" r="1" />
    </svg>
  );
}

function SortableStageRow({
  stageKey, agentId, config, models, catalogInfo,
}: {
  stageKey: string;
  agentId: string;
  config: AgentConfig;
  models: { value: AIModel; label: string }[];
  catalogInfo?: CatalogCommand;
}) {
  const setStage = useSettingsStore((s) => s.setAgentConfigStage);
  const isRequired = REQUIRED_STAGE_KEYS.has(stageKey);
  const requiredInfo = REQUIRED_STAGE_INFO.get(stageKey);

  const label = requiredInfo?.label ?? catalogInfo?.name ?? stageKey;
  const description = requiredInfo?.description ?? catalogInfo?.description ?? '';
  const stageConfig = config.workflowStages[stageKey];

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: stageKey, disabled: isRequired });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  if (!stageConfig) return null;

  return (
    <div
      ref={setNodeRef}
      style={style}
      className={cn(
        'grid grid-cols-[20px_40px_1fr_130px] gap-2 items-center px-2 py-1.5 rounded-lg transition-all duration-150',
        stageConfig.enabled ? 'glass-subtle' : 'opacity-50',
        isDragging && 'opacity-70 ring-1 ring-board-accent z-10',
      )}
    >
      {!isRequired ? (
        <button
          {...attributes}
          {...listeners}
          className="flex items-center justify-center cursor-grab active:cursor-grabbing text-board-text-muted hover:text-board-text transition-colors"
          title="Drag to reorder"
          tabIndex={-1}
        >
          <GripIcon />
        </button>
      ) : (
        <div />
      )}
      <button
        onClick={() => { if (!isRequired) setStage(agentId, stageKey, { enabled: !stageConfig.enabled }); }}
        disabled={isRequired}
        className={cn(
          'relative inline-flex h-5 w-9 flex-shrink-0 rounded-full transition-colors duration-200',
          isRequired ? 'cursor-not-allowed' : 'cursor-pointer',
          stageConfig.enabled ? 'bg-board-accent' : 'glass'
        )}
        title={isRequired ? 'Required stage' : `${stageConfig.enabled ? 'Disable' : 'Enable'} ${label}`}
      >
        <span className={cn(
          'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow transition duration-200',
          stageConfig.enabled ? 'translate-x-4' : 'translate-x-0.5'
        )} style={{ marginTop: '2px' }} />
      </button>
      <div className="min-w-0">
        <span className="text-sm font-medium text-board-text">{label}</span>
        {isRequired && (
          <span className="ml-1.5 text-[9px] font-medium px-1 py-0 rounded-full bg-board-accent/15 text-board-accent leading-relaxed">required</span>
        )}
        {catalogInfo && stageKey === 'code-review' && (
          <span className="ml-1.5 text-[9px] font-medium px-1 py-0 rounded-full bg-purple-500/15 text-purple-400 leading-relaxed">composite</span>
        )}
        <p className="text-[11px] text-board-text-muted truncate">{description}</p>
      </div>
      <select
        value={stageConfig.model}
        onChange={(e) => setStage(agentId, stageKey, { model: e.target.value as AIModel })}
        disabled={!stageConfig.enabled}
        className="w-full px-2 py-1 text-xs glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all disabled:opacity-40 disabled:cursor-not-allowed"
      >
        {models.map((opt) => (
          <option key={opt.value} value={opt.value}>{opt.label}</option>
        ))}
      </select>
    </div>
  );
}

function ZoneSeparator({ label }: { label: string }) {
  return (
    <div className="flex items-center gap-2 px-2 py-0.5">
      <div className="flex-1 border-t border-dashed border-board-border/30" />
      <span className="text-[9px] font-medium text-board-text-muted/60 uppercase tracking-widest">{label}</span>
      <div className="flex-1 border-t border-dashed border-board-border/30" />
    </div>
  );
}

function WorkflowSection({ agentId, config, models }: { agentId: string; config: AgentConfig; models: { value: AIModel; label: string }[] }) {
  const setStageOrder = useSettingsStore((s) => s.setAgentConfigStageOrder);
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);
  const catalog = useSettingsStore((s) => s.commandsCatalog);

  const catalogMap = useMemo(
    () => new Map(catalog.map((c) => [c.id, c])),
    [catalog],
  );

  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  const handleDragEnd = useCallback((event: DragEndEvent) => {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const oldOrder = config.stageOrder;
    const oldIdx = oldOrder.indexOf(active.id as string);
    const newIdx = oldOrder.indexOf(over.id as string);
    if (oldIdx === -1 || newIdx === -1) return;

    const newOrder = [...oldOrder];
    newOrder.splice(oldIdx, 1);
    newOrder.splice(newIdx, 0, active.id as string);

    if (newOrder[0] !== 'branchGen') return;
    if (newOrder[newOrder.length - 1] !== 'commit') return;
    const planIdx = newOrder.indexOf('plan');
    const implIdx = newOrder.indexOf('implement');
    if (planIdx >= implIdx) return;

    setStageOrder(agentId, newOrder);
  }, [config.stageOrder, agentId, setStageOrder]);

  const sortableKeys = useMemo(
    () => config.stageOrder.filter((k) => !REQUIRED_STAGE_KEYS.has(k) && config.workflowStages[k]),
    [config.stageOrder, config.workflowStages],
  );

  const renderStagesWithZones = () => {
    const elements: React.ReactNode[] = [];
    let lastRequiredKey: string | null = null;

    for (const key of config.stageOrder) {
      if (!REQUIRED_STAGE_KEYS.has(key) && !config.workflowStages[key]) continue;
      const isRequired = REQUIRED_STAGE_KEYS.has(key);

      if (!isRequired && lastRequiredKey) {
        const zoneLabels: Record<string, string> = {
          branchGen: 'pre-plan',
          plan: 'post-plan',
          implement: 'post-implement',
        };
        if (zoneLabels[lastRequiredKey] && elements.length > 0) {
          const zoneKey = `zone-${lastRequiredKey}-${key}`;
          if (!elements.some((el) => el && typeof el === 'object' && 'key' in el && (el as React.ReactElement).key === zoneKey)) {
            elements.push(<ZoneSeparator key={zoneKey} label={zoneLabels[lastRequiredKey]} />);
          }
        }
        lastRequiredKey = null;
      }

      elements.push(
        <SortableStageRow
          key={key}
          stageKey={key}
          agentId={agentId}
          config={config}
          models={models}
          catalogInfo={catalogMap.get(key)}
        />
      );

      if (isRequired) {
        lastRequiredKey = key;
      }
    }

    return elements;
  };

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-board-text">Workflow</h3>
        <p className="text-xs text-board-text-muted mt-0.5">Configure workflow stages and models. Manage available commands in the Commands tab.</p>
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h4 className="text-sm font-medium text-board-text">Stage Configuration</h4>
          <p className="text-xs text-board-text-muted mt-0.5">Toggle stages and choose models. Drag command stages to reorder.</p>
        </div>
        <div className="space-y-1">
          <div className="grid grid-cols-[20px_40px_1fr_130px] gap-2 px-2 py-1">
            <span />
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">On</span>
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">Stage</span>
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">Model</span>
          </div>
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            modifiers={[restrictToVerticalAxis, restrictToParentElement]}
            onDragEnd={handleDragEnd}
          >
            <SortableContext items={sortableKeys} strategy={verticalListSortingStrategy}>
              {renderStagesWithZones()}
            </SortableContext>
          </DndContext>
        </div>
      </div>

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
