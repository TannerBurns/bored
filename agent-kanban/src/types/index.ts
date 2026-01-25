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
}
