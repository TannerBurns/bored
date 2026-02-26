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
  rawJson: string;
  isStderr: boolean;
}
