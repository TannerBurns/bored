export interface Project {
  id: string;
  name: string;
  path: string;
  
  // Safety settings
  allowShellCommands: boolean;
  allowFileWrites: boolean;
  blockedPatterns: string[];
  
  // General
  settings: Record<string, unknown>;
  
  createdAt: Date;
  updatedAt: Date;
}

export interface CreateProjectInput {
  name: string;
  path: string;
}

export interface UpdateProjectInput {
  name?: string;
  allowShellCommands?: boolean;
  allowFileWrites?: boolean;
  blockedPatterns?: string[];
}

export interface Board {
  id: string;
  name: string;
  createdAt: Date;
  updatedAt: Date;
}

export interface Workspace {
  id: string;
  name: string;
  projectIds: string[];
  createdAt: Date;
  updatedAt: Date;
}

export interface Column {
  id: string;
  boardId: string;
  name: string;
  position: number;
  wipLimit?: number;
}

// Note: 'basic' workflow has been removed - all tickets now use multi_stage
export type WorkflowType = 'multi_stage';

/** A single issue from structured code-review output. */
export interface CodeReviewIssue {
  title: string;
  file: string;
  lines: string;
  severity: string;
  description: string;
}

/** Structured iteration data stored in code-review sub-run metadata. */
export interface CodeReviewIterationData {
  code_review_iteration: number;
  code_review_issues_found: number | null;
  code_review_issues_section: string;
  code_review_issues?: CodeReviewIssue[];
}

/** Real-time event payload for code-review iteration progress. */
export interface CodeReviewIterationEvent {
  parentRunId: string;
  iteration: number;
  issuesFound: number | null;
  subRunId: string;
  status: 'running' | 'finished';
}

export interface Ticket {
  id: string;
  boardId: string;
  columnId: string;
  title: string;
  descriptionMd: string;
  priority: 'low' | 'medium' | 'high' | 'urgent';
  labels: string[];
  createdAt: Date | string;
  updatedAt: Date | string;
  lockedByRunId?: string | null;
  lockExpiresAt?: Date | string;
  projectId?: string;
  workspaceId?: string;
  workflowType?: WorkflowType;
  model?: string;
  /** The git branch name for this ticket (agent-generated) */
  branchName?: string;
  /** Whether this ticket is an epic (contains child tickets) */
  isEpic?: boolean;
  /** The parent epic ID (if this ticket is a child of an epic) */
  epicId?: string;
  /** The order of this ticket within its parent epic */
  orderInEpic?: number;
  /** Cross-epic dependency: which epic must complete before this epic can start */
  dependsOnEpicId?: string;
  /** Link back to spec version that created this ticket */
  specVersionId?: string;
  /** When the ticket was paused (if currently paused) */
  pausedAt?: Date | string;
  /** Which workflow stage was active when paused (e.g., "branch", "implement", "deslop", "review") */
  pausedAtStage?: string;
  /** The run ID that was in progress when paused */
  pausedRunId?: string;
}

export type ReadinessCheck =
  | { ready: { projectId: string } }
  | { noProject: null }
  | { projectNotFound: null }
  | { projectPathMissing: { path: string } };

export interface Comment {
  id: string;
  ticketId: string;
  authorType: 'user' | 'agent' | 'system';
  bodyMd: string;
  createdAt: Date;
  metadata?: Record<string, unknown>;
}

export type AgentType = string;

/** A model option supported by a specific agent. */
export interface AgentModelOption {
  value: string;
  label: string;
}

/** Agent metadata returned by the backend `get_available_agents` command. */
export interface AgentInfo {
  id: string;
  displayName: string;
  isAvailable: boolean;
  version: string | null;
  brandColor: string | null;
  availableModels: AgentModelOption[];
}
export type RunStatus = 'queued' | 'running' | 'finished' | 'error' | 'aborted' | 'paused';

export interface AgentRun {
  id: string;
  ticketId: string;
  agentType: AgentType;
  repoPath: string;
  status: RunStatus;
  startedAt: Date;
  endedAt?: Date;
  exitCode?: number;
  summaryMd?: string;
  metadata?: Record<string, unknown>;
  /** For sub-runs: the parent run ID */
  parentRunId?: string;
  /** For sub-runs: the stage name (e.g., "branch", "plan", "implement", "deslop") */
  stage?: string;
  /** For resumed runs: the ID of the run this is resuming from */
  resumedFromRunId?: string;
}

/** An agent run with additional context for display (board, project, ticket info) */
export interface AgentRunWithContext extends AgentRun {
  /** The ticket title */
  ticketTitle?: string;
  /** The board ID this run's ticket belongs to */
  boardId?: string;
  /** The board name */
  boardName?: string;
  /** The project ID (if the ticket has one) */
  projectId?: string;
  /** The project name (if the ticket has one) */
  projectName?: string;
  /** The current stage name for multi-stage workflows (if running) */
  currentStage?: string;
  /** Number of completed stages (sub-runs with status = finished) */
  completedStages: number;
  /** Total number of stages (all sub-runs) */
  totalStages: number;
}

export interface AgentEvent {
  id: string;
  runId: string;
  ticketId: string;
  eventType: string;
  payload: Record<string, unknown>;
  createdAt: Date;
}

export type Priority = 'low' | 'medium' | 'high' | 'urgent';

export interface CreateTicketInput {
  title: string;
  descriptionMd: string;
  priority: Priority;
  labels: string[];
  columnId: string;
  projectId?: string;
  workspaceId?: string;
  workflowType?: WorkflowType;
  model?: string;
  /** Optional pre-defined branch name (if not provided, will be AI-generated on first run) */
  branchName?: string;
  /** Whether to create this ticket as an epic */
  isEpic?: boolean;
  /** The parent epic ID (when creating a child ticket) */
  epicId?: string;
}

// Worker types
export type WorkerState = 'idle' | 'running' | 'stopped';

export interface WorkerStatus {
  id: string;
  agentType: AgentType;
  projectId?: string;
  status: WorkerState;
  currentTicketId?: string;
  currentRunId?: string;
  ticketsProcessed: number;
  startedAt: Date;
  lastPollAt?: Date;
}

export interface WorkerQueueStatus {
  readyCount: number;
  inProgressCount: number;
  workerCount: number;
}

// Task Queue System types

export type TaskStatus = 'pending' | 'in_progress' | 'completed' | 'failed';

export interface Task {
  id: string;
  ticketId: string;
  orderIndex: number;
  taskType: string;
  title?: string;
  content?: string;
  status: TaskStatus;
  runId?: string;
  createdAt: Date;
  startedAt?: Date;
  completedAt?: Date;
}

export interface TaskCounts {
  pending: number;
  inProgress: number;
  completed: number;
  failed: number;
}

/** Extract the command ID from a task type string (e.g. "command:fix-lint" -> "fix-lint") */
export function getCommandId(taskType: string): string | null {
  if (taskType.startsWith('command:')) {
    return taskType.slice('command:'.length);
  }
  return null;
}

/** Get a display label for a task type.
 *  Handles both DB format ("command:fix-lint") and serde/IPC format ("fix-lint"). */
export function getTaskTypeLabel(taskType: string): string {
  if (taskType === 'custom') return 'Custom';
  const id = getCommandId(taskType) ?? taskType;
  return id
    .split('-')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ');
}

// Epic types

/** Progress information for an epic's children */
export interface EpicProgress {
  /** Total number of child tickets */
  total: number;
  /** Children in Backlog */
  backlog: number;
  /** Children in Ready */
  ready: number;
  /** Children in In Progress */
  inProgress: number;
  /** Children in Blocked */
  blocked: number;
  /** Children in Review */
  review: number;
  /** Children in Done */
  done: number;
}

/** Status of a spec version in the planning workflow */
export type SpecVersionStatus = 
  | 'conversing'  // In spec discovery conversation (default for new versions)
  | 'exploring'
  | 'planning'
  | 'awaiting_approval'
  | 'approved'
  | 'executing'
  | 'executed'  // Epics/tickets created, ready to start work
  | 'working'   // Work in progress
  | 'paused'    // Work paused (can be resumed)
  | 'halted'    // Work halted (can be restarted from beginning)
  | 'completed'
  | 'failed';

/** Status of a single ticket within an epic */
export interface SpecTicketStatus {
  id: string;
  title: string;
  column: string;
}

/** Status of a single epic within a spec */
export interface SpecEpicStatus {
  id: string;
  title: string;
  column: string;
  /** The epics this one depends on (empty = independent/root epic) */
  dependsOnIds: string[];
  /** Titles of the dependency epics (for display, in same order as dependsOnIds) */
  dependsOnTitles: string[];
  /** Child tickets in this epic */
  tickets: SpecTicketStatus[];
}

/** Progress stats for a spec's epics */
export interface SpecProgress {
  /** Number of epics */
  total: number;
  /** Epics in Done column */
  done: number;
  /** Epics in Ready/In Progress/Review */
  inProgress: number;
  /** Epics in Blocked column */
  blocked: number;
  /** Total number of all tickets (epics + child tickets) */
  totalTickets: number;
  /** List of epics with their status */
  epics: SpecEpicStatus[];
}

/** A single exploration query and its result */
export interface Exploration {
  query: string;
  response: string;
  timestamp: Date;
}

/** A spec for the planning agent (top-level entity with shared conversation) */
export interface Spec {
  id: string;
  /** The board this spec belongs to (for organization) */
  boardId: string;
  /** The board where tickets will be created (defaults to boardId if not set) */
  targetBoardId?: string;
  /** The project this spec is scoped to (required) */
  projectId: string;
  name: string;
  userInput: string;
  /** Preferred model for the agent */
  model?: string;
  /** Settings for this spec (auto_approve, etc.) */
  settings: Record<string, unknown>;
  createdAt: Date;
  updatedAt: Date;
}

/** A version of a spec (contains versioned exploration/plan data) */
export interface SpecVersion {
  id: string;
  specId: string;
  versionNumber: number;
  status: SpecVersionStatus;
  /** Log of exploration queries and responses */
  explorationLog: Exploration[];
  /** Generated plan in markdown format (for display) */
  planMarkdown?: string;
  /** Parsed plan structure (for execution) */
  planJson?: ProjectPlan;
  /** When work phase was started (for ETA calculation) */
  workStartedAt?: Date;
  createdAt: Date;
  updatedAt: Date;
}

/** Spec with its latest version (convenience type for API responses) */
export interface SpecWithVersion extends Spec {
  /** The latest version of this spec */
  latestVersion?: SpecVersion;
  /** Total number of versions */
  versionCount: number;
}

/** Confidence level for ETA estimates */
export type EtaConfidence = 'low' | 'medium' | 'high';

/** ETA calculation result for a spec */
export interface SpecEta {
  specId: string;
  /** When work phase was started */
  workStartedAt?: Date;
  /** Total number of tickets */
  totalTickets: number;
  /** Completed tickets */
  completedTickets: number;
  /** Currently in-progress tickets */
  inProgressTickets: number;
  /** Paused tickets */
  pausedTickets: number;
  /** Time elapsed since work started (seconds) */
  elapsedSeconds: number;
  /** Average seconds per completed ticket */
  avgSecondsPerTicket?: number;
  /** Average seconds per stage (for completed stages) */
  avgSecondsPerStage: Record<string, number>;
  /** Estimated seconds remaining */
  estimatedSecondsRemaining?: number;
  /** Estimated completion time (ISO 8601) */
  estimatedCompletionTime?: Date;
  /** Confidence level based on sample size */
  confidence: EtaConfidence;
}

export interface CreateSpecInput {
  boardId: string;
  /** The board where tickets will be created (defaults to boardId if not set) */
  targetBoardId?: string;
  /** The project this spec is scoped to (required) */
  projectId: string;
  name: string;
  userInput: string;
  /** Preferred model */
  model?: string;
  /** Optional settings (e.g. agentType for spec discovery agent selection) */
  settings?: Record<string, unknown>;
}

export interface UpdateSpecInput {
  name?: string;
  userInput?: string;
  model?: string;
}

/** An epic in a generated plan */
export interface PlanEpic {
  title: string;
  description: string;
  /** 
   * Titles of epics this depends on (empty array = root epic, no dependencies)
   * Supports both old format (string | null) and new format (string[]) for backward compatibility
   */
  dependsOn: string[] | string | null;
  tickets: PlanTicket[];
}

/** A ticket in a generated plan */
export interface PlanTicket {
  title: string;
  description: string;
  acceptanceCriteria?: string[];
  /** Branch name assigned at planning time (skips AI generation at work time if set) */
  branchName?: string;
}

/** A generated project plan */
export interface ProjectPlan {
  overview: string;
  epics: PlanEpic[];
}

export interface RunCostData {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  totalCostUsd: number;
  modelUsage: Record<string, ModelCostData>;
  isEstimated: boolean;
}

export interface ModelCostData {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  costUsd: number;
}

export interface AggregatedCost {
  totalCostUsd: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCacheCreationTokens: number;
  runCount: number;
  estimatedCount: number;
  modelTotals: Record<string, ModelCostData>;
}

// Release notes types

export interface ReleaseNoteCategory {
  category: string;
  items: string[];
}

export interface PreviousVersionHighlight {
  version: string;
  highlight: string;
}

export interface ReleaseNote {
  version: string;
  publishedAt: string;
  summary: string | null;
  notes: ReleaseNoteCategory[];
  previousVersions?: PreviousVersionHighlight[] | null;
}

export interface DiffLine {
  lineType: 'add' | 'delete' | 'context';
  content: string;
  oldLineNum?: number;
  newLineNum?: number;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface FileDiff {
  path: string;
  status: string;
  additions: number;
  deletions: number;
  hunks: DiffHunk[];
}

export interface ProjectBranchStatus {
  projectId: string;
  projectName: string;
  branch: string;
  workingDir: string;
  hasChanges: boolean;
  hasUnpushed: boolean;
  hasUncommitted: boolean;
  filesChanged: number;
  additions: number;
  deletions: number;
}

export interface ProjectFileDiffs {
  projectId: string;
  projectName: string;
  files: FileDiff[];
}

export interface ProjectPushResult {
  projectId: string;
  projectName: string;
  success: boolean;
  message: string;
  branch: string;
}

export interface WorkspacePushResult {
  results: ProjectPushResult[];
}

// Dashboard types

export interface DashboardSummary {
  ticketsCompleted: number;
  tasksCompleted: number;
  totalRuns: number;
  successfulRuns: number;
  successRate: number;
  avgRunDurationSecs: number;
  totalCostUsd: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCacheReadTokens: number;
  totalCommits: number;
  totalPrs: number;
  totalLinesAdded: number;
  totalLinesRemoved: number;
  avgCycleTimeHours: number;
}

export interface DashboardTrendPoint {
  date: string;
  ticketsCompleted: number;
  tasksCompleted: number;
  costUsd: number;
  tokensUsed: number;
  runs: number;
  commits: number;
  linesAdded: number;
  linesRemoved: number;
}

export interface ModelBreakdownEntry {
  model: string;
  costUsd: number;
  inputTokens: number;
  outputTokens: number;
  runCount: number;
}

export interface AgentBreakdownEntry {
  agentType: string;
  runCount: number;
  successCount: number;
  avgDurationSecs: number;
}

// Chat types

export type ChatMode = 'general' | 'spec_builder' | 'ticket_builder' | 'review';
export type ChatStatus = 'active' | 'thinking' | 'completed' | 'error';
export type ChatRunStatus = 'running' | 'finished' | 'error';
export type ChatMessageRole = 'user' | 'assistant' | 'system';

export interface Chat {
  id: string;
  title?: string;
  agentType: string;
  projectId?: string;
  workspaceId?: string;
  mode: ChatMode;
  boardId?: string;
  ticketId?: string;
  specId?: string;
  model?: string;
  status: ChatStatus;
  createdAt: Date;
  updatedAt: Date;
}

export interface CreateChat {
  agentType: string;
  projectId?: string;
  workspaceId?: string;
  mode: ChatMode;
  boardId?: string;
  ticketId?: string;
  specId?: string;
  model?: string;
}

export interface ChatMessage {
  id: string;
  chatId: string;
  role: ChatMessageRole;
  content: string;
  metadata?: Record<string, unknown>;
  createdAt: Date;
}

export interface ChatEvent {
  id: string;
  chatId: string;
  messageId?: string;
  eventType: string;
  payload: Record<string, unknown>;
  createdAt: Date;
}

export interface ChatRun {
  id: string;
  chatId: string;
  chatMessageId?: string;
  agentType: string;
  status: ChatRunStatus;
  metadata?: Record<string, unknown>;
  createdAt: Date;
  updatedAt: Date;
}

