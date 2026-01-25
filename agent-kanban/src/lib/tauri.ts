import { invoke } from '@tauri-apps/api/tauri';
import type { Board, Ticket, AgentRun } from '../types';

export async function getBoards(): Promise<Board[]> {
  return invoke('get_boards');
}

export async function createBoard(name: string): Promise<Board> {
  return invoke('create_board', { name });
}

export async function getTickets(boardId: string): Promise<Ticket[]> {
  return invoke('get_tickets', { boardId });
}

export async function createTicket(ticket: Omit<Ticket, 'id' | 'createdAt' | 'updatedAt'>): Promise<Ticket> {
  return invoke('create_ticket', { ticket });
}

export async function moveTicket(ticketId: string, columnId: string): Promise<void> {
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
