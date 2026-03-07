export type AIModel = 'claude-opus-4-6' | 'claude-opus-4-5' | 'claude-sonnet-4-6' | 'claude-sonnet-4-5' | 'gpt-5.4' | 'gpt-5.3-codex' | 'gpt-5.2-codex' | (string & {});

export const CLAUDE_MODEL_OPTIONS: { value: AIModel; label: string }[] = [
  { value: 'claude-opus-4-6', label: 'Opus 4.6' },
  { value: 'claude-opus-4-5', label: 'Opus 4.5' },
  { value: 'claude-sonnet-4-6', label: 'Sonnet 4.6' },
  { value: 'claude-sonnet-4-5', label: 'Sonnet 4.5' },
];

export const CODEX_MODEL_OPTIONS: { value: AIModel; label: string }[] = [
  { value: 'gpt-5.4', label: 'GPT-5.4' },
  { value: 'gpt-5.3-codex', label: 'GPT-5.3 Codex' },
  { value: 'gpt-5.2-codex', label: 'GPT-5.2 Codex' },
];

export interface WorkflowStageConfig {
  enabled: boolean;
  model: AIModel;
}

export interface CatalogCommand {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
  source: 'builtin' | 'custom';
  filename: string;
}

export type WorkflowStageKey = 'branchGen' | 'plan' | 'implement' | 'commit' | (string & {});

export type WorkflowStages = Record<string, WorkflowStageConfig>;

export const REQUIRED_STAGE_KEYS: ReadonlySet<string> = new Set([
  'branchGen', 'plan', 'implement', 'commit',
]);

/** Backend stage names produced by expanding required/special frontend keys.
 *  Custom command IDs must not collide with these or `should_skip_stage`
 *  resume logic breaks due to duplicate positions in `full_execution_order`. */
export const RESERVED_INTERNAL_STAGE_IDS: ReadonlySet<string> = new Set([
  'branch-gen', 'branch', 'plan-validation', 'plan-decompose',
  'code-review-fix', 'add-and-commit',
]);

export const DEFAULT_STAGE_ORDER: string[] = [
  'branchGen', 'plan', 'implement',
  'code-review', 'cleanup', 'unit-tests', 'review-changes', 'deslop',
  'commit',
];

interface WorkflowStageInfo {
  key: string;
  label: string;
  description: string;
  required: boolean;
}

export const WORKFLOW_STAGE_INFO: WorkflowStageInfo[] = [
  { key: 'branchGen', label: 'Branch Name', description: 'Generate a descriptive branch name for the changes', required: true },
  { key: 'plan', label: 'Plan', description: 'Explore the codebase and generate an implementation plan', required: true },
  { key: 'implement', label: 'Implement', description: 'Write the code changes based on the plan', required: true },
  { key: 'commit', label: 'Commit', description: 'Stage changes and create a git commit', required: true },
];

/** Expand a frontend stage key into backend execution stage names.
 *  Mirrors the backend `expand_stage_key` in orchestrator/config.rs. */
export function expandStageKey(key: string): string[] {
  switch (key) {
    case 'branchGen': return ['branch-gen', 'branch'];
    case 'plan': return ['plan', 'plan-validation', 'plan-decompose'];
    case 'implement': return ['implement'];
    case 'code-review': return ['code-review', 'code-review-fix'];
    case 'commit': return ['add-and-commit'];
    default: return [key];
  }
}

/** Build the full backend execution-order list from a frontend stage order.
 *  Mirrors the backend `build_full_stage_order` in orchestrator/config.rs. */
export function buildFullExecutionOrder(stageOrder: string[]): string[] {
  const seen = new Set<string>();
  const order: string[] = [];
  for (const key of stageOrder) {
    for (const stage of expandStageKey(key)) {
      if (!seen.has(stage)) {
        seen.add(stage);
        order.push(stage);
      }
    }
  }
  return order;
}

export const BUILTIN_CATALOG_COMMANDS: CatalogCommand[] = [
  { id: 'code-review', name: 'Code Review', description: 'Iterative review loop to find and fix issues', enabled: true, source: 'builtin', filename: 'code-review.md' },
  { id: 'cleanup', name: 'Cleanup', description: 'Run linters, fix build warnings, and clean up code', enabled: true, source: 'builtin', filename: 'cleanup.md' },
  { id: 'unit-tests', name: 'Unit Tests', description: 'Generate and run unit tests for the changes', enabled: true, source: 'builtin', filename: 'unit-tests.md' },
  { id: 'review-changes', name: 'Review Changes', description: 'Senior code review for correctness and security', enabled: true, source: 'builtin', filename: 'review-changes.md' },
  { id: 'deslop', name: 'De-slop', description: 'Remove AI-generated slop and improve code taste', enabled: true, source: 'builtin', filename: 'deslop.md' },
  { id: 'add-tests', name: 'Add Tests', description: 'Add comprehensive tests for the changes', enabled: false, source: 'builtin', filename: 'add-tests.md' },
  { id: 'fix-lint', name: 'Fix Lint', description: 'Fix linting errors and warnings', enabled: false, source: 'builtin', filename: 'fix-lint.md' },
  { id: 'sync-with-main', name: 'Sync with Main', description: 'Sync the working branch with the main branch', enabled: false, source: 'builtin', filename: 'sync-with-main.md' },
  { id: 'review-polish', name: 'Review & Polish', description: 'Final review and polishing pass', enabled: false, source: 'builtin', filename: 'review-polish.md' },
  { id: 'patch-security', name: 'Patch Security', description: 'Security review and fix pass scoped to branch diff', enabled: false, source: 'builtin', filename: 'patch-security.md' },
  { id: 'api-contract-check', name: 'API Contract Check', description: 'Verify and fix public contract consistency across call sites', enabled: false, source: 'builtin', filename: 'api-contract-check.md' },
  { id: 'observability-pass', name: 'Observability Pass', description: 'Align logs, metrics, and tracing with repo standards', enabled: false, source: 'builtin', filename: 'observability-pass.md' },
  { id: 'integration-test', name: 'Integration Test', description: 'Add minimal integration tests for boundary-spanning changes', enabled: false, source: 'builtin', filename: 'integration-test.md' },
  { id: 'doc-sync', name: 'Documentation Sync', description: 'Update or create documentation from branch changes', enabled: false, source: 'builtin', filename: 'doc-sync.md' },
];

export interface AgentConfig {
  autoPilotEnabled: boolean;
  autoPilotModel: AIModel;
  autoCompleteTickets: boolean;
  workflowStages: WorkflowStages;
  stageOrder: string[];
  stageTimeoutHours: number;
  stageMaxRetries: number;
  codeReviewMaxIterations: number;

  plannerModel: AIModel;
  plannerAutoApprove: boolean;
  plannerMaxExplorations: number;
  plannerTimeoutMinutes: number;
  plannerMaxRetries: number;

  generalModel: AIModel;

  ticketBuilderModel: AIModel;

  validationModel: AIModel;
  validationTimeoutMinutes: number;

  diagnosticModel: AIModel;

  settings: Record<string, unknown>;
}

/** Claude CLI requires full model identifiers (claude-opus-4-6, not opus-4.6). */
export const DEFAULT_CLAUDE_WORKFLOW_STAGES: WorkflowStages = {
  branchGen:         { enabled: true, model: 'claude-sonnet-4-6' },
  plan:              { enabled: true, model: 'claude-opus-4-6' },
  implement:         { enabled: true, model: 'claude-opus-4-6' },
  'code-review':     { enabled: true, model: 'claude-opus-4-6' },
  cleanup:           { enabled: true, model: 'claude-sonnet-4-6' },
  'unit-tests':      { enabled: true, model: 'claude-opus-4-5' },
  'review-changes':  { enabled: true, model: 'claude-opus-4-5' },
  deslop:            { enabled: true, model: 'claude-opus-4-5' },
  commit:            { enabled: true, model: 'claude-sonnet-4-6' },
};

/** Codex uses its own native model identifiers. */
export const DEFAULT_CODEX_WORKFLOW_STAGES: WorkflowStages = {
  branchGen:         { enabled: true, model: 'gpt-5.2-codex' },
  plan:              { enabled: true, model: 'gpt-5.4' },
  implement:         { enabled: true, model: 'gpt-5.4' },
  'code-review':     { enabled: true, model: 'gpt-5.4' },
  cleanup:           { enabled: true, model: 'gpt-5.2-codex' },
  'unit-tests':      { enabled: true, model: 'gpt-5.4' },
  'review-changes':  { enabled: true, model: 'gpt-5.4' },
  deslop:            { enabled: true, model: 'gpt-5.4' },
  commit:            { enabled: true, model: 'gpt-5.2-codex' },
};


function deepCopyStages(stages: WorkflowStages): WorkflowStages {
  const copy: WorkflowStages = {};
  for (const [key, val] of Object.entries(stages)) {
    copy[key] = { ...val };
  }
  return copy;
}

function cloneConfig(base: AgentConfig, settingsOverride?: Record<string, unknown>): AgentConfig {
  return {
    ...base,
    workflowStages: deepCopyStages(base.workflowStages),
    stageOrder: [...base.stageOrder],
    settings: { ...(settingsOverride ?? base.settings) },
  };
}

const DEFAULT_CLAUDE_CONFIG: AgentConfig = {
  autoPilotEnabled: false,
  autoPilotModel: 'claude-opus-4-6',
  autoCompleteTickets: false,
  workflowStages: { ...DEFAULT_CLAUDE_WORKFLOW_STAGES },
  stageOrder: [...DEFAULT_STAGE_ORDER],
  stageTimeoutHours: 1,
  stageMaxRetries: 2,
  codeReviewMaxIterations: 3,
  plannerModel: 'claude-opus-4-5',
  plannerAutoApprove: false,
  plannerMaxExplorations: 10,
  plannerTimeoutMinutes: 10,
  plannerMaxRetries: 2,
  generalModel: 'claude-opus-4-6',
  ticketBuilderModel: 'claude-opus-4-5',
  validationModel: 'claude-sonnet-4-6',
  validationTimeoutMinutes: 10,
  diagnosticModel: 'claude-sonnet-4-6',
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
  autoPilotEnabled: false,
  autoPilotModel: 'claude-opus-4-6',
  autoCompleteTickets: false,
  workflowStages: { ...DEFAULT_CLAUDE_WORKFLOW_STAGES },
  stageOrder: [...DEFAULT_STAGE_ORDER],
  stageTimeoutHours: 1,
  stageMaxRetries: 2,
  codeReviewMaxIterations: 3,
  plannerModel: 'claude-opus-4-5',
  plannerAutoApprove: false,
  plannerMaxExplorations: 10,
  plannerTimeoutMinutes: 10,
  plannerMaxRetries: 2,
  generalModel: 'claude-opus-4-6',
  ticketBuilderModel: 'claude-opus-4-5',
  validationModel: 'claude-sonnet-4-6',
  validationTimeoutMinutes: 10,
  diagnosticModel: 'claude-sonnet-4-6',
  settings: {},
};

const DEFAULT_CODEX_CONFIG: AgentConfig = {
  autoPilotEnabled: false,
  autoPilotModel: 'gpt-5.4',
  autoCompleteTickets: false,
  workflowStages: { ...DEFAULT_CODEX_WORKFLOW_STAGES },
  stageOrder: [...DEFAULT_STAGE_ORDER],
  stageTimeoutHours: 1,
  stageMaxRetries: 2,
  codeReviewMaxIterations: 3,
  plannerModel: 'gpt-5.4',
  plannerAutoApprove: false,
  plannerMaxExplorations: 10,
  plannerTimeoutMinutes: 10,
  plannerMaxRetries: 2,
  generalModel: 'gpt-5.4',
  ticketBuilderModel: 'gpt-5.4',
  validationModel: 'gpt-5.2-codex',
  validationTimeoutMinutes: 10,
  diagnosticModel: 'gpt-5.2-codex',
  settings: {
    ossEnabled: false,
    localProvider: 'ollama',
    modelOverride: '',
    reasoningEffort: 'high',
    multiAgentEnabled: true,
  },
};

export function getDefaultConfigForAgent(agentId: string): AgentConfig {
  switch (agentId) {
    case 'claude': return cloneConfig(DEFAULT_CLAUDE_CONFIG);
    case 'cursor': return cloneConfig(DEFAULT_CURSOR_CONFIG);
    case 'codex': return cloneConfig(DEFAULT_CODEX_CONFIG);
    default: return cloneConfig(DEFAULT_CLAUDE_CONFIG, {});
  }
}

export function validateStageOrder(order: string[]): boolean {
  if (order.length === 0) return false;
  if (order[0] !== 'branchGen') return false;
  if (order[order.length - 1] !== 'commit') return false;
  const planIdx = order.indexOf('plan');
  const implIdx = order.indexOf('implement');
  if (planIdx === -1 || implIdx === -1) return false;
  return planIdx < implIdx;
}
