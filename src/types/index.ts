export interface Project {
  id: string;
  name: string;
  path: string;
  
  // Hook status
  cursorHooksInstalled: boolean;
  claudeHooksInstalled: boolean;
  
  // Preferences
  preferredAgent?: 'cursor' | 'claude' | 'any';
  
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
  preferredAgent?: 'cursor' | 'claude' | 'any';
}

export interface UpdateProjectInput {
  name?: string;
  preferredAgent?: 'cursor' | 'claude' | 'any';
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
  agentPref?: 'cursor' | 'claude' | 'any';
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
  /** Link back to scratchpad that created this ticket */
  scratchpadId?: string;
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
export type RunStatus = 'queued' | 'running' | 'finished' | 'error' | 'aborted';

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
}

export interface AgentEvent {
  id: string;
  runId: string;
  ticketId: string;
  eventType: string;
  payload: Record<string, unknown>;
  createdAt: Date;
}

export interface NormalizedEvent {
  runId: string;
  ticketId: string;
  agentType: AgentType;
  eventType: 
    | 'command_requested'
    | 'command_executed'
    | 'file_read'
    | 'file_edited'
    | 'run_started'
    | 'run_stopped'
    | 'error';
  payload: {
    raw?: string;
    structured?: Record<string, unknown>;
  };
  timestamp: Date;
}

export type Priority = 'low' | 'medium' | 'high' | 'urgent';

export interface CreateTicketInput {
  title: string;
  descriptionMd: string;
  priority: Priority;
  labels: string[];
  columnId: string;
  projectId?: string;
  agentPref?: 'cursor' | 'claude' | 'any';
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

// Validation types
export interface ValidationCheck {
  name: string;
  passed: boolean;
  message: string;
  fixAction?: string;
  isWarning?: boolean;
}

export interface ValidationResult {
  valid: boolean;
  checks: ValidationCheck[];
  errors: string[];
  warnings: string[];
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

export interface CreateTaskInput {
  ticketId: string;
  title?: string;
  content?: string;
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

// ===== Scratchpad / Planner Types =====

export type ScratchpadStatus = 
  | 'draft'
  | 'exploring'
  | 'planning'
  | 'awaiting_approval'
  | 'approved'
  | 'executing'
  | 'completed'
  | 'failed';

/** A single exploration query and its result */
export interface Exploration {
  query: string;
  response: string;
  timestamp: Date;
}

/** A scratchpad for the planner agent */
export interface Scratchpad {
  id: string;
  boardId: string;
  /** The project this scratchpad is scoped to (required) */
  projectId: string;
  name: string;
  userInput: string;
  status: ScratchpadStatus;
  /** Preferred agent type for executing the plan */
  agentPref?: 'cursor' | 'claude' | 'any';
  /** Preferred model for the agent */
  model?: string;
  /** Log of exploration queries and responses */
  explorationLog: Exploration[];
  /** Generated plan in markdown format (for display) */
  planMarkdown?: string;
  /** Parsed plan structure (for execution) */
  planJson?: ProjectPlan;
  /** Settings for this scratchpad (auto_approve, etc.) */
  settings: Record<string, unknown>;
  createdAt: Date;
  updatedAt: Date;
}

export interface CreateScratchpadInput {
  boardId: string;
  /** The project this scratchpad is scoped to (required) */
  projectId: string;
  name: string;
  userInput: string;
  /** Preferred agent type */
  agentPref?: 'cursor' | 'claude' | 'any';
  /** Preferred model */
  model?: string;
}

export interface UpdateScratchpadInput {
  name?: string;
  userInput?: string;
  agentPref?: 'cursor' | 'claude' | 'any';
  model?: string;
}

/** An epic in a generated plan */
export interface PlanEpic {
  title: string;
  description: string;
  /** Title of epic this depends on (null for first epic) */
  dependsOn?: string;
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
