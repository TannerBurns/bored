#!/usr/bin/env node
/**
 * Cursor hook script for Agent Kanban.
 * Called by Cursor at lifecycle events; reads JSON from stdin, writes JSON to stdout.
 */

const https = require('https');
const http = require('http');
const fs = require('fs');
const path = require('path');
const os = require('os');

const RUN_ID = process.env.AGENT_KANBAN_RUN_ID;
const API_URL = process.env.AGENT_KANBAN_API_URL || 'http://127.0.0.1:7432';

// Get API token: try environment variable first, then fall back to reading from file
// This allows the hook to work even if Cursor caches old hooks.json commands
function getApiToken() {
  // First try the environment variable
  const envToken = process.env.AGENT_KANBAN_API_TOKEN;
  
  // Try to read the current token from the persisted file
  // This handles the case where Cursor caches old hooks.json with stale tokens
  const tokenPath = path.join(
    os.homedir(),
    'Library',
    'Application Support',
    'com.agent-kanban.app',
    'api_token'
  );
  
  try {
    const fileToken = fs.readFileSync(tokenPath, 'utf8').trim();
    if (fileToken) {
      // Always prefer the file token as it's the current server's token
      return fileToken;
    }
  } catch (e) {
    // File doesn't exist or can't be read
  }
  
  // Fall back to environment variable
  return envToken;
}

const API_TOKEN = getApiToken();

const hookEvent = process.argv[2];

let inputData = '';
process.stdin.setEncoding('utf8');

process.stdin.on('data', (chunk) => {
  inputData += chunk;
});

process.stdin.on('end', async () => {
  try {
    const input = inputData ? JSON.parse(inputData) : {};
    const result = await handleHook(hookEvent, input);
    console.log(JSON.stringify(result));
    process.exit(0);
  } catch (error) {
    console.error('Hook error:', error.message);
    console.log(JSON.stringify({ continue: true }));
    process.exit(0);
  }
});

async function handleHook(event, input) {
  // Execute hook handler first - security checks must run regardless of API availability
  let result;
  switch (event) {
    case 'beforeShellExecution':
      result = handleBeforeShellExecution(input);
      break;
    case 'beforeReadFile':
      result = handleBeforeReadFile(input);
      break;
    case 'beforeMCPExecution':
      result = { continue: true };
      break;
    case 'afterFileEdit':
      result = { continue: true };
      break;
    case 'stop':
      result = await handleStop(input);
      break;
    default:
      result = { continue: true };
  }

  // Post event after handler executes - failures should not affect hook result
  try {
    await postEvent(event, input);
  } catch (error) {
    console.error('Failed to post event:', error.message);
  }

  return result;
}

function handleBeforeShellExecution(input) {
  const command = input.command || '';
  
  const dangerousPatterns = [
    /rm\s+-rf\s+\//,
    /rm\s+-rf\s+~\//,
    /git\s+push\s+.*--force/,
    /:\(\)\{\s*:\|:&\s*\};:/,
  ];

  for (const pattern of dangerousPatterns) {
    if (pattern.test(command)) {
      return {
        continue: false,
        permission: 'deny',
        userMessage: `Blocked dangerous command: ${command}`,
        agentMessage: 'This command was blocked for safety. Please use a safer alternative.',
      };
    }
  }

  return { continue: true, permission: 'allow' };
}

function handleBeforeReadFile(input) {
  const filePath = input.path || '';
  
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
      console.error(`Warning: Reading sensitive file: ${filePath}`);
    }
  }

  return { continue: true };
}

async function handleStop(input) {
  const status = input.status || 'completed';
  const exitCode = status === 'completed' ? 0 : 1;
  
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

async function postEvent(eventType, payload) {
  if (!RUN_ID || !API_TOKEN) return;

  const normalizedEvent = {
    eventType: normalizeEventType(eventType),
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

function extractStructuredData(eventType, payload) {
  switch (eventType) {
    case 'beforeShellExecution':
      return { command: payload.command, workingDirectory: payload.cwd };
    case 'afterFileEdit':
      return { filePath: payload.path, hasChanges: true };
    case 'beforeReadFile':
      return { filePath: payload.path };
    case 'stop':
      return { status: payload.status, reason: payload.reason };
    default:
      return payload;
  }
}

function generateSummary(status) {
  if (status === 'completed') return 'Agent completed successfully.';
  if (status === 'error') return 'Agent encountered an error and stopped.';
  if (status === 'aborted') return 'Agent was aborted by user.';
  return `Agent stopped with status: ${status}`;
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
