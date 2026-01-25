#!/usr/bin/env node
/**
 * Cursor hook script for Agent Kanban
 * 
 * This script is called by Cursor at various lifecycle events.
 * It reads JSON from stdin, processes the event, and writes JSON to stdout.
 */

const https = require('https');
const http = require('http');
const fs = require('fs');
const path = require('path');

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
    const result = await handleHook(hookEvent, input);
    
    // Write result to stdout
    console.log(JSON.stringify(result));
    process.exit(0);
  } catch (error) {
    console.error('Hook error:', error.message);
    // Return continue: true to not block on errors
    console.log(JSON.stringify({ continue: true }));
    process.exit(0);
  }
});

/**
 * Handle a hook event
 */
async function handleHook(event, input) {
  // Post event to API
  await postEvent(event, input);

  // Return appropriate response based on event type
  switch (event) {
    case 'beforeShellExecution':
      return handleBeforeShellExecution(input);
    
    case 'beforeReadFile':
      return handleBeforeReadFile(input);
    
    case 'beforeMCPExecution':
      return handleBeforeMCPExecution(input);
    
    case 'afterFileEdit':
      return handleAfterFileEdit(input);
    
    case 'stop':
      return await handleStop(input);
    
    default:
      return { continue: true };
  }
}

/**
 * Handle beforeShellExecution hook
 * Can block dangerous commands
 */
function handleBeforeShellExecution(input) {
  const command = input.command || '';
  
  // Check for dangerous commands (optional blocking)
  const dangerousPatterns = [
    /rm\s+-rf\s+\//,           // rm -rf /
    /rm\s+-rf\s+~\//,          // rm -rf ~/
    /git\s+push\s+.*--force/,  // force push
    /:\(\)\{\s*:\|:&\s*\};:/,  // fork bomb
  ];

  for (const pattern of dangerousPatterns) {
    if (pattern.test(command)) {
      return {
        continue: true,
        permission: 'deny',
        userMessage: `Blocked dangerous command: ${command}`,
        agentMessage: 'This command was blocked for safety. Please use a safer alternative.',
      };
    }
  }

  // Allow the command
  return {
    continue: true,
    permission: 'allow',
  };
}

/**
 * Handle beforeReadFile hook
 * Can block reading sensitive files
 */
function handleBeforeReadFile(input) {
  const filePath = input.path || '';
  
  // Check for sensitive files
  const sensitivePatterns = [
    /\.env$/,
    /\.env\.local$/,
    /credentials\.json$/,
    /secrets\.(json|yaml|yml)$/,
    /\.ssh\//,
    /\.aws\//,
  ];

  for (const pattern of sensitivePatterns) {
    if (pattern.test(filePath)) {
      // Log but allow (or deny based on settings)
      // For now, just log
      console.error(`Warning: Reading sensitive file: ${filePath}`);
    }
  }

  return { continue: true };
}

/**
 * Handle beforeMCPExecution hook
 */
function handleBeforeMCPExecution(input) {
  // Log MCP calls, allow by default
  return { continue: true };
}

/**
 * Handle afterFileEdit hook
 */
function handleAfterFileEdit(input) {
  // This is informational only - Cursor doesn't read our response
  // But we still post the event to the API
  return { continue: true };
}

/**
 * Handle stop hook
 * Finalize the run
 */
async function handleStop(input) {
  const status = input.status || 'completed';
  const exitCode = status === 'completed' ? 0 : 1;
  
  // Update run status
  if (RUN_ID) {
    try {
      await updateRunStatus({
        status: status === 'completed' ? 'finished' : 'error',
        exitCode,
        summaryMd: generateSummary(status),
      });
    } catch (error) {
      console.error('Failed to update run status:', error.message);
    }
  }

  return { continue: true };
}

/**
 * Post an event to the API
 */
async function postEvent(eventType, payload) {
  if (!RUN_ID || !API_TOKEN) {
    console.error('Missing RUN_ID or API_TOKEN');
    return;
  }

  const normalizedEvent = {
    eventType: normalizeEventType(eventType),
    payload: {
      raw: JSON.stringify(payload),
      structured: extractStructuredData(eventType, payload),
    },
    timestamp: new Date().toISOString(),
  };

  const url = `${API_URL}/v1/runs/${RUN_ID}/events`;
  
  await httpRequest('POST', url, normalizedEvent);
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
 * Normalize Cursor event type to canonical type
 */
function normalizeEventType(cursorEvent) {
  const mapping = {
    'beforeShellExecution': 'command_requested',
    'afterFileEdit': 'file_edited',
    'beforeReadFile': 'file_read',
    'beforeMCPExecution': 'command_requested',
    'stop': 'run_stopped',
  };
  return mapping[cursorEvent] || cursorEvent;
}

/**
 * Extract structured data from event payload
 */
function extractStructuredData(eventType, payload) {
  switch (eventType) {
    case 'beforeShellExecution':
      return {
        command: payload.command,
        workingDirectory: payload.cwd,
      };
    
    case 'afterFileEdit':
      return {
        filePath: payload.path,
        // Note: Cursor may provide old/new content
        hasChanges: true,
      };
    
    case 'beforeReadFile':
      return {
        filePath: payload.path,
      };
    
    case 'stop':
      return {
        status: payload.status,
        reason: payload.reason,
      };
    
    default:
      return payload;
  }
}

/**
 * Generate a summary for the stop event
 */
function generateSummary(status) {
  if (status === 'completed') {
    return 'Agent completed successfully.';
  } else if (status === 'error') {
    return 'Agent encountered an error and stopped.';
  } else if (status === 'aborted') {
    return 'Agent was aborted by user.';
  }
  return `Agent stopped with status: ${status}`;
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
