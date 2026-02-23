import { invoke } from '@tauri-apps/api/core';
import type {
  Board,
  Column,
  Ticket,
  AgentRun,
  AgentRunWithContext,
  AgentInfo,
  Project,
  CreateProjectInput,
  UpdateProjectInput,
  ReadinessCheck,
  AggregatedCost,
} from '../types';

// API configuration
export interface ApiConfig {
  url: string;
  port: number;
  token: string;
}

export async function getApiConfig(): Promise<ApiConfig> {
  return invoke('get_api_config');
}

export async function setNotificationsEnabled(enabled: boolean): Promise<void> {
  return invoke('set_notifications_enabled', { enabled });
}

// Agent registry
export async function getAvailableAgents(): Promise<AgentInfo[]> {
  return invoke('get_available_agents');
}

export async function getProjects(): Promise<Project[]> {
  return invoke('get_projects');
}

export async function getProject(projectId: string): Promise<Project | null> {
  return invoke('get_project', { projectId });
}

export async function createProject(input: CreateProjectInput): Promise<Project> {
  return invoke('create_project', { input });
}

export async function updateProject(
  projectId: string,
  input: UpdateProjectInput
): Promise<void> {
  return invoke('update_project', { projectId, input });
}

export async function deleteProject(projectId: string): Promise<void> {
  return invoke('delete_project', { projectId });
}

export async function checkTicketReadiness(
  ticketId: string
): Promise<ReadinessCheck> {
  return invoke('check_ticket_readiness', { ticketId });
}

export async function browseForDirectory(): Promise<string | null> {
  return invoke('browse_for_directory');
}

export async function checkGitStatus(path: string): Promise<boolean> {
  return invoke('check_git_status', { path });
}

export async function initGitRepo(path: string): Promise<void> {
  return invoke('init_git_repo', { path });
}

export async function createProjectFolder(
  parentPath: string,
  name: string
): Promise<string> {
  return invoke('create_project_folder', { parentPath, name });
}

export async function getBoards(): Promise<Board[]> {
  return invoke('get_boards');
}

export async function getColumns(boardId: string): Promise<Column[]> {
  return invoke('get_columns', { boardId });
}

export async function createBoard(name: string): Promise<Board> {
  return invoke('create_board', { name });
}

export async function getTickets(boardId: string): Promise<Ticket[]> {
  return invoke('get_tickets', { boardId });
}

export async function getTicket(ticketId: string): Promise<Ticket> {
  return invoke('get_ticket', { ticketId });
}

export async function createTicket(
  ticket: Omit<Ticket, 'id' | 'createdAt' | 'updatedAt'>
): Promise<Ticket> {
  return invoke('create_ticket', { ticket });
}

export async function moveTicket(
  ticketId: string,
  columnId: string
): Promise<void> {
  return invoke('move_ticket', { ticketId, columnId });
}

export async function deleteTicket(ticketId: string): Promise<void> {
  return invoke('delete_ticket', { ticketId });
}

export async function startAgentRun(
  ticketId: string,
  agentType: string,
  repoPath: string,
  options?: {
    codeReviewMaxIterations?: number;
    stageTimeoutHours?: number;
    stageMaxRetries?: number;
    stageConfigs?: Record<string, { enabled: boolean; model: string }>;
  }
): Promise<string> {
  return invoke('start_agent_run', { 
    input: {
      ticketId, 
      agentType, 
      repoPath, 
      codeReviewMaxIterations: options?.codeReviewMaxIterations,
      stageTimeoutHours: options?.stageTimeoutHours,
      stageMaxRetries: options?.stageMaxRetries,
      stageConfigs: options?.stageConfigs,
    },
  });
}

export async function getAgentRuns(ticketId: string): Promise<AgentRun[]> {
  return invoke('get_agent_runs', { ticketId });
}

/** Get recent runs with full context (board, project, ticket info).
 * This is the preferred method for the runs list as it eliminates
 * client-side lookups and works across all boards.
 */
export async function getRecentRunsWithContext(limit?: number): Promise<AgentRunWithContext[]> {
  return invoke('get_recent_runs_with_context', { limit });
}

export async function cancelAgentRun(runId: string): Promise<void> {
  return invoke('cancel_agent_run', { runId });
}

export async function cleanupStaleRuns(): Promise<number> {
  return invoke('cleanup_stale_runs');
}

export async function getAgentRun(runId: string): Promise<AgentRun> {
  return invoke('get_agent_run', { runId });
}

export interface AgentEvent {
  id: string;
  runId: string;
  ticketId: string;
  eventType: string;
  payload: {
    raw?: string;
    structured?: Record<string, unknown>;
  };
  createdAt: string;
}

export async function getRunEvents(runId: string): Promise<AgentEvent[]> {
  return invoke('get_run_events', { runId });
}

// Unified agent integration
export interface AgentStatus {
  isAvailable: boolean;
  version: string | null;
}

export async function getAgentStatus(agentId: string): Promise<AgentStatus> {
  return invoke('get_agent_status', { agentId });
}

export async function checkAgentAvailable(agentId: string): Promise<boolean> {
  return invoke('check_agent_available', { agentId });
}

export async function getAgentSettings(agentId: string): Promise<Record<string, unknown>> {
  return invoke('get_agent_settings', { agentId });
}

export async function setAgentSettings(agentId: string, settings: Record<string, unknown>): Promise<void> {
  return invoke('set_agent_settings', { agentId, settings });
}

export async function getCommandsPath(): Promise<string | null> {
  return invoke('get_commands_path');
}

export async function getAvailableCommands(): Promise<string[]> {
  return invoke('get_available_commands');
}

export async function installCommandsToProject(
  agentType: string,
  repoPath: string
): Promise<string[]> {
  return invoke('install_commands_to_project', { agentType, repoPath });
}

export async function installCommandsToUser(agentType: string): Promise<string[]> {
  return invoke('install_commands_to_user', { agentType });
}

export async function checkCommandsInstalled(
  agentType: string,
  repoPath: string
): Promise<boolean> {
  return invoke('check_commands_installed', { agentType, repoPath });
}

export async function checkUserCommandsInstalled(agentType: string): Promise<boolean> {
  return invoke('check_user_commands_installed', { agentType });
}

export async function readCommandContent(filename: string): Promise<string> {
  return invoke('read_command_content', { filename });
}

export async function saveCustomCommand(
  id: string,
  filename: string,
  content: string
): Promise<void> {
  return invoke('save_custom_command', { id, filename, content });
}

export async function deleteCustomCommand(filename: string): Promise<void> {
  return invoke('delete_custom_command', { filename });
}

export async function installCatalogCommandsToAllProjects(filenames: string[], removeFilenames: string[]): Promise<void> {
  return invoke('install_catalog_commands_to_all_projects', { filenames, removeFilenames });
}

// Factory reset - clears all data from the database
export async function factoryReset(): Promise<void> {
  return invoke('factory_reset');
}

// Repair specs table - fixes CHECK constraint issue
export async function repairSpecsTable(): Promise<string> {
  return invoke('repair_specs_table');
}

// Conversation (brainstorming) functions
import type { ConversationMessage } from '../types';

export async function getConversationMessages(specId: string): Promise<ConversationMessage[]> {
  return invoke('get_conversation_messages', { specId });
}

export async function sendConversationMessage(
  specId: string,
  content: string,
  timeoutMinutes?: number,
  agentType?: string
): Promise<ConversationMessage> {
  return invoke('send_conversation_message', {
    input: {
      specId,
      content,
      timeoutMinutes,
      agentType: agentType ?? null,
    },
  });
}

export async function startConversation(
  specId: string,
  timeoutMinutes?: number,
  agentType?: string
): Promise<ConversationMessage> {
  return invoke('start_conversation', {
    input: {
      specId,
      timeoutMinutes,
      agentType: agentType ?? null,
    },
  });
}

export async function getTicketCost(ticketId: string): Promise<AggregatedCost> {
  return invoke('get_ticket_cost', { ticketId });
}

export async function backfillRunCosts(): Promise<number> {
  return invoke('backfill_run_costs');
}

export async function getSpecCost(specId: string): Promise<AggregatedCost> {
  return invoke('get_spec_cost', { specId });
}

// Release notes
import type { ReleaseNote } from '../types';

export async function getReleaseNotes(version: string): Promise<ReleaseNote | null> {
  return invoke('get_release_notes', { version });
}

export async function getAllReleaseNotes(): Promise<ReleaseNote[]> {
  return invoke('get_all_release_notes');
}

// Validation functions
import type {
  ValidationSession,
  ValidationMessage,
  FixTask,
  PushResult,
  PullRequestResult,
  BranchDiff,
  FileDiff,
} from '../types';

export async function createValidationSession(input: {
  ticketId: string;
  projectId?: string;
  agentType?: string;
}): Promise<ValidationSession> {
  return invoke('create_validation_session', { input });
}

export async function getValidationSession(
  sessionId: string
): Promise<ValidationSession> {
  return invoke('get_validation_session', { sessionId });
}

export async function getValidationSessions(
  ticketId: string
): Promise<ValidationSession[]> {
  return invoke('get_validation_sessions', { ticketId });
}

export async function deleteValidationSession(
  sessionId: string
): Promise<void> {
  return invoke('delete_validation_session', { sessionId });
}

export async function getValidationMessages(
  sessionId: string
): Promise<ValidationMessage[]> {
  return invoke('get_validation_messages', { sessionId });
}

export async function sendValidationMessage(
  sessionId: string,
  content: string,
  options?: { model?: string; timeoutMinutes?: number }
): Promise<ValidationMessage> {
  return invoke('send_validation_message', {
    request: {
      sessionId,
      content,
      options: options
        ? { model: options.model ?? null, timeoutMinutes: options.timeoutMinutes ?? null }
        : null,
    },
  });
}

export async function stopValidationApp(sessionId: string): Promise<void> {
  return invoke('stop_validation_app', { sessionId });
}

export interface ValidationAppStatus {
  running: boolean;
}

export async function getValidationAppStatus(
  sessionId: string
): Promise<ValidationAppStatus> {
  return invoke('get_validation_app_status', { sessionId });
}

export async function createFixTasks(input: {
  sessionId: string;
  ticketId: string;
  tasks: FixTask[];
}): Promise<string[]> {
  return invoke('create_fix_tasks', { input });
}

// Next steps functions
export async function pushBranch(ticketId: string): Promise<PushResult> {
  return invoke('push_branch', { ticketId });
}

export async function createPullRequest(
  ticketId: string,
  title?: string,
  body?: string
): Promise<PullRequestResult> {
  return invoke('create_pull_request', { ticketId, title, body });
}

export async function getBranchDiff(ticketId: string): Promise<BranchDiff> {
  return invoke('get_branch_diff', { ticketId });
}

export async function getBranchDiffFiles(ticketId: string): Promise<FileDiff[]> {
  return invoke('get_branch_diff_files', { ticketId });
}

// Cursor model list from CLI
export interface CursorModelInfo {
  id: string;
  label: string;
  isDefault: boolean;
  isCurrent: boolean;
}
export interface CursorModelList {
  models: CursorModelInfo[];
  currentModel: string | null;
  defaultModel: string | null;
}
export async function listCursorModels(): Promise<CursorModelList> {
  return invoke('list_cursor_models');
}

// Dashboard
import type {
  DashboardSummary,
  DashboardTrendPoint,
  ModelBreakdownEntry,
  AgentBreakdownEntry,
} from '../types';

export async function getDashboardSummary(days?: number): Promise<DashboardSummary> {
  return invoke('get_dashboard_summary', { days: days ?? null });
}

export async function getDashboardTrends(days: number): Promise<DashboardTrendPoint[]> {
  return invoke('get_dashboard_trends', { days });
}

export async function getModelBreakdown(days?: number): Promise<ModelBreakdownEntry[]> {
  return invoke('get_model_breakdown', { days: days ?? null });
}

export async function getAgentBreakdown(days?: number): Promise<AgentBreakdownEntry[]> {
  return invoke('get_agent_breakdown', { days: days ?? null });
}

export async function backfillGitStats(): Promise<number> {
  return invoke('backfill_git_stats');
}

// Per-agent workflow settings sync
export async function syncAgentConfigs(agentConfigs: Record<string, {
  autoPilotEnabled: boolean;
  stageConfigs: Record<string, { enabled: boolean; model: string }>;
  codeReviewMaxIterations: number;
  stageTimeoutHours: number;
  stageMaxRetries: number;
  diagnosticModel: string;
}>): Promise<void> {
  return invoke('sync_agent_configs', { agentConfigs });
}
