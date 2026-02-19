export type AIModel = 'opus-4.6' | 'opus-4.5' | 'sonnet-4.6' | 'sonnet-4.5' | 'gpt-5.3-codex' | 'gpt-5.2-codex' | (string & {});

export const MODEL_OPTIONS: { value: AIModel; label: string }[] = [
  { value: 'opus-4.6', label: 'Opus 4.6' },
  { value: 'opus-4.5', label: 'Opus 4.5' },
  { value: 'sonnet-4.6', label: 'Sonnet 4.6' },
  { value: 'sonnet-4.5', label: 'Sonnet 4.5' },
];

export const CODEX_MODEL_OPTIONS: { value: AIModel; label: string }[] = [
  { value: 'gpt-5.3-codex', label: 'GPT-5.3 Codex' },
  { value: 'gpt-5.2-codex', label: 'GPT-5.2 Codex' },
];

export interface WorkflowStageConfig {
  enabled: boolean;
  model: AIModel;
}

export type WorkflowStageKey =
  | 'branchGen'
  | 'plan'
  | 'implement'
  | 'codeReview'
  | 'deslop'
  | 'cleanup'
  | 'unitTests'
  | 'finalReview'
  | 'commit';

export type WorkflowStages = Record<WorkflowStageKey, WorkflowStageConfig>;

export type WorkflowPreset =
  | 'comprehensive'
  | 'balanced'
  | 'vibe'
  | 'standard'
  | 'quick-fix'
  | 'fastest'
  | 'custom';

interface WorkflowStageInfo {
  key: WorkflowStageKey;
  label: string;
  description: string;
  required: boolean;
}

export const WORKFLOW_STAGE_INFO: WorkflowStageInfo[] = [
  { key: 'branchGen', label: 'Branch Name', description: 'Generate a descriptive branch name for the changes', required: true },
  { key: 'plan', label: 'Plan', description: 'Explore the codebase and generate an implementation plan', required: true },
  { key: 'implement', label: 'Implement', description: 'Write the code changes based on the plan', required: true },
  { key: 'codeReview', label: 'Code Review', description: 'Iterative review loop to find and fix issues', required: false },
  { key: 'deslop', label: 'De-slop', description: 'Remove AI-generated slop and improve code taste', required: false },
  { key: 'cleanup', label: 'Cleanup', description: 'Run linters, fix build warnings, and clean up code', required: false },
  { key: 'unitTests', label: 'Unit Tests', description: 'Generate and run unit tests for the changes', required: false },
  { key: 'finalReview', label: 'Final Review', description: 'Senior code review for correctness and security', required: false },
  { key: 'commit', label: 'Commit', description: 'Stage changes and create a git commit', required: true },
];

export const WORKFLOW_PRESETS: Record<Exclude<WorkflowPreset, 'custom'>, { label: string; description: string; stages: WorkflowStages }> = {
  comprehensive: {
    label: 'Most Comprehensive',
    description: 'Maximum quality, highest cost — all stages with Opus 4.6',
    stages: {
      branchGen:   { enabled: true, model: 'sonnet-4.6' },
      plan:        { enabled: true, model: 'opus-4.6' },
      implement:   { enabled: true, model: 'opus-4.6' },
      codeReview:  { enabled: true, model: 'opus-4.6' },
      deslop:      { enabled: true, model: 'opus-4.6' },
      cleanup:     { enabled: true, model: 'opus-4.6' },
      unitTests:   { enabled: true, model: 'opus-4.6' },
      finalReview: { enabled: true, model: 'opus-4.6' },
      commit:      { enabled: true, model: 'opus-4.6' },
    },
  },
  balanced: {
    label: 'Balanced',
    description: 'Smart cost/quality tradeoff — all stages, mixed models',
    stages: {
      branchGen:   { enabled: true, model: 'sonnet-4.6' },
      plan:        { enabled: true, model: 'opus-4.6' },
      implement:   { enabled: true, model: 'opus-4.6' },
      codeReview:  { enabled: true, model: 'opus-4.6' },
      deslop:      { enabled: true, model: 'opus-4.5' },
      cleanup:     { enabled: true, model: 'sonnet-4.6' },
      unitTests:   { enabled: true, model: 'opus-4.5' },
      finalReview: { enabled: true, model: 'opus-4.5' },
      commit:      { enabled: true, model: 'sonnet-4.6' },
    },
  },
  vibe: {
    label: 'Vibe',
    description: 'Trust the implementation, light QA — creative core with Opus 4.6',
    stages: {
      branchGen:   { enabled: true,  model: 'sonnet-4.6' },
      plan:        { enabled: true,  model: 'opus-4.6' },
      implement:   { enabled: true,  model: 'opus-4.6' },
      codeReview:  { enabled: true,  model: 'opus-4.5' },
      deslop:      { enabled: true,  model: 'sonnet-4.6' },
      cleanup:     { enabled: false, model: 'sonnet-4.6' },
      unitTests:   { enabled: false, model: 'sonnet-4.6' },
      finalReview: { enabled: false, model: 'sonnet-4.6' },
      commit:      { enabled: true,  model: 'sonnet-4.6' },
    },
  },
  standard: {
    label: 'Standard',
    description: 'Core workflow without polish — skips deslop and final review',
    stages: {
      branchGen:   { enabled: true,  model: 'sonnet-4.6' },
      plan:        { enabled: true,  model: 'opus-4.5' },
      implement:   { enabled: true,  model: 'opus-4.5' },
      codeReview:  { enabled: true,  model: 'opus-4.5' },
      deslop:      { enabled: false, model: 'sonnet-4.6' },
      cleanup:     { enabled: true,  model: 'sonnet-4.6' },
      unitTests:   { enabled: true,  model: 'sonnet-4.6' },
      finalReview: { enabled: false, model: 'sonnet-4.6' },
      commit:      { enabled: true,  model: 'sonnet-4.6' },
    },
  },
  'quick-fix': {
    label: 'Quick Fix',
    description: 'Minimal stages for small changes — plan, implement, cleanup, commit',
    stages: {
      branchGen:   { enabled: true,  model: 'sonnet-4.6' },
      plan:        { enabled: true,  model: 'sonnet-4.6' },
      implement:   { enabled: true,  model: 'sonnet-4.6' },
      codeReview:  { enabled: false, model: 'sonnet-4.6' },
      deslop:      { enabled: false, model: 'sonnet-4.6' },
      cleanup:     { enabled: true,  model: 'sonnet-4.6' },
      unitTests:   { enabled: false, model: 'sonnet-4.6' },
      finalReview: { enabled: false, model: 'sonnet-4.6' },
      commit:      { enabled: true,  model: 'sonnet-4.6' },
    },
  },
  fastest: {
    label: 'Fastest',
    description: 'Maximum speed — all stages with Sonnet 4.6',
    stages: {
      branchGen:   { enabled: true, model: 'sonnet-4.6' },
      plan:        { enabled: true, model: 'sonnet-4.6' },
      implement:   { enabled: true, model: 'sonnet-4.6' },
      codeReview:  { enabled: true, model: 'sonnet-4.6' },
      deslop:      { enabled: true, model: 'sonnet-4.6' },
      cleanup:     { enabled: true, model: 'sonnet-4.6' },
      unitTests:   { enabled: true, model: 'sonnet-4.6' },
      finalReview: { enabled: true, model: 'sonnet-4.6' },
      commit:      { enabled: true, model: 'sonnet-4.6' },
    },
  },
};

export interface AgentConfig {
  workflowPreset: WorkflowPreset;
  workflowStages: WorkflowStages;
  stageTimeoutHours: number;
  stageMaxRetries: number;
  codeReviewMaxIterations: number;

  plannerModel: AIModel;
  plannerAutoApprove: boolean;
  plannerMaxExplorations: number;
  plannerTimeoutMinutes: number;
  plannerMaxRetries: number;

  validationModel: AIModel;
  validationTimeoutMinutes: number;

  diagnosticModel: AIModel;

  settings: Record<string, unknown>;
}

export const DEFAULT_WORKFLOW_PRESET: WorkflowPreset = 'balanced';
export const DEFAULT_WORKFLOW_STAGES: WorkflowStages = WORKFLOW_PRESETS.balanced.stages;

export function mapModelForCodex(model: string): string {
  if (model.startsWith('opus')) return 'gpt-5.3-codex';
  if (model.startsWith('sonnet')) return 'gpt-5.2-codex';
  return model;
}

export function mapStagesForCodex(stages: WorkflowStages): WorkflowStages {
  const mapped = {} as WorkflowStages;
  for (const [key, val] of Object.entries(stages)) {
    mapped[key as WorkflowStageKey] = { ...val, model: mapModelForCodex(val.model) };
  }
  return mapped;
}

const DEFAULT_CLAUDE_CONFIG: AgentConfig = {
  workflowPreset: DEFAULT_WORKFLOW_PRESET,
  workflowStages: { ...DEFAULT_WORKFLOW_STAGES },
  stageTimeoutHours: 1,
  stageMaxRetries: 2,
  codeReviewMaxIterations: 3,
  plannerModel: 'opus-4.5',
  plannerAutoApprove: false,
  plannerMaxExplorations: 10,
  plannerTimeoutMinutes: 10,
  plannerMaxRetries: 2,
  validationModel: 'sonnet-4.6',
  validationTimeoutMinutes: 10,
  diagnosticModel: 'sonnet-4.6',
  settings: {
    authToken: '',
    apiKey: '',
    baseUrl: '',
    modelOverride: '',
    thinkingEnabled: true,
    extendedContext: false,
    chromeEnabled: false,
  },
};

const DEFAULT_CURSOR_CONFIG: AgentConfig = {
  ...DEFAULT_CLAUDE_CONFIG,
  settings: { thinkingEnabled: true },
};

const DEFAULT_CODEX_CONFIG: AgentConfig = {
  workflowPreset: DEFAULT_WORKFLOW_PRESET,
  workflowStages: mapStagesForCodex(DEFAULT_WORKFLOW_STAGES),
  stageTimeoutHours: 1,
  stageMaxRetries: 2,
  codeReviewMaxIterations: 3,
  plannerModel: 'gpt-5.3-codex',
  plannerAutoApprove: false,
  plannerMaxExplorations: 10,
  plannerTimeoutMinutes: 10,
  plannerMaxRetries: 2,
  validationModel: 'gpt-5.2-codex',
  validationTimeoutMinutes: 10,
  diagnosticModel: 'gpt-5.2-codex',
  settings: {
    ossEnabled: false,
    localProvider: 'ollama',
    modelOverride: '',
  },
};

function deepCopyStages(stages: WorkflowStages): WorkflowStages {
  const copy = {} as WorkflowStages;
  for (const [key, val] of Object.entries(stages)) {
    copy[key as WorkflowStageKey] = { ...val };
  }
  return copy;
}

function cloneConfig(base: AgentConfig, settingsOverride?: Record<string, unknown>): AgentConfig {
  return {
    ...base,
    workflowStages: deepCopyStages(base.workflowStages),
    settings: { ...(settingsOverride ?? base.settings) },
  };
}

export function getDefaultConfigForAgent(agentId: string): AgentConfig {
  switch (agentId) {
    case 'claude': return cloneConfig(DEFAULT_CLAUDE_CONFIG);
    case 'cursor': return cloneConfig(DEFAULT_CURSOR_CONFIG);
    case 'codex': return cloneConfig(DEFAULT_CODEX_CONFIG);
    default: return cloneConfig(DEFAULT_CLAUDE_CONFIG, {});
  }
}

export function getPresetStagesForAgent(
  preset: Exclude<WorkflowPreset, 'custom'>,
  agentId: string,
): WorkflowStages {
  const stages = deepCopyStages(WORKFLOW_PRESETS[preset].stages);
  if (agentId === 'codex') return mapStagesForCodex(stages);
  return stages;
}
