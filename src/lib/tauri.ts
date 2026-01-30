import { invoke } from '@tauri-apps/api/tauri';
import type {
  Board,
  Column,
  Ticket,
  AgentRun,
  Project,
  CreateProjectInput,
  UpdateProjectInput,
  ReadinessCheck,
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
  repoPath: string
): Promise<string> {
  // Backend returns just the run ID as a string
  return invoke('start_agent_run', { ticketId, agentType, repoPath });
}

export async function getAgentRuns(ticketId: string): Promise<AgentRun[]> {
  return invoke('get_agent_runs', { ticketId });
}

export async function getRecentRuns(limit?: number): Promise<AgentRun[]> {
  return invoke('get_recent_runs', { limit });
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

// Worker validation and commands
import type { ValidationResult } from '../types';

export async function validateWorker(
  agentType: string,
  repoPath: string
): Promise<ValidationResult> {
  return invoke('validate_worker', { agentType, repoPath });
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
