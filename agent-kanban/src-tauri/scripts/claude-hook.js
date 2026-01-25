#!/usr/bin/env node
/**
 * Claude Code hook script for Agent Kanban
 * 
 * This script is called by Claude Code at various lifecycle events.
 * It reads JSON from stdin and can write to stdout/stderr.
 * 
 * Exit codes:
 * - 0: Success
 * - 2: Blocking error (stderr fed to Claude)
 */

const https = require('https');
const http = require('http');

// Get environment variables
const TICKET_ID = process.env.AGENT_KANBAN_TICKET_ID;
const RUN_ID = process.env.AGENT_KANBAN_RUN_ID;
const API_URL = process.env.AGENT_KANBAN_API_URL || 'http://127.0.0.1:7432';
const API_TOKEN = process.env.AGENT_KANBAN_API_TOKEN;

// Get the hook event type from args
const hookEvent = process.argv[2];

// Read stdin
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
    // Log error but don't block
    console.error('Hook error:', error.message);
    process.exit(0);
  }
});

/**
 * Handle a hook event
 */
async function handleHook(event, input) {
  // Post event to API first
  try {
    await postEvent(event, input);
  } catch (error) {
    console.error('Failed to post event:', error.message);
  }

  switch (event) {
    case 'UserPromptSubmit':
      handleUserPromptSubmit(input);
      break;
    
    case 'PreToolUse':
      handlePreToolUse(input);
      break;
    
    case 'PostToolUse':
      handlePostToolUse(input);
      break;
    
    case 'PostToolUseFailure':
      handlePostToolUseFailure(input);
      break;
    
    case 'Stop':
      await handleStop(input);
      break;
    
    case 'SessionStart':
    case 'SessionEnd':
      // Informational only
      break;
    
    default:
      // Unknown event
      break;
  }

  process.exit(0);
}

/**
 * Handle UserPromptSubmit hook
 * stdout is injected as context to Claude
 */
function handleUserPromptSubmit(input) {
  // Inject context about the ticket and rules
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

/**
 * Handle PreToolUse hook
 * Can block with exit code 2
 */
function handlePreToolUse(input) {
  const tool = input.tool_name || '';
  const toolInput = input.tool_input || {};

  // Check for dangerous operations
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
        // Block the command
        console.error(`Blocked dangerous command: ${command}`);
        process.exit(2); // Exit 2 feeds stderr to Claude
      }
    }
  }

  // Check for sensitive file access
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
        console.error(`Warning: Accessing potentially sensitive file: ${filePath}`);
        // Could block with exit(2) or just warn
      }
    }
  }
}

/**
 * Handle PostToolUse hook
 */
function handlePostToolUse(input) {
  // Log successful tool use - informational only
  const tool = input.tool_name || '';
  const result = input.tool_result || '';
  
  // Could log to file or update metrics
}

/**
 * Handle PostToolUseFailure hook
 */
function handlePostToolUseFailure(input) {
  // Log failed tool use
  const tool = input.tool_name || '';
  const error = input.error || '';
  
  console.error(`Tool ${tool} failed: ${error}`);
}

/**
 * Handle Stop hook
 * Finalize the run
 */
async function handleStop(input) {
  const stopReason = input.stop_reason || 'unknown';
  
  // Determine status based on stop reason
  let status = 'finished';
  let exitCode = 0;
  
  if (stopReason === 'error' || stopReason === 'tool_error') {
    status = 'error';
    exitCode = 1;
  } else if (stopReason === 'user_cancelled' || stopReason === 'interrupt') {
    status = 'aborted';
    exitCode = 130;
  }

  // Update run status
  if (RUN_ID) {
    try {
      await updateRunStatus({
        status,
        exitCode,
        summaryMd: generateSummary(stopReason, input),
      });
    } catch (error) {
      console.error('Failed to update run status:', error.message);
    }
  }

  // Could store transcript path for later reference
  if (input.transcript_path) {
    // Log or store for linking from the UI
  }
}

/**
 * Post an event to the API
 */
async function postEvent(eventType, payload) {
  if (!RUN_ID || !API_TOKEN) {
    return;
  }

  const normalizedEvent = {
    eventType: normalizeEventType(eventType, payload),
    payload: {
      raw: JSON.stringify(payload),
      structured: extractStructuredData(eventType, payload),
    },
    timestamp: new Date().toISOString(),
  };

  const url = `${API_URL}/v1/runs/${RUN_ID}/events`;
  
  try {
    await httpRequest('POST', url, normalizedEvent);
  } catch (error) {
    // Don't block on API errors
    console.error('Failed to post event:', error.message);
  }
}

/**
 * Update run status
 */
async function updateRunStatus(data) {
  if (!RUN_ID || !API_TOKEN) return;

  const url = `${API_URL}/v1/runs/${RUN_ID}`;
  await httpRequest('PATCH', url, data);
}

/**
 * Normalize Claude event type to canonical type
 */
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
    
    case 'SessionStart':
      return 'session_started';
    
    case 'SessionEnd':
      return 'session_ended';
    
    default:
      return claudeEvent.toLowerCase();
  }
}

/**
 * Extract structured data from event payload
 */
function extractStructuredData(eventType, payload) {
  const tool = payload.tool_name || '';
  const input = payload.tool_input || {};
  
  switch (eventType) {
    case 'UserPromptSubmit':
      return {
        prompt: payload.prompt,
        sessionId: payload.session_id,
      };
    
    case 'PreToolUse':
    case 'PostToolUse':
      if (tool === 'Bash') {
        return {
          tool: 'bash',
          command: input.command,
          workingDirectory: payload.cwd,
        };
      }
      if (tool === 'Read') {
        return {
          tool: 'read',
          filePath: input.file_path,
        };
      }
      if (tool === 'Edit' || tool === 'Write') {
        return {
          tool: tool.toLowerCase(),
          filePath: input.file_path,
        };
      }
      return { tool, input };
    
    case 'PostToolUseFailure':
      return {
        tool,
        error: payload.error,
      };
    
    case 'Stop':
      return {
        reason: payload.stop_reason,
        transcriptPath: payload.transcript_path,
      };
    
    default:
      return payload;
  }
}

/**
 * Generate a summary for the stop event
 */
function generateSummary(stopReason, input) {
  switch (stopReason) {
    case 'end_turn':
    case 'stop_sequence':
      return 'Agent completed the task successfully.';
    case 'max_tokens':
      return 'Agent stopped due to reaching token limit.';
    case 'tool_error':
      return 'Agent stopped due to a tool error.';
    case 'error':
      return 'Agent encountered an error and stopped.';
    case 'user_cancelled':
    case 'interrupt':
      return 'Agent was cancelled by user.';
    default:
      return `Agent stopped: ${stopReason}`;
  }
}

/**
 * Make an HTTP request
 */
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
