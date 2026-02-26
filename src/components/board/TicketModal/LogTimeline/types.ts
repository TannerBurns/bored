export type TimelineEntryType =
  | 'system'
  | 'assistant'
  | 'tool_use'
  | 'tool_result'
  | 'user'
  | 'result'
  | 'error'
  | 'streaming';

export interface TimelineEntry {
  id: string;
  type: TimelineEntryType;
  timestamp: string;
  summary: string;
  content?: string;
  toolName?: string;
  toolInput?: string;
  costData?: {
    inputTokens: number;
    outputTokens: number;
    totalCostUsd: number;
  };
  /** Whether this event came from a subagent (e.g. Claude Task tool) */
  isSubagent?: boolean;
  /** Model used for this event (useful for distinguishing subagent models) */
  model?: string;
  rawJson: string;
  isStderr: boolean;
}
