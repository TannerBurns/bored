import { useCallback, useMemo } from 'react';
import { DndContext, closestCenter, PointerSensor, useSensor, useSensors, type DragEndEvent } from '@dnd-kit/core';
import { SortableContext, useSortable, verticalListSortingStrategy, arrayMove } from '@dnd-kit/sortable';
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
  CLAUDE_MODEL_OPTIONS,
  CODEX_MODEL_OPTIONS,
  validateStageOrder,
  type AIModel,
  type AgentConfig,
  type AutoPilotRequiredCommand,
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

function getModelOptions(
  agentId: string,
  availableModels?: AgentModelOption[],
  cursorModels?: { value: string; label: string }[],
): { value: AIModel; label: string }[] {
  if (agentId === 'cursor' && cursorModels && cursorModels.length > 0) {
    return cursorModels.map((m) => ({ value: m.value as AIModel, label: m.label }));
  }
  if (availableModels && availableModels.length > 0) {
    return availableModels.map((m) => ({ value: m.value as AIModel, label: m.label }));
  }
  if (agentId === 'codex') return CODEX_MODEL_OPTIONS;
  return CLAUDE_MODEL_OPTIONS;
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

function useModelColWidth(models: { value: string; label: string }[]): number {
  return useMemo(() => {
    const longest = models.reduce((max, m) => Math.max(max, m.label.length), 0);
    return Math.max(130, longest * 7.5 + 28);
  }, [models]);
}

function SortableStageRow({
  stageKey, agentId, config, models, catalogInfo, modelColWidth, autoPilotMode,
}: {
  stageKey: string;
  agentId: string;
  config: AgentConfig;
  models: { value: AIModel; label: string }[];
  catalogInfo?: CatalogCommand;
  modelColWidth: number;
  autoPilotMode?: boolean;
}) {
  const setStage = useSettingsStore((s) => s.setAgentConfigStage);
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);
  const isRequired = REQUIRED_STAGE_KEYS.has(stageKey);
  const requiredInfo = REQUIRED_STAGE_INFO.get(stageKey);

  const label = requiredInfo?.label ?? catalogInfo?.name ?? stageKey;
  const description = requiredInfo?.description ?? catalogInfo?.description ?? '';
  const stageConfig = config.workflowStages[stageKey];

  const requiredEntry: AutoPilotRequiredCommand | undefined = autoPilotMode && !isRequired
    ? (config.autoPilotRequiredCommands ?? []).find((r) => r.command === stageKey)
    : undefined;
  const isAutoPilotRequired = !!requiredEntry;

  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: stageKey, disabled: isRequired || !!autoPilotMode });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  if (!stageConfig) return null;

  const toggleActive = autoPilotMode ? isAutoPilotRequired : stageConfig.enabled;

  const handleToggle = () => {
    if (isRequired) return;
    if (autoPilotMode) {
      const current = config.autoPilotRequiredCommands ?? [];
      const next = isAutoPilotRequired
        ? current.filter((r) => r.command !== stageKey)
        : [...current, { command: stageKey, phase: 'after' as const }];
      updateConfig(agentId, { autoPilotRequiredCommands: next });
    } else {
      setStage(agentId, stageKey, { enabled: !stageConfig.enabled });
    }
  };

  const handlePhaseToggle = () => {
    if (!requiredEntry) return;
    const current = config.autoPilotRequiredCommands ?? [];
    const next = current.map((r) =>
      r.command === stageKey
        ? { ...r, phase: (r.phase === 'before' ? 'after' : 'before') as 'before' | 'after' }
        : r
    );
    updateConfig(agentId, { autoPilotRequiredCommands: next });
  };

  const modelDisabled = autoPilotMode ? !isAutoPilotRequired && !isRequired : !stageConfig.enabled;

  const gridStyle = {
    ...style,
    gridTemplateColumns: `20px 40px 1fr ${modelColWidth}px`,
  };

  return (
    <div
      ref={setNodeRef}
      style={gridStyle}
      className={cn(
        'grid gap-2 items-center px-2 py-1.5 rounded-lg transition-all duration-150',
        autoPilotMode && !isRequired
          ? isAutoPilotRequired
            ? 'glass-subtle ring-1 ring-emerald-500/20'
            : 'opacity-60 hover:opacity-80'
          : toggleActive ? 'glass-subtle' : 'opacity-50',
        isDragging && 'opacity-70 ring-1 ring-board-accent z-10',
      )}
    >
      {!isRequired && !autoPilotMode ? (
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
        onClick={handleToggle}
        disabled={isRequired}
        className={cn(
          'relative inline-flex h-5 w-9 flex-shrink-0 rounded-full transition-colors duration-200',
          isRequired ? 'cursor-not-allowed' : 'cursor-pointer',
          autoPilotMode && !isRequired
            ? toggleActive ? 'bg-emerald-500' : 'glass'
            : toggleActive ? 'bg-board-accent' : 'glass'
        )}
        title={isRequired ? 'Required stage' : autoPilotMode ? `${isAutoPilotRequired ? 'Remove from' : 'Add to'} always-run commands` : `${stageConfig.enabled ? 'Disable' : 'Enable'} ${label}`}
      >
        <span className={cn(
          'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white shadow transition duration-200',
          toggleActive ? 'translate-x-4' : 'translate-x-0.5'
        )} style={{ marginTop: '2px' }} />
      </button>
      <div className="min-w-0">
        <div className="flex items-center gap-1.5 flex-wrap">
          <span className="text-sm font-medium text-board-text">{label}</span>
          {isRequired && (
            <span className="text-[9px] font-medium px-1 py-0 rounded-full bg-board-accent/15 text-board-accent leading-relaxed">required</span>
          )}
          {catalogInfo && stageKey === 'code-review' && (
            <span className="text-[9px] font-medium px-1 py-0 rounded-full bg-purple-500/15 text-purple-400 leading-relaxed">composite</span>
          )}
          {autoPilotMode && !isRequired && isAutoPilotRequired && (
            <span className="inline-flex items-center gap-1.5 text-[10px] font-medium leading-none">
              <span className="inline-flex rounded-md overflow-hidden border border-board-border/30">
                <button
                  onClick={() => { if (requiredEntry?.phase !== 'before') handlePhaseToggle(); }}
                  className={cn(
                    'px-2 py-1 transition-colors',
                    requiredEntry?.phase === 'before'
                      ? 'bg-amber-500/20 text-amber-400'
                      : 'text-board-text-muted/70 hover:text-board-text hover:bg-board-text/10',
                  )}
                  title="Run before auto-pilot selected commands"
                >
                  before
                </button>
                <button
                  onClick={() => { if (requiredEntry?.phase === 'before') handlePhaseToggle(); }}
                  className={cn(
                    'px-2 py-1 transition-colors border-l border-board-border/30',
                    requiredEntry?.phase !== 'before'
                      ? 'bg-sky-500/20 text-sky-400'
                      : 'text-board-text-muted/70 hover:text-board-text hover:bg-board-text/10',
                  )}
                  title="Run after auto-pilot selected commands"
                >
                  after
                </button>
              </span>
              <span className="text-board-text-muted">auto-pilot commands</span>
            </span>
          )}
          {autoPilotMode && !isRequired && !isAutoPilotRequired && (
            <span className="text-[10px] text-board-text-muted/50 italic">auto-pilot decides</span>
          )}
        </div>
        <p className="text-[11px] text-board-text-muted truncate">{description}</p>
      </div>
      {autoPilotMode && !isRequired && !isAutoPilotRequired ? (
        <div className="w-full px-2 py-1 text-xs text-board-text-muted/40 italic text-center">--</div>
      ) : (
        <select
          value={stageConfig.model}
          onChange={(e) => setStage(agentId, stageKey, { model: e.target.value as AIModel })}
          disabled={modelDisabled}
          className="w-full px-2 py-1 text-xs glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent transition-all disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {models.map((opt) => (
            <option key={opt.value} value={opt.value}>{opt.label}</option>
          ))}
        </select>
      )}
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

function WorkflowSection({ agentId, config, models, modelColWidth }: { agentId: string; config: AgentConfig; models: { value: AIModel; label: string }[]; modelColWidth: number }) {
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

    const newOrder = arrayMove(oldOrder, oldIdx, newIdx);

    if (!validateStageOrder(newOrder)) return;

    setStageOrder(agentId, newOrder);
  }, [config.stageOrder, agentId, setStageOrder]);

  const sortableKeys = useMemo(
    () => config.stageOrder.filter((k) => !REQUIRED_STAGE_KEYS.has(k) && config.workflowStages[k]),
    [config.stageOrder, config.workflowStages],
  );

  const renderStagesWithZones = () => {
    if (config.autoPilotEnabled) {
      const coreElements: React.ReactNode[] = [];
      const cmdElements: React.ReactNode[] = [];

      for (const key of config.stageOrder) {
        if (REQUIRED_STAGE_KEYS.has(key)) {
          coreElements.push(
            <SortableStageRow key={key} stageKey={key} agentId={agentId} config={config}
              models={models} catalogInfo={catalogMap.get(key)} modelColWidth={modelColWidth} />
          );
        } else if (config.workflowStages[key]) {
          cmdElements.push(
            <SortableStageRow key={key} stageKey={key} agentId={agentId} config={config}
              models={models} catalogInfo={catalogMap.get(key)} modelColWidth={modelColWidth}
              autoPilotMode />
          );
        }
      }

      return [
        ...coreElements,
        ...(cmdElements.length > 0 ? [
          <div key="auto-pilot-sep" className="px-2 py-2 mt-1">
            <div className="flex items-center gap-2">
              <div className="flex-1 border-t border-dashed border-emerald-500/30" />
              <span className="text-[10px] font-semibold text-emerald-400/80 uppercase tracking-wider">Commands</span>
              <div className="flex-1 border-t border-dashed border-emerald-500/30" />
            </div>
            <p className="text-[11px] text-board-text-muted text-center mt-1">Toggle on to always run a command. Auto-pilot selects the rest.</p>
          </div>,
          <div key="auto-pilot-cmds" className="space-y-1">
            {cmdElements}
          </div>,
        ] : []),
      ];
    }

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
          modelColWidth={modelColWidth}
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
        <ToggleRow
          label="Auto-Pilot"
          description="Let the agent decide which commands to run after implementation instead of using the static stage pipeline"
          enabled={config.autoPilotEnabled}
          onChange={(v) => updateConfig(agentId, { autoPilotEnabled: v })}
        />
        <ToggleRow
          label="Auto-Complete Tickets"
          description="Automatically move tickets to Done instead of Review when the agent finishes work"
          enabled={config.autoCompleteTickets}
          onChange={(v) => updateConfig(agentId, { autoCompleteTickets: v })}
        />
        <ToggleRow
          label="Auto-Clarification"
          description="Let the agent resolve clarification questions automatically instead of blocking the ticket for user input"
          enabled={config.autoClarification}
          onChange={(v) => updateConfig(agentId, { autoClarification: v })}
        />
        {config.autoPilotEnabled && (
          <div className="flex items-center justify-between gap-3 pt-1">
            <div className="flex-1 min-w-0">
              <p className="text-sm font-medium text-board-text">Model</p>
              <p className="text-xs text-board-text-muted">Model used for the auto-pilot command selection call</p>
            </div>
            <select
              value={config.autoPilotModel}
              onChange={(e) => updateConfig(agentId, { autoPilotModel: e.target.value as AIModel })}
              style={{ maxWidth: modelColWidth }}
              className="w-full px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent"
            >
              {models.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
            </select>
          </div>
        )}
      </div>

      <div className="glass rounded-lg p-3 space-y-3">
        <div>
          <h4 className="text-sm font-medium text-board-text">Stage Configuration</h4>
          <p className="text-xs text-board-text-muted mt-0.5">
            {config.autoPilotEnabled
              ? 'Choose models for core stages. Toggle commands on to always run them.'
              : 'Toggle stages and choose models. Drag command stages to reorder.'}
          </p>
        </div>
        <div className="space-y-1">
          <div className="grid gap-2 px-2 py-1" style={{ gridTemplateColumns: `20px 40px 1fr ${modelColWidth}px` }}>
            <span />
            <span className="text-[11px] font-medium text-board-text-muted uppercase tracking-wider">{config.autoPilotEnabled ? 'Always' : 'On'}</span>
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

interface AgentSectionProps {
  title: string;
  description: string;
  agentId: string;
  config: AgentConfig;
  models: { value: AIModel; label: string }[];
  modelColWidth: number;
  modelKey: keyof AgentConfig;
  timeoutKey: keyof AgentConfig;
  retriesKey: keyof AgentConfig;
  children?: React.ReactNode;
}

function AgentSection({ title, description, agentId, config, models, modelColWidth, modelKey, timeoutKey, retriesKey, children }: AgentSectionProps) {
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-board-text">{title}</h3>
        <p className="text-xs text-board-text-muted mt-0.5">{description}</p>
      </div>
      <div className="glass rounded-lg p-3 space-y-3">
        {children}
        <div className="glass-subtle rounded-lg px-3 py-2">
          <label className="block text-sm font-medium text-board-text mb-1">Model</label>
          <select value={config[modelKey] as string}
            onChange={(e) => updateConfig(agentId, { [modelKey]: e.target.value as AIModel })}
            style={{ maxWidth: modelColWidth }}
            className="w-full px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent">
            {models.map((opt) => <option key={opt.value} value={opt.value}>{opt.label}</option>)}
          </select>
        </div>
        <div className="grid grid-cols-2 gap-2">
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Timeout (min)</label>
            <input type="number" min={1} max={120} value={config[timeoutKey] as number}
              onChange={(e) => updateConfig(agentId, { [timeoutKey]: Math.max(1, Math.min(120, parseInt(e.target.value) || 10)) })}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
          </div>
          <div className="glass-subtle rounded-lg px-3 py-2">
            <label className="block text-sm font-medium text-board-text mb-1">Max Retries</label>
            <input type="number" min={0} max={5} value={config[retriesKey] as number}
              onChange={(e) => updateConfig(agentId, { [retriesKey]: Math.max(0, Math.min(5, parseInt(e.target.value) || 0)) })}
              className="w-16 px-2 py-1 text-sm glass rounded-lg text-board-text focus:ring-1 focus:ring-board-accent" />
          </div>
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
  const updateConfig = useSettingsStore((s) => s.updateAgentConfig);
  const cursorModels = useSettingsStore((s) => s.cursorModels);

  const Icon = getAgentIcon(agentId);
  const brandColor = getAgentBrandColor(agentId, agent?.brandColor);
  const displayName = agent?.displayName ?? agentId.charAt(0).toUpperCase() + agentId.slice(1);
  const models = getModelOptions(agentId, agent?.availableModels, cursorModels);
  const modelColWidth = useModelColWidth(models);

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

      <WorkflowSection agentId={agentId} config={config} models={models} modelColWidth={modelColWidth} />
      <hr className="border-board-border/30" />
      <AgentSection title="General Chat Agent" description="Settings for general-purpose chat conversations."
        agentId={agentId} config={config} models={models} modelColWidth={modelColWidth}
        modelKey="generalModel" timeoutKey="generalTimeoutMinutes" retriesKey="generalMaxRetries" />
      <hr className="border-board-border/30" />
      <AgentSection title="Spec Agent" description="Configure how the AI spec agent generates plans."
        agentId={agentId} config={config} models={models} modelColWidth={modelColWidth}
        modelKey="plannerModel" timeoutKey="plannerTimeoutMinutes" retriesKey="plannerMaxRetries">
        <ToggleRow
          label="Auto-approve Plans"
          description="Automatically approve generated plans without manual review"
          enabled={config.plannerAutoApprove}
          onChange={(v) => updateConfig(agentId, { plannerAutoApprove: v })}
        />
      </AgentSection>
      <hr className="border-board-border/30" />
      <AgentSection title="Ticket Builder Agent" description="Settings for ticket builder chat conversations."
        agentId={agentId} config={config} models={models} modelColWidth={modelColWidth}
        modelKey="ticketBuilderModel" timeoutKey="ticketBuilderTimeoutMinutes" retriesKey="ticketBuilderMaxRetries" />
      <hr className="border-board-border/30" />
      <AgentSection title="Review Agent" description="Settings for ticket review chat."
        agentId={agentId} config={config} models={models} modelColWidth={modelColWidth}
        modelKey="validationModel" timeoutKey="validationTimeoutMinutes" retriesKey="validationMaxRetries" />
      <hr className="border-board-border/30" />
      <AgentSection title="Diagnostic Agent" description="Settings for diagnosing worktree and git failures."
        agentId={agentId} config={config} models={models} modelColWidth={modelColWidth}
        modelKey="diagnosticModel" timeoutKey="diagnosticTimeoutMinutes" retriesKey="diagnosticMaxRetries" />
    </div>
  );
}
