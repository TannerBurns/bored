import { invoke } from '@tauri-apps/api/core';
import type {
  Board,
  Column,
  Ticket,
  AgentRun,
  AgentRunWithContext,
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

export async function setBoardProject(
  boardId: string,
  projectId: string | null
): Promise<void> {
  return invoke('set_board_project', { boardId, projectId });
}

export async function setTicketProject(
  ticketId: string,
  projectId: string | null
): Promise<void> {
  return invoke('set_ticket_project', { ticketId, projectId });
}

export async function checkTicketReadiness(
  ticketId: string
): Promise<ReadinessCheck> {
  return invoke('check_ticket_readiness', { ticketId });
}

export async function updateProjectHooks(
  projectId: string,
  cursorInstalled?: boolean,
  claudeInstalled?: boolean
): Promise<void> {
  return invoke('update_project_hooks', {
    projectId,
    cursorInstalled,
    claudeInstalled,
  });
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
  agentType: 'cursor' | 'claude',
  repoPath: string,
  options?: {
    codeReviewMaxIterations?: number;
    stageTimeoutMinutes?: number;
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
      stageTimeoutMinutes: options?.stageTimeoutMinutes,
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

// Cursor integration
export interface CursorStatus {
  isAvailable: boolean;
  version: string | null;
  globalHooksInstalled: boolean;
  hookScriptPath: string | null;
}

export async function getCursorStatus(): Promise<CursorStatus> {
  return invoke('get_cursor_status');
}

export async function installCursorHooksGlobal(
  hookScriptPath: string,
  apiUrl?: string,
  apiToken?: string
): Promise<void> {
  return invoke('install_cursor_hooks_global', { hookScriptPath, apiUrl, apiToken });
}

export async function installCursorHooksProject(
  hookScriptPath: string,
  projectPath: string,
  apiUrl?: string,
  apiToken?: string
): Promise<void> {
  return invoke('install_cursor_hooks_project', { hookScriptPath, projectPath, apiUrl, apiToken });
}

export async function getCursorHooksConfig(
  hookScriptPath: string
): Promise<string> {
  return invoke('get_cursor_hooks_config', { hookScriptPath });
}

export async function checkProjectHooksInstalled(
  projectPath: string
): Promise<boolean> {
  return invoke('check_project_hooks_installed', { projectPath });
}

export async function getHookScriptPath(): Promise<string | null> {
  return invoke('get_hook_script_path_cmd');
}

// Claude Code integration
export interface ClaudeStatus {
  isAvailable: boolean;
  version: string | null;
  userHooksInstalled: boolean;
  hookScriptPath: string | null;
}

export async function getClaudeStatus(): Promise<ClaudeStatus> {
  return invoke('get_claude_status');
}

export async function installClaudeHooksUser(
  hookScriptPath: string,
  apiUrl?: string,
  apiToken?: string
): Promise<void> {
  return invoke('install_claude_hooks_user', { hookScriptPath, apiUrl, apiToken });
}

export async function installClaudeHooksProject(
  hookScriptPath: string,
  projectPath: string,
  apiUrl?: string,
  apiToken?: string
): Promise<void> {
  return invoke('install_claude_hooks_project', { hookScriptPath, projectPath, apiUrl, apiToken });
}

export async function installClaudeHooksLocal(
  hookScriptPath: string,
  projectPath: string,
  apiUrl?: string,
  apiToken?: string
): Promise<void> {
  return invoke('install_claude_hooks_local', { hookScriptPath, projectPath, apiUrl, apiToken });
}

export async function getClaudeHooksConfig(
  hookScriptPath: string
): Promise<string> {
  return invoke('get_claude_hooks_config', { hookScriptPath });
}

export async function checkClaudeAvailable(): Promise<boolean> {
  return invoke('check_claude_available');
}

export async function checkClaudeProjectHooksInstalled(
  projectPath: string
): Promise<boolean> {
  return invoke('check_claude_project_hooks_installed', { projectPath });
}

export async function getClaudeHookScriptPath(): Promise<string | null> {
  return invoke('get_claude_hook_script_path');
}

// Claude API Settings
export interface ClaudeApiSettings {
  authToken: string | null;
  apiKey: string | null;
  baseUrl: string | null;
  modelOverride: string | null;
}

export async function getClaudeApiSettings(): Promise<ClaudeApiSettings> {
  return invoke('get_claude_api_settings');
}

export async function setClaudeApiSettings(settings: ClaudeApiSettings): Promise<void> {
  return invoke('set_claude_api_settings', { settings });
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
  agentType?: 'cursor' | 'claude'
): Promise<ConversationMessage> {
  return invoke('send_conversation_message', {
    specId,
    content,
    timeoutMinutes,
    agentType: agentType ?? null,
  });
}

export async function startConversation(
  specId: string,
  timeoutMinutes?: number,
  agentType?: 'cursor' | 'claude'
): Promise<ConversationMessage> {
  return invoke('start_conversation', {
    specId,
    timeoutMinutes,
    agentType: agentType ?? null,
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
  agentType?: 'cursor' | 'claude';
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

export async function updateValidationSessionStatus(
  sessionId: string,
  status: string
): Promise<void> {
  return invoke('update_validation_session_status', { sessionId, status });
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

// Workflow settings sync
export async function syncWorkflowSettings(settings: {
  stageConfigs: Record<string, { enabled: boolean; model: string }>;
  codeReviewMaxIterations: number;
  stageTimeoutMinutes: number;
  stageMaxRetries: number;
}): Promise<void> {
  return invoke('sync_workflow_settings', { settings });
}
