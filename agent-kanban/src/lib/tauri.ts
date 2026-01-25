import { invoke } from '@tauri-apps/api/tauri';
import type {
  Board,
  Ticket,
  AgentRun,
  Project,
  CreateProjectInput,
  UpdateProjectInput,
  ReadinessCheck,
} from '../types';

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

export async function getBoards(): Promise<Board[]> {
  return invoke('get_boards');
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

export async function startAgentRun(
  ticketId: string,
  agentType: 'cursor' | 'claude',
  repoPath: string
): Promise<AgentRun> {
  return invoke('start_agent_run', { ticketId, agentType, repoPath });
}

export async function getAgentRuns(ticketId: string): Promise<AgentRun[]> {
  return invoke('get_agent_runs', { ticketId });
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
