#!/usr/bin/env node
/**
 * Unified hook script for Cursor and Claude Code.
 * Normalizes events and posts them to the local API with offline spooling.
 */

const https = require('https');
const http = require('http');
const fs = require('fs');
const path = require('path');
const os = require('os');

const CONFIG = {
  apiUrl: process.env.AGENT_KANBAN_API_URL || 'http://127.0.0.1:7432',
  apiToken: process.env.AGENT_KANBAN_API_TOKEN,
  ticketId: process.env.AGENT_KANBAN_TICKET_ID,
  runId: process.env.AGENT_KANBAN_RUN_ID,
  agentType: process.env.AGENT_KANBAN_AGENT_TYPE || detectAgentType(),
  spoolDir: process.env.AGENT_KANBAN_SPOOL_DIR || getDefaultSpoolDir(),
  maxRetries: 3,
  retryDelayMs: 1000,
};

function getDefaultSpoolDir() {
  const baseDir = process.platform === 'darwin'
    ? path.join(os.homedir(), 'Library', 'Application Support', 'agent-kanban')
    : process.platform === 'win32'
    ? path.join(os.homedir(), 'AppData', 'Roaming', 'agent-kanban')
    : path.join(os.homedir(), '.local', 'share', 'agent-kanban');
  
  return path.join(baseDir, 'spool');
}

function detectAgentType() {
  const args = process.argv.slice(2);
  if (args.includes('--agent=cursor')) return 'cursor';
  if (args.includes('--agent=claude')) return 'claude';
  if (process.env.CLAUDE_SESSION_ID) return 'claude';
  return 'cursor';
}

const CURSOR_EVENT_MAP = {
  'beforeShellExecution': 'command_requested',
  'afterShellExecution': 'command_executed',
  'beforeReadFile': 'file_read',
  'afterFileEdit': 'file_edited',
  'beforeMCPExecution': 'command_requested',
  'stop': 'run_stopped',
  'beforeSubmitPrompt': 'prompt_submitted',
};

const CLAUDE_EVENT_MAP = {
  'UserPromptSubmit': 'prompt_submitted',
  'PreToolUse': 'command_requested',
  'PostToolUse': 'command_executed',
  'PostToolUseFailure': 'error',
  'Stop': 'run_stopped',
  'SessionStart': 'run_started',
  'SessionEnd': 'run_stopped',
};

function mapEventType(rawEvent, agentType) {
  const map = agentType === 'cursor' ? CURSOR_EVENT_MAP : CLAUDE_EVENT_MAP;
  return map[rawEvent] || rawEvent.toLowerCase();
}

function extractStructuredData(eventType, rawEvent, payload, agentType) {
  if (agentType === 'cursor') {
    return extractCursorData(rawEvent, payload);
  } else {
    return extractClaudeData(rawEvent, payload);
  }
}

function extractCursorData(eventType, payload) {
  switch (eventType) {
    case 'beforeShellExecution':
      return {
        command: payload.command,
        workingDirectory: payload.cwd,
      };
    
    case 'afterFileEdit':
      return {
        filePath: payload.path,
        oldContent: payload.oldContent?.substring(0, 500),
        newContent: payload.newContent?.substring(0, 500),
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

function extractClaudeData(eventType, payload) {
  const tool = payload.tool_name || '';
  const input = payload.tool_input || {};

  switch (eventType) {
    case 'PreToolUse':
    case 'PostToolUse':
      if (tool === 'Bash') {
        return {
          tool: 'bash',
          command: input.command,
          timeout: input.timeout,
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
    
    case 'Stop':
      return {
        reason: payload.stop_reason,
        transcriptPath: payload.transcript_path,
      };
    
    default:
      return payload;
  }
}

function normalizeEvent(rawEventType, payload) {
  const eventType = mapEventType(rawEventType, CONFIG.agentType);
  const structured = extractStructuredData(rawEventType, rawEventType, payload, CONFIG.agentType);

  return {
    runId: CONFIG.runId,
    ticketId: CONFIG.ticketId,
    agentType: CONFIG.agentType,
    eventType,
    payload: {
      raw: JSON.stringify(payload),
      structured,
    },
    timestamp: new Date().toISOString(),
  };
}

async function postEvent(event) {
  if (!CONFIG.runId || !CONFIG.apiToken) {
    console.error('Missing RUN_ID or API_TOKEN');
    return false;
  }

  const url = `${CONFIG.apiUrl}/v1/runs/${CONFIG.runId}/events`;
  
  for (let attempt = 0; attempt < CONFIG.maxRetries; attempt++) {
    try {
      await httpRequest('POST', url, {
        eventType: event.eventType,
        payload: event.payload,
        timestamp: event.timestamp,
      });
      return true;
    } catch (error) {
      if (attempt < CONFIG.maxRetries - 1) {
        await sleep(CONFIG.retryDelayMs * (attempt + 1));
      }
    }
  }
  
  return false;
}

async function updateRunStatus(status, exitCode, summary) {
  if (!CONFIG.runId || !CONFIG.apiToken) return;

  const url = `${CONFIG.apiUrl}/v1/runs/${CONFIG.runId}`;
  
  try {
    await httpRequest('PATCH', url, {
      status,
      exitCode,
      summaryMd: summary,
    });
  } catch (error) {
    console.error('Failed to update run status:', error.message);
  }
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
        'X-AgentKanban-Token': CONFIG.apiToken,
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

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function ensureSpoolDir() {
  if (!fs.existsSync(CONFIG.spoolDir)) {
    fs.mkdirSync(CONFIG.spoolDir, { recursive: true });
  }
}

function spoolEvent(event) {
  ensureSpoolDir();
  
  const filename = `${Date.now()}-${Math.random().toString(36).substr(2, 9)}.json`;
  const filepath = path.join(CONFIG.spoolDir, filename);
  
  fs.writeFileSync(filepath, JSON.stringify(event, null, 2));
  console.error(`Event spooled to ${filepath}`);
}

async function processSpooledEvents() {
  if (!fs.existsSync(CONFIG.spoolDir)) return;

  const files = fs.readdirSync(CONFIG.spoolDir)
    .filter(f => f.endsWith('.json'))
    .sort();

  for (const file of files) {
    const filepath = path.join(CONFIG.spoolDir, file);
    
    try {
      const content = fs.readFileSync(filepath, 'utf8');
      const event = JSON.parse(content);
      
      const success = await postEvent(event);
      if (success) {
        fs.unlinkSync(filepath);
      }
    } catch (error) {
      console.error(`Failed to process spooled event ${file}:`, error.message);
    }
  }
}

async function handleHook(rawEventType, payload) {
  const event = normalizeEvent(rawEventType, payload);
  const success = await postEvent(event);
  if (!success) spoolEvent(event);
  await processSpooledEvents().catch(() => {});
  return getHookResponse(rawEventType, payload);
}

function getHookResponse(eventType, payload) {
  if (CONFIG.agentType === 'cursor') {
    if (eventType === 'beforeShellExecution') {
      const command = payload.command || '';
      if (isDangerousCommand(command)) {
        return {
          continue: false,
          permission: 'deny',
          userMessage: 'Blocked by Agent Kanban for safety',
          agentMessage: 'This command was blocked. Please use a safer alternative.',
        };
      }
      return { continue: true, permission: 'allow' };
    }
    
    return { continue: true };
  }
  
  if (CONFIG.agentType === 'claude') {
    if (eventType === 'UserPromptSubmit') return getClaudeContext();
    if (eventType === 'PreToolUse') {
      const tool = payload.tool_name || '';
      const input = payload.tool_input || {};
      if (tool === 'Bash' && isDangerousCommand(input.command || '')) {
        console.error('Blocked dangerous command:', input.command);
        process.exit(2);
      }
    }
    return null;
  }
  
  return null;
}

function isDangerousCommand(command) {
  const patterns = [
    /rm\s+-rf\s+\//,
    /rm\s+-rf\s+~\//,
    /git\s+push\s+.*--force/,
    /sudo\s+rm/,
    /mkfs\./,
    /dd\s+if=.*of=\/dev/,
    /:\(\)\{\s*:\|:&\s*\};:/,
  ];
  
  return patterns.some(p => p.test(command));
}

function getClaudeContext() {
  if (!CONFIG.ticketId) return '';
  
  return `
## Agent Kanban Context

Working on ticket: ${CONFIG.ticketId}
Run ID: ${CONFIG.runId}

Your actions are being tracked. Please:
1. Focus on completing the task
2. Make incremental changes
3. Commit with descriptive messages
`;
}

// Handle stop events specially
async function handleStopEvent(payload) {
  let status = 'finished';
  let exitCode = 0;
  let summary = 'Completed successfully.';

  if (CONFIG.agentType === 'cursor') {
    const cursorStatus = payload.status || '';
    if (cursorStatus === 'error' || cursorStatus === 'aborted') {
      status = cursorStatus === 'aborted' ? 'aborted' : 'error';
      exitCode = 1;
      summary = `Stopped: ${cursorStatus}`;
    }
  } else {
    const reason = payload.stop_reason || '';
    if (reason === 'error' || reason === 'tool_error') {
      status = 'error';
      exitCode = 1;
      summary = `Error: ${reason}`;
    } else if (reason === 'user_cancelled') {
      status = 'aborted';
      exitCode = 130;
      summary = 'Cancelled by user.';
    }
  }

  await updateRunStatus(status, exitCode, summary);
}

async function main() {
  const eventType = process.argv[2];
  if (!eventType) {
    console.error('Usage: agent-kanban-hook.js <event-type>');
    process.exit(1);
  }

  let inputData = '';
  process.stdin.setEncoding('utf8');
  for await (const chunk of process.stdin) {
    inputData += chunk;
  }

  try {
    const payload = inputData ? JSON.parse(inputData) : {};

    if (eventType.toLowerCase() === 'stop' || eventType === 'Stop') {
      await handleStopEvent(payload);
    }
    const response = await handleHook(eventType, payload);
    if (response) {
      console.log(typeof response === 'string' ? response : JSON.stringify(response));
    }
    process.exit(0);
  } catch (error) {
    console.error('Hook error:', error.message);
    console.log(JSON.stringify({ continue: true }));
    process.exit(0);
  }
}

main();
