import type { Ticket, Column, Comment } from '../../types';

export interface AgentLogEvent {
  runId: string;
  stream: 'stdout' | 'stderr';
  content: string;
  timestamp: string;
}

export interface AgentCompleteEvent {
  runId: string;
  status: string;
  exitCode: number | null;
  durationSecs: number;
}

export interface AgentErrorEvent {
  runId: string;
  error: string;
}

export interface TicketCommentAddedEvent {
  ticketId: string;
  comment: string;
}

export interface AgentStageUpdateEvent {
  parentRunId: string;
  stage: string;
  status: string;
  subRunId?: string;
  durationSecs?: number;
}

export interface TicketModalProps {
  ticket: Ticket;
  columns: Column[];
  comments: Comment[];
  onClose: () => void;
  onUpdate: (ticketId: string, updates: Partial<Ticket>) => Promise<void>;
  onAddComment: (ticketId: string, body: string) => Promise<void>;
  onUpdateComment: (commentId: string, body: string) => Promise<void>;
  onRunWithAgent?: (ticketId: string, agentType: 'cursor' | 'claude') => void;
  onDelete?: (ticketId: string) => Promise<void>;
  onAgentComplete?: (runId: string, status: string) => void;
}
