import type { RunEvent } from '../types';
import type { TimelineEntry, TimelineEntryType } from './types';

export function getEventTypeString(eventType: unknown): string {
  if (typeof eventType === 'string') return eventType;
  if (typeof eventType === 'object' && eventType !== null) {
    const obj = eventType as Record<string, unknown>;
    if ('custom' in obj) return String(obj.custom);
    const keys = Object.keys(obj);
    if (keys.length === 1) return String(obj[keys[0]]);
    return JSON.stringify(eventType);
  }
  return String(eventType);
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  return s.slice(0, max) + '...';
}

function extractToolSummary(item: Record<string, unknown>): { toolName: string; toolInput?: string; summary: string } {
  const name = (item.name as string) || 'tool';
  const input = item.input as Record<string, unknown> | undefined;
  const detail =
    input?.file_path as string ??
    input?.path as string ??
    input?.command as string ??
    input?.pattern as string ??
    input?.query as string ??
    undefined;

  const summary = detail ? `${name}: ${truncate(detail, 60)}` : `Using ${name}`;
  return { toolName: name, toolInput: detail, summary };
}

/** Extract tool name and detail from Cursor's tool_call format.
 *  The tool_call object has a single key like "shellToolCall", "readToolCall", etc. */
function extractCursorToolCallSummary(
  toolCallObj: Record<string, unknown>,
): { toolName: string; toolInput?: string; summary: string } {
  const key = Object.keys(toolCallObj)[0] ?? '';
  const toolName = key.replace(/ToolCall$/, '') || 'tool';
  const displayName = toolName.charAt(0).toUpperCase() + toolName.slice(1);

  const inner = toolCallObj[key] as Record<string, unknown> | undefined;
  const args = inner?.args as Record<string, unknown> | undefined;

  const detail =
    args?.path as string ??
    args?.command as string ??
    args?.pattern as string ??
    args?.globPattern as string ??
    args?.query as string ??
    undefined;

  // Strip long worktree prefixes from paths for readability
  const shortDetail = detail?.replace(/^\/(?:private\/)?var\/folders\/.*?\/worktrees\/[^/]+\//, '') ?? detail;

  const summary = shortDetail
    ? `${displayName}: ${truncate(shortDetail, 60)}`
    : `Using ${displayName}`;
  return { toolName: displayName, toolInput: shortDetail, summary };
}

/** Shorten a model ID for display (e.g. "claude-haiku-4-5-20251001" -> "Haiku 4.5") */
function shortModelName(model: string | undefined): string | undefined {
  if (!model) return undefined;
  const m = model.toLowerCase();
  for (const family of ['opus', 'sonnet', 'haiku']) {
    if (m.includes(family)) {
      const label = family.charAt(0).toUpperCase() + family.slice(1);
      const ver = m.match(/(\d+[\d.]*\d)/)?.[1];
      return ver ? `${label} ${ver}` : label;
    }
  }
  return model;
}

function parseClaudeEvent(
  raw: string,
  json: Record<string, unknown>,
  id: string,
  timestamp: string,
  isStderr: boolean,
  taskDescriptions: Map<string, string>,
): TimelineEntry[] {
  const msgType = json.type as string | undefined;
  if (!msgType) return [];

  // Detect main agent vs subagent via parent_tool_use_id
  const parentToolUseId = json.parent_tool_use_id as string | null | undefined;
  const isSubagent = parentToolUseId != null && parentToolUseId !== '';
  const subagentLabel = (isSubagent && parentToolUseId) ? taskDescriptions.get(parentToolUseId) : undefined;
  const msg = json.message as Record<string, unknown> | undefined;
  const rawModel = (msg?.model as string) ?? undefined;
  const model = shortModelName(rawModel);

  // When main agent calls Task, record the subagent type for labeling
  if (!isSubagent && Array.isArray(msg?.content)) {
    for (const block of msg.content as Record<string, unknown>[]) {
      if (block.type === 'tool_use' && block.name === 'Task') {
        const toolId = block.id as string | undefined;
        const input = block.input as Record<string, unknown> | undefined;
        const subagentType = input?.subagent_type as string | undefined;
        const desc = input?.description as string | undefined;
        const label = subagentType ?? desc;
        if (toolId && label) {
          taskDescriptions.set(toolId, label);
        }
      }
    }
  }

  switch (msgType) {
    case 'system': {
      const subtype = json.subtype as string | undefined;
      const sysModel = json.model as string | undefined;
      if (subtype === 'init') {
        const label = sysModel ? `Agent starting... (${sysModel})` : 'Agent starting...';
        return [{ id, type: 'system', timestamp, summary: label, model: sysModel, rawJson: raw, isStderr }];
      }
      return [{ id, type: 'system', timestamp, summary: `System: ${subtype ?? 'event'}`, rawJson: raw, isStderr }];
    }

    case 'assistant': {
      const contentArr = msg?.content as unknown[];
      if (!Array.isArray(contentArr)) return [];

      const entries: TimelineEntry[] = [];

      for (let i = 0; i < contentArr.length; i++) {
        const b = contentArr[i] as Record<string, unknown>;
        if (b.type === 'tool_use') {
          const { toolName, toolInput, summary } = extractToolSummary(b);
          entries.push({
            id: `${id}-tool-${i}-${toolName}`,
            type: 'tool_use',
            timestamp,
            summary,
            toolName,
            toolInput,
            isSubagent,
            subagentLabel,
            model,
            rawJson: raw,
            isStderr,
          });
        } else if (b.type === 'text') {
          const text = b.text as string;
          if (text) {
            entries.push({
              id: `${id}-text-${i}`,
              type: 'assistant',
              timestamp,
              summary: truncate(text.replace(/\n/g, ' '), 120),
              content: text,
              isSubagent,
              subagentLabel,
              model,
              rawJson: raw,
              isStderr,
            });
          }
        }
      }

      return entries;
    }

    case 'user': {
      const contentArr = msg?.content as unknown[];
      if (!Array.isArray(contentArr)) {
        return [{ id, type: 'user', timestamp, summary: 'User input', isSubagent, subagentLabel, model, rawJson: raw, isStderr }];
      }

      const firstBlock = contentArr[0] as Record<string, unknown> | undefined;
      if (firstBlock?.tool_use_id) {
        let resultContent: string;
        const rawContent = firstBlock.content;
        if (typeof rawContent === 'string') {
          resultContent = rawContent;
        } else if (Array.isArray(rawContent)) {
          resultContent = rawContent
            .map((b: unknown) => {
              const block = b as Record<string, unknown>;
              return block.type === 'text' ? (block.text as string) : '';
            })
            .filter(Boolean)
            .join('\n');
        } else {
          resultContent = rawContent ? JSON.stringify(rawContent) : '';
        }
        return [{
          id,
          type: 'tool_result',
          timestamp,
          summary: truncate(`Result: ${resultContent.replace(/\n/g, ' ')}`, 120),
          content: resultContent,
          isSubagent,
          subagentLabel,
          model,
          rawJson: raw,
          isStderr,
        }];
      }

      return [{ id, type: 'user', timestamp, summary: 'User input', isSubagent, subagentLabel, model, rawJson: raw, isStderr }];
    }

    case 'result': {
      const usage = json.usage as Record<string, unknown> | undefined;
      const resultText = json.result as string | undefined;
      const topLevelCost = json.total_cost_usd as number | undefined;
      let costData: TimelineEntry['costData'] = undefined;

      if (usage || topLevelCost) {
        const inputTokens = (usage?.input_tokens as number) ?? 0;
        const cacheRead = (usage?.cache_read_input_tokens as number) ?? 0;
        const cacheCreation = (usage?.cache_creation_input_tokens as number) ?? 0;
        const outputTokens = (usage?.output_tokens as number) ?? 0;
        costData = {
          inputTokens: inputTokens + cacheRead + cacheCreation,
          outputTokens,
          totalCostUsd: topLevelCost ?? 0,
        };
      }

      const totalTokens = costData ? costData.inputTokens + costData.outputTokens : 0;
      const costStr = costData && costData.totalCostUsd > 0
        ? `$${costData.totalCostUsd < 0.01 ? costData.totalCostUsd.toFixed(4) : costData.totalCostUsd.toFixed(2)}`
        : '';

      return [{
        id,
        type: 'result',
        timestamp,
        summary: costData
          ? `Result — ${totalTokens.toLocaleString()} tokens${costStr ? `, ${costStr}` : ''}`
          : 'Result',
        content: resultText,
        costData,
        rawJson: raw,
        isStderr,
      }];
    }

    // Cursor emits tool_call events with subtype started/completed
    case 'tool_call': {
      const subtype = json.subtype as string | undefined;
      const toolCallObj = json.tool_call as Record<string, unknown> | undefined;
      if (!toolCallObj) return [];

      // Only show "started" to avoid duplicate entries per call
      if (subtype !== 'started') return [];

      const { toolName, toolInput, summary } = extractCursorToolCallSummary(toolCallObj);
      return [{
        id,
        type: 'tool_use',
        timestamp,
        summary,
        toolName,
        toolInput,
        rawJson: raw,
        isStderr,
      }];
    }

    // Cursor thinking deltas — skip individually (too frequent per-token)
    case 'thinking':
    case 'content_block_delta':
    case 'stream_event':
      return [];

    default:
      return [];
  }
}

function parseCodexEvent(
  raw: string,
  json: Record<string, unknown>,
  id: string,
  timestamp: string,
  isStderr: boolean,
): TimelineEntry[] {
  const msgType = json.type as string | undefined;
  if (!msgType) return [];

  if (msgType === 'item.completed') {
    const item = json.item as Record<string, unknown> | undefined;
    if (!item) return [];

    const itemType = item.type as string;

    if (itemType === 'agent_message') {
      const text = item.text as string ?? '';
      return [{
        id,
        type: 'assistant',
        timestamp,
        summary: truncate(text.replace(/\n/g, ' '), 120),
        content: text,
        rawJson: raw,
        isStderr,
      }];
    }

    if (itemType === 'command_execution') {
      const output = item.aggregated_output as string ?? '';
      const cmd = item.command as string ?? 'shell';
      return [{
        id,
        type: 'tool_use',
        timestamp,
        summary: `Command: ${truncate(cmd, 60)}`,
        content: output,
        toolName: 'Command',
        toolInput: cmd,
        rawJson: raw,
        isStderr,
      }];
    }

    return [];
  }

  if (msgType === 'turn.completed') {
    const usage = json.usage as Record<string, unknown> | undefined;
    let costData: TimelineEntry['costData'] = undefined;

    if (usage) {
      const input = (usage.input_tokens as number) ?? 0;
      const output = (usage.output_tokens as number) ?? 0;
      costData = { inputTokens: input, outputTokens: output, totalCostUsd: 0 };
    }

    return [{
      id,
      type: 'result',
      timestamp,
      summary: costData
        ? `Turn complete — ${costData.inputTokens + costData.outputTokens} tokens`
        : 'Turn complete',
      costData,
      rawJson: raw,
      isStderr,
    }];
  }

  return [];
}

export interface AgentLog {
  stream: string;
  message: string;
  timestamp: string;
}

export function parseAgentLogToEntries(
  logs: AgentLog[],
  agentType: string,
): TimelineEntry[] {
  const isCodex = agentType === 'codex';
  const entries: TimelineEntry[] = [];
  const taskDescriptions = new Map<string, string>();

  for (const log of logs) {
    const trimmed = log.message.trim();
    if (!trimmed) continue;
    const isStderr = log.stream === 'stderr';

    if (!trimmed.startsWith('{')) {
      entries.push({
        id: entries.length.toString(),
        type: isStderr ? 'error' : 'streaming',
        timestamp: log.timestamp,
        summary: truncate(trimmed, 120),
        rawJson: log.message,
        isStderr,
      });
      continue;
    }

    let json: Record<string, unknown>;
    try {
      json = JSON.parse(trimmed);
    } catch {
      entries.push({
        id: entries.length.toString(),
        type: isStderr ? 'error' : 'streaming',
        timestamp: log.timestamp,
        summary: truncate(trimmed, 120),
        rawJson: log.message,
        isStderr,
      });
      continue;
    }

    const parsed = isCodex
      ? parseCodexEvent(log.message, json, entries.length.toString(), log.timestamp, isStderr)
      : parseClaudeEvent(log.message, json, entries.length.toString(), log.timestamp, isStderr, taskDescriptions);

    entries.push(...parsed);
  }

  return entries;
}

export function parseLogEvents(events: RunEvent[], agentType: string): TimelineEntry[] {
  const isCodex = agentType === 'codex';
  const entries: TimelineEntry[] = [];
  // Maps Task tool_use IDs to their description for subagent labeling
  const taskDescriptions = new Map<string, string>();

  for (const event of events) {
    const typeStr = getEventTypeString(event.eventType);
    const isStdout = typeStr === 'log_stdout';
    const isStderr = typeStr === 'log_stderr';
    if (!isStdout && !isStderr) continue;

    const payload = event.payload as { raw?: string } | null;
    const raw = payload?.raw ?? '';
    if (!raw) continue;

    const trimmed = raw.trim();

    if (!trimmed.startsWith('{')) {
      if (trimmed) {
        const entryType: TimelineEntryType = isStderr ? 'error' : 'system';
        entries.push({
          id: event.id,
          type: entryType,
          timestamp: event.createdAt,
          summary: truncate(trimmed, 120),
          content: trimmed,
          rawJson: raw,
          isStderr,
        });
      }
      continue;
    }

    let json: Record<string, unknown>;
    try {
      json = JSON.parse(trimmed);
    } catch {
      entries.push({
        id: event.id,
        type: isStderr ? 'error' : 'system',
        timestamp: event.createdAt,
        summary: truncate(trimmed, 120),
        content: trimmed,
        rawJson: raw,
        isStderr,
      });
      continue;
    }

    const parsed = isCodex
      ? parseCodexEvent(raw, json, event.id, event.createdAt, isStderr)
      : parseClaudeEvent(raw, json, event.id, event.createdAt, isStderr, taskDescriptions);

    entries.push(...parsed);
  }

  return entries;
}
