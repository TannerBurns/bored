const DEFAULT_API_BASE = 'http://127.0.0.1:7432';

interface ApiClientOptions {
  baseUrl?: string;
  token?: string;
}

class ApiClient {
  private baseUrl: string;
  private token: string = '';

  constructor(options: ApiClientOptions = {}) {
    this.baseUrl = options.baseUrl || DEFAULT_API_BASE;
    if (options.token) {
      this.token = options.token;
    }
  }

  configure(options: { baseUrl?: string; token?: string }) {
    if (options.baseUrl) {
      this.baseUrl = options.baseUrl;
    }
    if (options.token) {
      this.token = options.token;
    }
  }

  setToken(token: string) {
    this.token = token;
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown
  ): Promise<T> {
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
    };

    if (this.token) {
      headers['X-AgentKanban-Token'] = this.token;
    }

    const response = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers,
      body: body ? JSON.stringify(body) : undefined,
    });

    if (!response.ok) {
      const error = await response.json().catch(() => ({
        error: response.statusText,
        code: 'UNKNOWN_ERROR',
      }));
      throw new ApiError(response.status, error.error, error.code);
    }

    const text = await response.text();
    return (text ? JSON.parse(text) : null) as T;
  }

  // Health
  async health(): Promise<{ status: string }> {
    return this.request('GET', '/health/detailed');
  }

  // Boards
  async getBoards() {
    return this.request<Board[]>('GET', '/v1/boards');
  }

  async getBoard(boardId: string) {
    return this.request<BoardWithColumns>('GET', `/v1/boards/${boardId}`);
  }

  // Tickets
  async getTickets(boardId: string, columnId?: string) {
    const query = columnId ? `?column=${columnId}` : '';
    return this.request<Ticket[]>('GET', `/v1/boards/${boardId}/tickets${query}`);
  }

  async createTicket(ticket: CreateTicketRequest) {
    return this.request<Ticket>('POST', '/v1/tickets', ticket);
  }

  async getTicket(ticketId: string) {
    return this.request<Ticket>('GET', `/v1/tickets/${ticketId}`);
  }

  async updateTicket(ticketId: string, updates: UpdateTicketRequest) {
    return this.request<Ticket>('PATCH', `/v1/tickets/${ticketId}`, updates);
  }

  async deleteTicket(ticketId: string) {
    return this.request<{ deleted: boolean }>('DELETE', `/v1/tickets/${ticketId}`);
  }

  async moveTicket(ticketId: string, columnId: string) {
    return this.request<Ticket>('POST', `/v1/tickets/${ticketId}/move`, { columnId });
  }

  async reserveTicket(ticketId: string, agentType: 'cursor' | 'claude', repoPath?: string) {
    return this.request<ReservationResponse>('POST', `/v1/tickets/${ticketId}/reserve`, {
      agentType,
      repoPath,
    });
  }

  // Runs
  async getRun(runId: string) {
    return this.request<AgentRun>('GET', `/v1/runs/${runId}`);
  }

  async updateRun(runId: string, updates: UpdateRunRequest) {
    return this.request<AgentRun>('PATCH', `/v1/runs/${runId}`, updates);
  }

  async heartbeat(runId: string) {
    return this.request<HeartbeatResponse>('POST', `/v1/runs/${runId}/heartbeat`);
  }

  async releaseRun(runId: string) {
    return this.request<AgentRun>('POST', `/v1/runs/${runId}/release`);
  }

  // Events
  async createEvent(runId: string, eventType: string, payload: unknown) {
    return this.request<AgentEvent>('POST', `/v1/runs/${runId}/events`, {
      eventType,
      payload,
      timestamp: new Date().toISOString(),
    });
  }

  async getEvents(runId: string) {
    return this.request<AgentEvent[]>('GET', `/v1/runs/${runId}/events`);
  }

  // Comments
  async getComments(ticketId: string) {
    return this.request<Comment[]>('GET', `/v1/tickets/${ticketId}/comments`);
  }

  async createComment(ticketId: string, bodyMd: string, authorType: string = 'agent') {
    return this.request<Comment>('POST', `/v1/tickets/${ticketId}/comments`, {
      bodyMd,
      authorType,
    });
  }

  // Queue
  async getNextTicket(agentType: 'cursor' | 'claude', repoPath?: string, boardId?: string) {
    return this.request<QueueNextResponse>('POST', '/v1/queue/next', {
      agentType,
      repoPath,
      boardId,
    });
  }

  async getQueueStatus() {
    return this.request<QueueStatusResponse>('GET', '/v1/queue/status');
  }
}

class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public code: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

// Types
interface Board {
  id: string;
  name: string;
  defaultProjectId?: string;
  createdAt: string;
  updatedAt: string;
}

interface BoardWithColumns extends Board {
  columns: Column[];
}

interface Column {
  id: string;
  boardId: string;
  name: string;
  position: number;
  wipLimit?: number;
}

interface Ticket {
  id: string;
  boardId: string;
  columnId: string;
  title: string;
  descriptionMd: string;
  priority: 'low' | 'medium' | 'high' | 'urgent';
  labels: string[];
  createdAt: string;
  updatedAt: string;
  lockedByRunId?: string;
  lockExpiresAt?: string;
  projectId?: string;
  agentPref?: 'cursor' | 'claude' | 'any';
}

interface CreateTicketRequest {
  boardId: string;
  columnId: string;
  title: string;
  descriptionMd?: string;
  priority?: 'low' | 'medium' | 'high' | 'urgent';
  labels?: string[];
  projectId?: string;
  agentPref?: 'cursor' | 'claude' | 'any';
}

interface UpdateTicketRequest {
  title?: string;
  descriptionMd?: string;
  priority?: 'low' | 'medium' | 'high' | 'urgent';
  labels?: string[];
  projectId?: string;
  agentPref?: 'cursor' | 'claude' | 'any';
}

interface AgentRun {
  id: string;
  ticketId: string;
  agentType: 'cursor' | 'claude';
  repoPath: string;
  status: 'queued' | 'running' | 'finished' | 'error' | 'aborted';
  startedAt: string;
  endedAt?: string;
  exitCode?: number;
  summaryMd?: string;
}

interface UpdateRunRequest {
  status?: string;
  exitCode?: number;
  summaryMd?: string;
}

interface ReservationResponse {
  runId: string;
  ticketId: string;
  lockExpiresAt: string;
  heartbeatIntervalSecs: number;
}

interface HeartbeatResponse {
  runId: string;
  lockExpiresAt: string;
  ok: boolean;
}

interface AgentEvent {
  id: string;
  runId: string;
  ticketId: string;
  eventType: string;
  payload: unknown;
  createdAt: string;
}

interface Comment {
  id: string;
  ticketId: string;
  authorType: 'user' | 'agent' | 'system';
  bodyMd: string;
  createdAt: string;
  metadata?: unknown;
}

interface QueueNextResponse {
  ticket: Ticket;
  runId: string;
  lockExpiresAt: string;
  heartbeatIntervalSecs: number;
}

interface QueueStatusResponse {
  readyCount: number;
  inProgressCount: number;
  boards: { boardId: string; boardName: string; readyCount: number }[];
}

export const api = new ApiClient();
export { ApiClient, ApiError };
export type {
  Board,
  BoardWithColumns,
  Column,
  Ticket,
  CreateTicketRequest,
  UpdateTicketRequest,
  AgentRun,
  AgentEvent,
  Comment,
  ReservationResponse,
  QueueNextResponse,
  QueueStatusResponse,
};
