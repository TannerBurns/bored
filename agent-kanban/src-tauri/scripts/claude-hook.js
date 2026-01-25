#!/usr/bin/env node
/**
 * Claude Code hook script for Agent Kanban.
 * Called by Claude Code at lifecycle events; reads JSON from stdin.
 * Exit 0 = success, Exit 2 = blocking error (stderr fed to Claude).
 */

const https = require('https');
const http = require('http');

const TICKET_ID = process.env.AGENT_KANBAN_TICKET_ID;
const RUN_ID = process.env.AGENT_KANBAN_RUN_ID;
const API_URL = process.env.AGENT_KANBAN_API_URL || 'http://127.0.0.1:7432';
const API_TOKEN = process.env.AGENT_KANBAN_API_TOKEN;

const hookEvent = process.argv[2];

let inputData = '';
process.stdin.setEncoding('utf8');

process.stdin.on('data', (chunk) => {
  inputData += chunk;
});

process.stdin.on('end', async () => {
  try {
    const input = inputData ? JSON.parse(inputData) : {};
    await handleHook(hookEvent, input);
  } catch (error) {
    console.error('Hook error:', error.message);
    process.exit(2);
  }
});

async function handleHook(event, input) {
  // Execute handler first - security checks must run regardless of API availability
  switch (event) {
    case 'UserPromptSubmit':
      handleUserPromptSubmit();
      break;
    case 'PreToolUse':
      handlePreToolUse(input);
      break;
    case 'PostToolUseFailure':
      console.error(`Tool ${input.tool_name || 'unknown'} failed: ${input.error || ''}`);
      break;
    case 'Stop':
      await handleStop(input);
      break;
    default:
      break;
  }

  // Post event after handler - failures should not affect hook result
  try {
    await postEvent(event, input);
  } catch (error) {
    console.error('Failed to post event:', error.message);
  }

  process.exit(0);
}

function handleUserPromptSubmit() {
  if (TICKET_ID && RUN_ID) {
    console.log(`
## Agent Kanban Context

You are working on ticket ${TICKET_ID} (run ${RUN_ID}).

### Guidelines:
1. Focus on completing the described task
2. Make incremental changes and verify they work
3. Commit your changes with descriptive messages
4. If blocked, document the issue clearly

### Important:
- Your actions are being tracked and will appear in the ticket timeline
- Avoid accessing sensitive files (.env, credentials, etc.)
`);
  }
}

function handlePreToolUse(input) {
  const tool = input.tool_name || '';
  const toolInput = input.tool_input || {};

  if (tool === 'Bash') {
    const command = toolInput.command || '';
    
    const dangerousPatterns = [
      /rm\s+-rf\s+\//,
      /rm\s+-rf\s+~\//,
      /git\s+push\s+.*--force/,
      /sudo\s+rm/,
      /mkfs\./,
      /:\(\)\{\s*:\|:&\s*\};:/,
    ];

    for (const pattern of dangerousPatterns) {
      if (pattern.test(command)) {
        console.error(`Blocked dangerous command: ${command}`);
        process.exit(2);
      }
    }
  }

  if (tool === 'Read' || tool === 'Edit' || tool === 'Write') {
    const filePath = toolInput.file_path || toolInput.path || '';
    
    const sensitivePatterns = [
      /\.env$/,
      /\.env\.local$/,
      /credentials\.(json|yaml|yml)$/,
      /secrets\.(json|yaml|yml)$/,
    ];

    for (const pattern of sensitivePatterns) {
      if (pattern.test(filePath)) {
        console.error(`Warning: Accessing sensitive file: ${filePath}`);
      }
    }
  }
}

async function handleStop(input) {
  const stopReason = input.stop_reason || 'unknown';
  
  let status = 'finished';
  let exitCode = 0;
  
  if (stopReason === 'error' || stopReason === 'tool_error') {
    status = 'error';
    exitCode = 1;
  } else if (stopReason === 'user_cancelled' || stopReason === 'interrupt') {
    status = 'aborted';
    exitCode = 130;
  }

  if (RUN_ID) {
    try {
      await updateRunStatus({
        status,
        exitCode,
        summaryMd: generateSummary(stopReason),
      });
    } catch (error) {
      console.error('Failed to update run status:', error.message);
    }
  }
}

async function postEvent(eventType, payload) {
  if (!RUN_ID || !API_TOKEN) return;

  const normalizedEvent = {
    eventType: normalizeEventType(eventType, payload),
    payload: {
      raw: JSON.stringify(payload),
      structured: extractStructuredData(eventType, payload),
    },
    timestamp: new Date().toISOString(),
  };

  await httpRequest('POST', `${API_URL}/v1/runs/${RUN_ID}/events`, normalizedEvent);
}

async function updateRunStatus(data) {
  if (!RUN_ID || !API_TOKEN) return;
  await httpRequest('PATCH', `${API_URL}/v1/runs/${RUN_ID}`, data);
}

function normalizeEventType(claudeEvent, payload) {
  const toolName = payload.tool_name || '';
  
  switch (claudeEvent) {
    case 'UserPromptSubmit':
      return 'prompt_submitted';
    case 'PreToolUse':
      if (toolName === 'Bash') return 'command_requested';
      if (['Read', 'Edit', 'Write'].includes(toolName)) return 'file_read';
      return 'command_requested';
    case 'PostToolUse':
      if (toolName === 'Bash') return 'command_executed';
      if (['Edit', 'Write'].includes(toolName)) return 'file_edited';
      return 'command_executed';
    case 'PostToolUseFailure':
      return 'command_failed';
    case 'Stop':
      return 'run_stopped';
    default:
      return claudeEvent.toLowerCase();
  }
}

function extractStructuredData(eventType, payload) {
  const tool = payload.tool_name || '';
  const input = payload.tool_input || {};
  
  switch (eventType) {
    case 'UserPromptSubmit':
      return { prompt: payload.prompt, sessionId: payload.session_id };
    case 'PreToolUse':
    case 'PostToolUse':
      if (tool === 'Bash') {
        return { tool: 'bash', command: input.command, workingDirectory: payload.cwd };
      }
      if (tool === 'Read') {
        return { tool: 'read', filePath: input.file_path };
      }
      if (tool === 'Edit' || tool === 'Write') {
        return { tool: tool.toLowerCase(), filePath: input.file_path };
      }
      return { tool, input };
    case 'PostToolUseFailure':
      return { tool, error: payload.error };
    case 'Stop':
      return { reason: payload.stop_reason, transcriptPath: payload.transcript_path };
    default:
      return payload;
  }
}

function generateSummary(stopReason) {
  if (stopReason === 'end_turn' || stopReason === 'stop_sequence') {
    return 'Agent completed successfully.';
  }
  if (stopReason === 'max_tokens') return 'Agent stopped due to token limit.';
  if (stopReason === 'tool_error') return 'Agent stopped due to a tool error.';
  if (stopReason === 'error') return 'Agent encountered an error and stopped.';
  if (stopReason === 'user_cancelled' || stopReason === 'interrupt') {
    return 'Agent was cancelled by user.';
  }
  return `Agent stopped: ${stopReason}`;
}

function httpRequest(method, url, data) {
  return new Promise((resolve, reject) => {
    const urlObj = new URL(url);
    const isHttps = urlObj.protocol === 'https:';
    const lib = isHttps ? https : http;

    const options = {
      hostname: urlObj.hostname,
      port: urlObj.port || (isHttps ? 443 : 80),
      path: urlObj.pathname + urlObj.search,
      method,
      headers: {
        'Content-Type': 'application/json',
        'X-AgentKanban-Token': API_TOKEN,
      },
    };

    const req = lib.request(options, (res) => {
      let body = '';
      res.on('data', chunk => body += chunk);
      res.on('end', () => {
        if (res.statusCode >= 200 && res.statusCode < 300) {
          resolve(body);
        } else {
          reject(new Error(`HTTP ${res.statusCode}: ${body}`));
        }
      });
    });

    req.on('error', reject);
    req.setTimeout(5000, () => {
      req.destroy();
      reject(new Error('Request timeout'));
    });

    if (data) {
      req.write(JSON.stringify(data));
    }
    req.end();
  });
}
