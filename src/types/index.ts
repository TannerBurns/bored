export interface Project {
  id: string;
  name: string;
  path: string;
  
  // Hook status
  cursorHooksInstalled: boolean;
  claudeHooksInstalled: boolean;
  
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
  defaultProjectId?: string;
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

export interface Ticket {
  id: string;
  boardId: string;
  columnId: string;
  title: string;
  descriptionMd: string;
  priority: 'low' | 'medium' | 'high' | 'urgent';
  labels: string[];
  createdAt: Date;
  updatedAt: Date;
  lockedByRunId?: string;
  lockExpiresAt?: Date;
  projectId?: string;
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
  pausedAt?: Date;
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

export type AgentType = 'cursor' | 'claude';
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
  ticketTitle: string;
  /** The board ID this run's ticket belongs to */
  boardId: string;
  /** The board name */
  boardName: string;
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

export type TaskType = 'custom' | 'sync_with_main' | 'add_tests' | 'review_polish' | 'fix_lint';
export type TaskStatus = 'pending' | 'in_progress' | 'completed' | 'failed';

export interface Task {
  id: string;
  ticketId: string;
  orderIndex: number;
  taskType: TaskType;
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

export interface PresetTaskInfo {
  typeName: string;
  displayName: string;
  description: string;
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
  | 'conversing'  // In brainstorming conversation (default for new versions)
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
}

/** A generated project plan */
export interface ProjectPlan {
  overview: string;
  epics: PlanEpic[];
}

export type ConversationRole = 'user' | 'assistant' | 'system';

export interface ConversationMessage {
  id: string;
  specId: string;
  role: ConversationRole;
  content: string;
  createdAt: Date;
}

