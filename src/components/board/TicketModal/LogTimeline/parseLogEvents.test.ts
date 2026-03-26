import { describe, it, expect } from 'vitest';
import { parseLogEvents, parseAgentLogToEntries } from './parseLogEvents';
import type { AgentLog } from './parseLogEvents';
import type { RunEvent } from '../types';

function mkEvent(overrides: Partial<RunEvent> = {}): RunEvent {
  return {
    id: 'evt-1',
    eventType: 'log_stdout',
    payload: { raw: '' },
    createdAt: '2025-06-15T12:00:00Z',
    ...overrides,
  };
}

function jsonPayload(obj: Record<string, unknown>): { raw: string } {
  return { raw: JSON.stringify(obj) };
}

describe('parseLogEvents', () => {
  // -----------------------------------------------------------------------
  // Filtering & routing
  // -----------------------------------------------------------------------

  describe('event filtering', () => {
    it('only processes log_stdout and log_stderr events', () => {
      const events = [
        mkEvent({ id: 'a', eventType: 'log_stdout', payload: { raw: 'stdout line' } }),
        mkEvent({ id: 'b', eventType: 'agent_status', payload: { raw: 'ignored' } }),
        mkEvent({ id: 'c', eventType: 'log_stderr', payload: { raw: 'stderr line' } }),
        mkEvent({ id: 'd', eventType: 'heartbeat', payload: { raw: 'nope' } }),
      ];

      const entries = parseLogEvents(events, 'claude');
      expect(entries).toHaveLength(2);
      expect(entries[0].id).toBe('a');
      expect(entries[1].id).toBe('c');
    });

    it('handles eventType as {custom: "log_stdout"} object', () => {
      const events = [
        mkEvent({ id: 'e1', eventType: { custom: 'log_stdout' }, payload: { raw: 'from custom' } }),
      ];

      const entries = parseLogEvents(events, 'claude');
      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
    });

    it('handles eventType as single-key object {log_stdout: true}', () => {
      const events = [
        mkEvent({ id: 'e1', eventType: { some_key: 'log_stdout' }, payload: { raw: 'from single-key' } }),
      ];

      const entries = parseLogEvents(events, 'claude');
      expect(entries).toHaveLength(1);
    });

    it('skips events with empty or missing raw payload', () => {
      const events = [
        mkEvent({ id: 'a', payload: { raw: '' } }),
        mkEvent({ id: 'b', payload: null }),
        mkEvent({ id: 'c', payload: { raw: '   ' } }),
        mkEvent({ id: 'd', payload: { raw: 'valid' } }),
      ];

      const entries = parseLogEvents(events, 'claude');
      expect(entries).toHaveLength(1);
      expect(entries[0].id).toBe('d');
    });

    it('returns empty array when no events match', () => {
      expect(parseLogEvents([], 'claude')).toEqual([]);
      expect(parseLogEvents([mkEvent({ eventType: 'other' })], 'claude')).toEqual([]);
    });
  });

  // -----------------------------------------------------------------------
  // Non-JSON / malformed input
  // -----------------------------------------------------------------------

  describe('non-JSON lines', () => {
    it('creates system entry for plain stdout text', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: { raw: 'Starting server...' } })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
      expect(entries[0].summary).toBe('Starting server...');
      expect(entries[0].isStderr).toBe(false);
    });

    it('creates error entry for plain stderr text', () => {
      const entries = parseLogEvents(
        [mkEvent({ eventType: 'log_stderr', payload: { raw: 'Connection refused' } })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('error');
      expect(entries[0].isStderr).toBe(true);
    });

    it('truncates long non-JSON summaries at 120 chars', () => {
      const longText = 'A'.repeat(200);
      const entries = parseLogEvents(
        [mkEvent({ payload: { raw: longText } })],
        'claude',
      );

      expect(entries[0].summary.length).toBeLessThanOrEqual(123); // 120 + "..."
      expect(entries[0].summary.endsWith('...')).toBe(true);
      expect(entries[0].content).toBe(longText);
    });

    it('creates system entry for malformed JSON', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: { raw: '{ broken json' } })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
      expect(entries[0].content).toBe('{ broken json');
    });

    it('creates error entry for malformed JSON on stderr', () => {
      const entries = parseLogEvents(
        [mkEvent({ eventType: 'log_stderr', payload: { raw: '{ broken' } })],
        'claude',
      );

      expect(entries[0].type).toBe('error');
      expect(entries[0].isStderr).toBe(true);
    });
  });

  // -----------------------------------------------------------------------
  // Claude / Cursor format
  // -----------------------------------------------------------------------

  describe('Claude/Cursor format parsing', () => {
    it('parses system init event', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'system', subtype: 'init' }) })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
      expect(entries[0].summary).toBe('Agent starting...');
    });

    it('parses system event with other subtype', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'system', subtype: 'config' }) })],
        'claude',
      );

      expect(entries[0].type).toBe('system');
      expect(entries[0].summary).toBe('System: config');
    });

    it('parses system event without subtype', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'system' }) })],
        'claude',
      );

      expect(entries[0].summary).toBe('System: event');
    });

    it('parses assistant message with text content', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'assistant',
            message: {
              content: [{ type: 'text', text: 'I will help you fix the bug.' }],
            },
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('assistant');
      expect(entries[0].summary).toContain('I will help you fix the bug.');
      expect(entries[0].content).toBe('I will help you fix the bug.');
    });

    it('parses assistant message with tool_use content', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'assistant',
            message: {
              content: [{
                type: 'tool_use',
                name: 'read_file',
                input: { file_path: '/src/main.ts' },
              }],
            },
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('tool_use');
      expect(entries[0].toolName).toBe('read_file');
      expect(entries[0].toolInput).toBe('/src/main.ts');
      expect(entries[0].summary).toContain('read_file');
    });

    it('returns all content blocks when text and tool_use are both present', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'assistant',
            message: {
              content: [
                { type: 'text', text: 'Let me read the file.' },
                { type: 'tool_use', name: 'read_file', input: { path: 'test.ts' } },
              ],
            },
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(2);
      expect(entries[0].type).toBe('assistant');
      expect(entries[1].type).toBe('tool_use');
    });

    it('returns all tool_use blocks when multiple are present', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'assistant',
            message: {
              content: [
                { type: 'text', text: 'I will read both files.' },
                { type: 'tool_use', name: 'read_file', input: { path: '/src/a.ts' } },
                { type: 'tool_use', name: 'read_file', input: { path: '/src/b.ts' } },
                { type: 'tool_use', name: 'write_file', input: { path: '/src/c.ts' } },
              ],
            },
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(4);
      expect(entries[0].type).toBe('assistant');
      expect(entries[1].type).toBe('tool_use');
      expect(entries[1].toolInput).toBe('/src/a.ts');
      expect(entries[2].type).toBe('tool_use');
      expect(entries[2].toolInput).toBe('/src/b.ts');
      expect(entries[3].type).toBe('tool_use');
      expect(entries[3].toolInput).toBe('/src/c.ts');

      const ids = entries.map(e => e.id);
      expect(new Set(ids).size).toBe(ids.length);
    });

    it('returns null for assistant with no content array', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'assistant', message: {} }) })],
        'claude',
      );

      expect(entries).toHaveLength(0);
    });

    it('returns null for assistant with empty content array', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({ type: 'assistant', message: { content: [] } }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(0);
    });

    it('parses user message (plain)', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'user',
            message: { content: [{ type: 'text', text: 'hello' }] },
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('user');
      expect(entries[0].summary).toBe('User input');
    });

    it('parses user message with tool_result (string content)', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'user',
            message: {
              content: [{
                tool_use_id: 'tool-123',
                content: 'File contents here',
              }],
            },
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('tool_result');
      expect(entries[0].summary).toContain('Result: File contents here');
      expect(entries[0].content).toBe('File contents here');
    });

    it('parses user message with tool_result (array content)', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'user',
            message: {
              content: [{
                tool_use_id: 'tool-456',
                content: [
                  { type: 'text', text: 'Line 1' },
                  { type: 'text', text: 'Line 2' },
                ],
              }],
            },
          }),
        })],
        'claude',
      );

      expect(entries[0].type).toBe('tool_result');
      expect(entries[0].content).toBe('Line 1\nLine 2');
    });

    it('parses user message without content array as plain user input', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({ type: 'user', message: {} }),
        })],
        'claude',
      );

      expect(entries[0].type).toBe('user');
      expect(entries[0].summary).toBe('User input');
    });

    it('parses result event with usage and cost', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'result',
            usage: {
              input_tokens: 1000,
              output_tokens: 500,
              cache_read_input_tokens: 200,
              cache_creation_input_tokens: 100,
            },
            total_cost_usd: 0.025,
            result: 'Task completed successfully',
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('result');
      expect(entries[0].costData).toBeDefined();
      expect(entries[0].costData!.inputTokens).toBe(1300); // 1000 + 200 + 100
      expect(entries[0].costData!.outputTokens).toBe(500);
      expect(entries[0].costData!.totalCostUsd).toBe(0.025);
      expect(entries[0].content).toBe('Task completed successfully');
    });

    it('parses result event without usage', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'result' }) })],
        'claude',
      );

      expect(entries[0].type).toBe('result');
      expect(entries[0].summary).toBe('Result');
      expect(entries[0].costData).toBeUndefined();
    });

    it('parses result event with cost only (no usage)', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'result', total_cost_usd: 0.05 }) })],
        'claude',
      );

      expect(entries[0].costData).toBeDefined();
      expect(entries[0].costData!.totalCostUsd).toBe(0.05);
    });

    it('parses Cursor tool_call (started subtype)', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'tool_call',
            subtype: 'started',
            tool_call: {
              readToolCall: {
                args: { path: '/src/utils.ts' },
              },
            },
          }),
        })],
        'cursor',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('tool_use');
      expect(entries[0].toolName).toBe('Read');
      expect(entries[0].toolInput).toBe('/src/utils.ts');
    });

    it('skips Cursor tool_call with completed subtype', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'tool_call',
            subtype: 'completed',
            tool_call: { readToolCall: { args: { path: '/src/utils.ts' } } },
          }),
        })],
        'cursor',
      );

      expect(entries).toHaveLength(0);
    });

    it('strips worktree path prefix from Cursor tool calls', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'tool_call',
            subtype: 'started',
            tool_call: {
              readToolCall: {
                args: { path: '/private/var/folders/xx/yy/worktrees/my-branch/src/main.ts' },
              },
            },
          }),
        })],
        'cursor',
      );

      expect(entries[0].toolInput).toBe('src/main.ts');
    });

    it.each([
      'thinking',
      'content_block_delta',
      'stream_event',
    ] as const)('skips %s event type', (msgType) => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: msgType }) })],
        'claude',
      );

      expect(entries).toHaveLength(0);
    });

    it('returns null for unknown JSON type', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'unknown_event_type' }) })],
        'claude',
      );

      expect(entries).toHaveLength(0);
    });

    it('returns null when JSON has no type field', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ data: 'something' }) })],
        'claude',
      );

      expect(entries).toHaveLength(0);
    });

    it('parses bored_system event as system entry', () => {
      const entries = parseLogEvents(
        [mkEvent({
          id: 'bs-1',
          payload: jsonPayload({
            type: 'bored_system',
            message: 'CLI Command [plan]',
            command: 'claude --model sonnet-4.5 exec -p "plan prompt"',
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
      expect(entries[0].id).toBe('bs-1');
      expect(entries[0].summary).toBe('CLI Command [plan]');
      expect(entries[0].content).toBe('claude --model sonnet-4.5 exec -p "plan prompt"');
    });

    it('bored_system falls back to default summary when message missing', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'bored_system',
            command: 'some-cmd',
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].summary).toBe('Bored System');
    });

    it('bored_system is handled for codex agent type too', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'bored_system',
            message: 'CLI Command [implement]',
            command: 'codex exec -p "impl"',
          }),
        })],
        'codex',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
      expect(entries[0].summary).toBe('CLI Command [implement]');
    });
  });

  // -----------------------------------------------------------------------
  // Codex format
  // -----------------------------------------------------------------------

  describe('Codex format parsing', () => {
    it('parses agent_message from item.completed', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'item.completed',
            item: {
              type: 'agent_message',
              text: 'I found the issue in main.rs',
            },
          }),
        })],
        'codex',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('assistant');
      expect(entries[0].content).toBe('I found the issue in main.rs');
      expect(entries[0].summary).toContain('I found the issue in main.rs');
    });

    it('parses command_execution from item.completed', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'item.completed',
            item: {
              type: 'command_execution',
              command: 'ls -la /src',
              aggregated_output: 'total 42\ndrwxr-xr-x ...',
            },
          }),
        })],
        'codex',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('tool_use');
      expect(entries[0].toolName).toBe('Command');
      expect(entries[0].summary).toContain('ls -la /src');
      expect(entries[0].content).toBe('total 42\ndrwxr-xr-x ...');
    });

    it('parses turn.completed with usage data', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'turn.completed',
            usage: { input_tokens: 2000, output_tokens: 800 },
          }),
        })],
        'codex',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('result');
      expect(entries[0].costData).toEqual({
        inputTokens: 2000,
        outputTokens: 800,
        totalCostUsd: 0,
      });
      expect(entries[0].summary).toContain('2800 tokens');
    });

    it('parses turn.completed without usage', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'turn.completed' }) })],
        'codex',
      );

      expect(entries[0].type).toBe('result');
      expect(entries[0].summary).toBe('Turn complete');
      expect(entries[0].costData).toBeUndefined();
    });

    it('returns null for item.completed with no item', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'item.completed' }) })],
        'codex',
      );

      expect(entries).toHaveLength(0);
    });

    it('returns null for item.completed with unknown item type', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'item.completed',
            item: { type: 'unknown_item' },
          }),
        })],
        'codex',
      );

      expect(entries).toHaveLength(0);
    });

    it('returns null for unknown Codex event type', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'some.other.event' }) })],
        'codex',
      );

      expect(entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------------------
  // Agent type routing
  // -----------------------------------------------------------------------

  describe('agent type routing', () => {
    const codexTurnPayload = jsonPayload({ type: 'turn.completed' });
    const claudeResultPayload = jsonPayload({ type: 'result' });

    it('uses Codex parser when agentType is "codex"', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: codexTurnPayload })],
        'codex',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].summary).toContain('Turn complete');
    });

    it('uses Claude parser for non-codex agent types', () => {
      for (const agentType of ['claude', 'cursor', 'other']) {
        const entries = parseLogEvents(
          [mkEvent({ payload: claudeResultPayload })],
          agentType,
        );

        expect(entries).toHaveLength(1);
        expect(entries[0].type).toBe('result');
      }
    });

    it('Codex parser ignores Claude-specific event types', () => {
      const entries = parseLogEvents(
        [mkEvent({ payload: jsonPayload({ type: 'result' }) })],
        'codex',
      );

      expect(entries).toHaveLength(0);
    });
  });

  // -----------------------------------------------------------------------
  // Tool summary extraction
  // -----------------------------------------------------------------------

  describe('tool summary extraction', () => {
    it.each([
      ['file_path', '/src/main.ts'],
      ['path', '/utils/helper.ts'],
      ['command', 'npm run build'],
      ['pattern', '*.tsx'],
      ['query', 'auth middleware'],
    ] as const)('extracts detail from input.%s', (key, value) => {
      const input: Record<string, string> = {};
      input[key] = value;

      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'assistant',
            message: {
              content: [{
                type: 'tool_use',
                name: 'some_tool',
                input,
              }],
            },
          }),
        })],
        'claude',
      );

      expect(entries[0].summary).toContain(value);
    });

    it('shows "Using <name>" when no recognizable input field', () => {
      const entries = parseLogEvents(
        [mkEvent({
          payload: jsonPayload({
            type: 'assistant',
            message: {
              content: [{
                type: 'tool_use',
                name: 'custom_tool',
                input: { data: [1, 2, 3] },
              }],
            },
          }),
        })],
        'claude',
      );

      expect(entries[0].summary).toBe('Using custom_tool');
    });
  });

  // -----------------------------------------------------------------------
  // Mixed events end-to-end
  // -----------------------------------------------------------------------

  describe('end-to-end processing', () => {
    it('processes a realistic sequence of Claude events', () => {
      const events: RunEvent[] = [
        mkEvent({
          id: 'e1',
          payload: jsonPayload({ type: 'system', subtype: 'init' }),
          createdAt: '2025-06-15T12:00:00Z',
        }),
        mkEvent({
          id: 'e2',
          payload: jsonPayload({
            type: 'assistant',
            message: { content: [{ type: 'text', text: 'Analyzing the codebase...' }] },
          }),
          createdAt: '2025-06-15T12:00:01Z',
        }),
        mkEvent({
          id: 'e3',
          payload: jsonPayload({
            type: 'assistant',
            message: {
              content: [{
                type: 'tool_use',
                name: 'read_file',
                input: { file_path: '/src/app.ts' },
              }],
            },
          }),
          createdAt: '2025-06-15T12:00:02Z',
        }),
        mkEvent({
          id: 'e4',
          payload: jsonPayload({
            type: 'user',
            message: {
              content: [{
                tool_use_id: 'tu-1',
                content: 'export default function App() {}',
              }],
            },
          }),
          createdAt: '2025-06-15T12:00:03Z',
        }),
        mkEvent({
          id: 'e5',
          eventType: 'agent_status',
          payload: { raw: 'should be filtered' },
          createdAt: '2025-06-15T12:00:04Z',
        }),
        mkEvent({
          id: 'e6',
          payload: jsonPayload({
            type: 'result',
            usage: { input_tokens: 500, output_tokens: 200 },
            total_cost_usd: 0.01,
          }),
          createdAt: '2025-06-15T12:00:05Z',
        }),
      ];

      const entries = parseLogEvents(events, 'claude');

      expect(entries).toHaveLength(5);
      const types = entries.map(e => e.type);
      expect(types).toEqual(['system', 'assistant', 'tool_use', 'tool_result', 'result']);
    });

    it('preserves event ordering', () => {
      const events: RunEvent[] = [
        mkEvent({ id: 'z', payload: { raw: 'first' }, createdAt: '2025-06-15T12:00:00Z' }),
        mkEvent({ id: 'a', payload: { raw: 'second' }, createdAt: '2025-06-15T12:00:01Z' }),
        mkEvent({ id: 'm', payload: { raw: 'third' }, createdAt: '2025-06-15T12:00:02Z' }),
      ];

      const entries = parseLogEvents(events, 'claude');
      expect(entries.map(e => e.id)).toEqual(['z', 'a', 'm']);
    });
  });
});

function mkLog(overrides: Partial<AgentLog> = {}): AgentLog {
  return {
    stream: 'stdout',
    message: '',
    timestamp: '2025-06-15T12:00:00Z',
    ...overrides,
  };
}

describe('parseAgentLogToEntries', () => {
  describe('filtering', () => {
    it('returns empty array for empty input', () => {
      expect(parseAgentLogToEntries([], 'claude')).toEqual([]);
    });

    it('skips logs with empty or whitespace-only messages', () => {
      const logs = [
        mkLog({ message: '' }),
        mkLog({ message: '   ' }),
        mkLog({ message: '\n\t' }),
        mkLog({ message: 'valid' }),
      ];

      const entries = parseAgentLogToEntries(logs, 'claude');
      expect(entries).toHaveLength(1);
      expect(entries[0].summary).toBe('valid');
    });
  });

  describe('plain text logs', () => {
    it('creates streaming entry for stdout text', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({ message: 'Starting agent...' })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('streaming');
      expect(entries[0].summary).toBe('Starting agent...');
      expect(entries[0].isStderr).toBe(false);
    });

    it('creates error entry for stderr text', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({ stream: 'stderr', message: 'Connection refused' })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('error');
      expect(entries[0].isStderr).toBe(true);
    });

    it('truncates long non-JSON summaries at 120 chars', () => {
      const longText = 'A'.repeat(200);
      const entries = parseAgentLogToEntries(
        [mkLog({ message: longText })],
        'claude',
      );

      expect(entries[0].summary.length).toBeLessThanOrEqual(123);
      expect(entries[0].summary.endsWith('...')).toBe(true);
    });
  });

  describe('malformed JSON', () => {
    it('creates streaming entry for invalid JSON on stdout', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({ message: '{ broken json' })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('streaming');
    });

    it('creates error entry for invalid JSON on stderr', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({ stream: 'stderr', message: '{ broken' })],
        'claude',
      );

      expect(entries[0].type).toBe('error');
      expect(entries[0].isStderr).toBe(true);
    });
  });

  describe('Claude JSON events', () => {
    it('parses system init event', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({ message: JSON.stringify({ type: 'system', subtype: 'init' }) })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
      expect(entries[0].summary).toBe('Agent starting...');
    });

    it('parses assistant text message', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({
            type: 'assistant',
            message: { content: [{ type: 'text', text: 'I will fix that.' }] },
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('assistant');
      expect(entries[0].content).toBe('I will fix that.');
    });

    it('parses assistant tool_use message', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({
            type: 'assistant',
            message: {
              content: [{
                type: 'tool_use',
                name: 'read_file',
                input: { file_path: '/src/main.ts' },
              }],
            },
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('tool_use');
      expect(entries[0].toolName).toBe('read_file');
    });

    it('parses result event with cost data', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({
            type: 'result',
            usage: { input_tokens: 800, output_tokens: 300 },
            total_cost_usd: 0.015,
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('result');
      expect(entries[0].costData?.totalCostUsd).toBe(0.015);
    });

    it('skips thinking and stream_event types', () => {
      const entries = parseAgentLogToEntries(
        [
          mkLog({ message: JSON.stringify({ type: 'thinking' }) }),
          mkLog({ message: JSON.stringify({ type: 'stream_event' }) }),
          mkLog({ message: JSON.stringify({ type: 'content_block_delta' }) }),
        ],
        'claude',
      );

      expect(entries).toHaveLength(0);
    });
  });

  describe('bored_system events', () => {
    it('parses bored_system as system entry with message and command', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({
            type: 'bored_system',
            message: 'CLI Command [implement]',
            command: 'claude --model opus-4 --prompt "hello"',
          }),
        })],
        'claude',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
      expect(entries[0].summary).toBe('CLI Command [implement]');
      expect(entries[0].content).toBe('claude --model opus-4 --prompt "hello"');
      expect(entries[0].isStderr).toBe(false);
    });

    it('falls back to "Bored System" when message is missing', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({ type: 'bored_system', command: 'echo test' }),
        })],
        'claude',
      );

      expect(entries[0].summary).toBe('Bored System');
      expect(entries[0].content).toBe('echo test');
    });

    it('is parsed before agent-specific routing for codex', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({
            type: 'bored_system',
            message: 'CLI Command [plan]',
            command: 'codex exec',
          }),
        })],
        'codex',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('system');
    });

    it('marks stderr flag when log stream is stderr', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          stream: 'stderr',
          message: JSON.stringify({
            type: 'bored_system',
            message: 'CLI Command [plan]',
            command: 'claude --print',
          }),
        })],
        'claude',
      );

      expect(entries[0].type).toBe('system');
      expect(entries[0].isStderr).toBe(true);
    });
  });

  describe('Codex JSON events', () => {
    it('parses agent_message from item.completed', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({
            type: 'item.completed',
            item: { type: 'agent_message', text: 'Found the bug' },
          }),
        })],
        'codex',
      );

      expect(entries).toHaveLength(1);
      expect(entries[0].type).toBe('assistant');
      expect(entries[0].content).toBe('Found the bug');
    });

    it('parses command_execution from item.completed', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({
            type: 'item.completed',
            item: { type: 'command_execution', command: 'npm test', aggregated_output: 'ok' },
          }),
        })],
        'codex',
      );

      expect(entries[0].type).toBe('tool_use');
      expect(entries[0].toolName).toBe('Command');
    });

    it('parses turn.completed with usage', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({
          message: JSON.stringify({
            type: 'turn.completed',
            usage: { input_tokens: 1000, output_tokens: 400 },
          }),
        })],
        'codex',
      );

      expect(entries[0].type).toBe('result');
      expect(entries[0].costData?.inputTokens).toBe(1000);
      expect(entries[0].costData?.outputTokens).toBe(400);
    });
  });

  describe('agent type routing', () => {
    it('routes to Codex parser when agentType is codex', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({ message: JSON.stringify({ type: 'turn.completed' }) })],
        'codex',
      );

      expect(entries[0].summary).toContain('Turn complete');
    });

    it('routes to Claude parser for other agent types', () => {
      const entries = parseAgentLogToEntries(
        [mkLog({ message: JSON.stringify({ type: 'result' }) })],
        'claude',
      );

      expect(entries[0].type).toBe('result');
    });
  });

  describe('incremental IDs', () => {
    it('assigns sequential IDs based on entries array length', () => {
      const entries = parseAgentLogToEntries(
        [
          mkLog({ message: 'plain text one' }),
          mkLog({ message: 'plain text two' }),
          mkLog({ message: 'plain text three' }),
        ],
        'claude',
      );

      expect(entries.map(e => e.id)).toEqual(['0', '1', '2']);
    });
  });

  describe('mixed event sequence', () => {
    it('processes a realistic mix of plain text, JSON events, and errors', () => {
      const logs: AgentLog[] = [
        mkLog({ message: 'Agent starting up', timestamp: '2025-06-15T12:00:00Z' }),
        mkLog({
          message: JSON.stringify({ type: 'system', subtype: 'init' }),
          timestamp: '2025-06-15T12:00:01Z',
        }),
        mkLog({
          message: JSON.stringify({
            type: 'assistant',
            message: { content: [{ type: 'text', text: 'Analyzing...' }] },
          }),
          timestamp: '2025-06-15T12:00:02Z',
        }),
        mkLog({ stream: 'stderr', message: 'Warning: deprecated API', timestamp: '2025-06-15T12:00:03Z' }),
        mkLog({
          message: JSON.stringify({ type: 'result', total_cost_usd: 0.01 }),
          timestamp: '2025-06-15T12:00:04Z',
        }),
      ];

      const entries = parseAgentLogToEntries(logs, 'claude');

      expect(entries).toHaveLength(5);
      expect(entries.map(e => e.type)).toEqual([
        'streaming', 'system', 'assistant', 'error', 'result',
      ]);
    });
  });
});
