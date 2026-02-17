#!/usr/bin/env node
/**
 * Hook script for Agent Kanban.
 *
 * Forwards raw agent events to the API server for normalization and
 * action decisions. Response wrapping at the bottom is the only
 * agent-specific code.
 */

const http = require('http');
const https = require('https');
const fs = require('fs');
const path = require('path');
const os = require('os');

// ── Configuration (all from environment) ───────────────────────────

const CONFIG = {
  apiUrl: process.env.AGENT_KANBAN_API_URL || 'http://127.0.0.1:7432',
  apiToken: process.env.AGENT_KANBAN_API_TOKEN,
  runId: process.env.AGENT_KANBAN_RUN_ID,
  ticketId: process.env.AGENT_KANBAN_TICKET_ID,
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
  for (const arg of args) {
    const match = arg.match(/^--agent=(.+)$/);
    if (match) return match[1];
  }
  if (process.env.CLAUDE_SESSION_ID) return 'claude';
  return 'cursor';
}

// ── HTTP helpers ───────────────────────────────────────────────────

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

    if (data) req.write(JSON.stringify(data));
    req.end();
  });
}

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

// ── Offline spooling ───────────────────────────────────────────────

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
  const files = fs.readdirSync(CONFIG.spoolDir).filter(f => f.endsWith('.json')).sort();

  for (const file of files) {
    const filepath = path.join(CONFIG.spoolDir, file);
    try {
      const content = fs.readFileSync(filepath, 'utf8');
      const event = JSON.parse(content);
      const result = await postToApi(event);
      if (result.ok) fs.unlinkSync(filepath);
    } catch (error) {
      console.error(`Failed to process spooled event ${file}:`, error.message);
    }
  }
}

// ── Core: post raw event to API and get action back ────────────────

async function postToApi(body) {
  const url = `${CONFIG.apiUrl}/v1/hooks/event`;

  for (let attempt = 0; attempt < CONFIG.maxRetries; attempt++) {
    try {
      const responseBody = await httpRequest('POST', url, body);
      return { ok: true, data: JSON.parse(responseBody) };
    } catch (error) {
      if (attempt < CONFIG.maxRetries - 1) {
        await sleep(CONFIG.retryDelayMs * (attempt + 1));
      }
    }
  }
  return { ok: false, data: null };
}

// ── Response wrapping ──────────────────────────────────────────────

function formatResponse(actionResponse, rawEventType) {
  if (!actionResponse) {
    // API unreachable — default to allowing the operation so the agent
    // isn't left waiting for a permission decision it will never get.
    if (CONFIG.agentType === 'cursor') return { continue: true };
    return null;
  }

  const { action } = actionResponse;

  if (CONFIG.agentType === 'cursor') {
    if (action === 'deny') {
      return {
        continue: false,
        permission: 'deny',
        userMessage: actionResponse.reason || 'Blocked by Agent Kanban',
        agentMessage: actionResponse.reason || 'This operation was blocked.',
      };
    }
    return { continue: true };
  }

  if (CONFIG.agentType === 'claude') {
    if (action === 'deny') {
      console.error(actionResponse.reason || 'Blocked by Agent Kanban');
      process.exit(2);
    }
    if (action === 'inject_context') {
      return actionResponse.context || '';
    }
    return null;
  }

  // Unknown agent type — return the raw action for best-effort support
  return actionResponse;
}

// ── Main ───────────────────────────────────────────────────────────

async function main() {
  const rawEventType = process.argv[2];
  if (!rawEventType) {
    console.error('Usage: agent-kanban-hook.js <event-type>');
    process.exit(1);
  }

  let inputData = '';
  process.stdin.setEncoding('utf8');
  for await (const chunk of process.stdin) {
    inputData += chunk;
  }

  try {
    const rawPayload = inputData ? JSON.parse(inputData) : {};

    const body = {
      agentType: CONFIG.agentType,
      runId: CONFIG.runId || '',
      rawEventType,
      rawPayload,
      ticketId: CONFIG.ticketId || null,
      timestamp: new Date().toISOString(),
    };

    const result = await postToApi(body);

    if (!result.ok) {
      // API unreachable — spool and allow the operation to continue
      spoolEvent(body);
    }

    await processSpooledEvents().catch(() => {});

    const response = formatResponse(result.data, rawEventType);
    if (response) {
      console.log(typeof response === 'string' ? response : JSON.stringify(response));
    }
    process.exit(0);
  } catch (error) {
    console.error('Hook error:', error.message);
    // On error, allow the operation to continue
    if (CONFIG.agentType === 'cursor') {
      console.log(JSON.stringify({ continue: true }));
    }
    process.exit(0);
  }
}

main();
