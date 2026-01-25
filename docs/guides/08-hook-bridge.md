# 08 - Hook Bridge

Implement a unified event handling system that normalizes events from both Cursor and Claude Code into a consistent format.

## Overview

This guide covers:

- Normalized event schema design
- Event type mapping from Cursor and Claude Code
- Unified hook script template
- Reliability features: local spooling and retry logic
- Event timeline UI component

## Prerequisites

- Completed [06-cursor-integration.md](./06-cursor-integration.md)
- Completed [07-claude-code-integration.md](./07-claude-code-integration.md)

## Architecture

```mermaid
flowchart TB
    subgraph Agents[Agent Hooks]
        CursorHook[Cursor Hook Script]
        ClaudeHook[Claude Hook Script]
    end
    
    subgraph Bridge[Hook Bridge]
        Normalizer[Event Normalizer]
        Spool[Local Spool File]
        Uploader[Background Uploader]
    end
    
    subgraph API[Local API]
        Events[/v1/runs/:id/events]
        Runs[/v1/runs/:id]
    end
    
    CursorHook --> Normalizer
    ClaudeHook --> Normalizer
    Normalizer --> Events
    Normalizer -.->|on failure| Spool
    Spool --> Uploader
    Uploader --> Events
```

## Normalized Event Schema

All events from both agents are normalized to this canonical format:

```typescript
interface NormalizedEvent {
  // Identifiers
  runId: string;
  ticketId: string;
  agentType: 'cursor' | 'claude';
  
  // Event details
  eventType: EventType;
  payload: {
    raw?: string;           // Original event JSON
    structured?: object;    // Parsed/structured data
  };
  
  // Metadata
  timestamp: string;        // ISO 8601
}

type EventType = 
  | 'command_requested'     // Before shell command
  | 'command_executed'      // After shell command
  | 'file_read'            // File was read
  | 'file_edited'          // File was created/modified
  | 'run_started'          // Agent run started
  | 'run_stopped'          // Agent run ended
  | 'error'                // Error occurred
  | string;                // Custom event types
```

## Implementation Steps

### Step 1: Create Unified Hook Script

Create `scripts/agent-kanban-hook.js`:

```javascript
#!/usr/bin/env node
/**
 * Unified Agent Kanban Hook Script
 * 
 * Works with both Cursor and Claude Code hooks.
 * Normalizes events and posts them to the local API.
 * Includes reliability features for offline/error scenarios.
 */

const https = require('https');
const http = require('http');
const fs = require('fs');
const path = require('path');
const os = require('os');

// ============ Configuration ============

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
  // Detect based on environment or calling convention
  const args = process.argv.slice(2);
  if (args.includes('--agent=cursor')) return 'cursor';
  if (args.includes('--agent=claude')) return 'claude';
  
  // Check for Claude-specific env vars
  if (process.env.CLAUDE_SESSION_ID) return 'claude';
  
  return 'cursor'; // Default
}

// ============ Event Type Mapping ============

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

// ============ Structured Data Extraction ============

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

// ============ Event Normalization ============

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

// ============ API Communication ============

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

// ============ Spool Management ============

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

// ============ Hook Handlers ============

async function handleHook(rawEventType, payload) {
  // Normalize the event
  const event = normalizeEvent(rawEventType, payload);
  
  // Post to API
  const success = await postEvent(event);
  
  // If failed, spool for later
  if (!success) {
    spoolEvent(event);
  }
  
  // Process any spooled events in background
  processSpooledEvents().catch(() => {});
  
  // Return appropriate response based on agent type and event
  return getHookResponse(rawEventType, payload);
}

function getHookResponse(eventType, payload) {
  // For Cursor hooks that support blocking
  if (CONFIG.agentType === 'cursor') {
    if (eventType === 'beforeShellExecution') {
      // Check for dangerous commands
      const command = payload.command || '';
      if (isDangerousCommand(command)) {
        return {
          continue: true,
          permission: 'deny',
          userMessage: 'Blocked by Agent Kanban for safety',
          agentMessage: 'This command was blocked. Please use a safer alternative.',
        };
      }
      return { continue: true, permission: 'allow' };
    }
    
    return { continue: true };
  }
  
  // For Claude hooks
  if (CONFIG.agentType === 'claude') {
    if (eventType === 'UserPromptSubmit') {
      // Inject context
      return getClaudeContext();
    }
    
    if (eventType === 'PreToolUse') {
      const tool = payload.tool_name || '';
      const input = payload.tool_input || {};
      
      if (tool === 'Bash' && isDangerousCommand(input.command || '')) {
        // Return error to stderr and exit 2
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

// ============ Main Entry Point ============

async function main() {
  const eventType = process.argv[2];
  
  if (!eventType) {
    console.error('Usage: agent-kanban-hook.js <event-type>');
    process.exit(1);
  }

  // Read input from stdin
  let inputData = '';
  
  process.stdin.setEncoding('utf8');
  
  for await (const chunk of process.stdin) {
    inputData += chunk;
  }

  const payload = inputData ? JSON.parse(inputData) : {};

  try {
    // Handle stop events specially
    if (eventType.toLowerCase() === 'stop' || eventType === 'Stop') {
      await handleStopEvent(payload);
    }

    // Handle the hook and get response
    const response = await handleHook(eventType, payload);
    
    // Output response
    if (response) {
      if (typeof response === 'string') {
        console.log(response);
      } else {
        console.log(JSON.stringify(response));
      }
    }

    process.exit(0);
  } catch (error) {
    console.error('Hook error:', error.message);
    // Don't block on errors
    console.log(JSON.stringify({ continue: true }));
    process.exit(0);
  }
}

main();
```

### Step 2: Create Event Timeline Component

Create `src/components/timeline/EventTimeline.tsx`:

```typescript
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { formatDistanceToNow } from 'date-fns';

interface AgentEvent {
  id: string;
  runId: string;
  ticketId: string;
  eventType: string;
  payload: {
    raw?: string;
    structured?: Record<string, any>;
  };
  createdAt: string;
}

interface EventTimelineProps {
  runId: string;
}

export function EventTimeline({ runId }: EventTimelineProps) {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    loadEvents();
    
    // Poll for new events
    const interval = setInterval(loadEvents, 2000);
    return () => clearInterval(interval);
  }, [runId]);

  const loadEvents = async () => {
    try {
      const data = await invoke<AgentEvent[]>('get_run_events', { runId });
      setEvents(data);
    } catch (error) {
      console.error('Failed to load events:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const getEventIcon = (eventType: string) => {
    switch (eventType) {
      case 'command_requested':
        return '⌘';
      case 'command_executed':
        return '✓';
      case 'file_read':
        return '📖';
      case 'file_edited':
        return '✏️';
      case 'run_started':
        return '▶️';
      case 'run_stopped':
        return '⏹';
      case 'error':
        return '❌';
      default:
        return '•';
    }
  };

  const getEventColor = (eventType: string) => {
    switch (eventType) {
      case 'command_requested':
        return 'border-blue-500';
      case 'command_executed':
        return 'border-green-500';
      case 'file_edited':
        return 'border-yellow-500';
      case 'error':
        return 'border-red-500';
      case 'run_stopped':
        return 'border-gray-500';
      default:
        return 'border-gray-600';
    }
  };

  const formatPayload = (payload: AgentEvent['payload']) => {
    if (payload.structured) {
      const s = payload.structured;
      
      if (s.command) {
        return (
          <code className="text-xs bg-gray-800 px-2 py-1 rounded block mt-1 overflow-x-auto">
            {s.command}
          </code>
        );
      }
      
      if (s.filePath) {
        return (
          <span className="text-xs text-gray-400">
            {s.tool || 'file'}: <code>{s.filePath}</code>
          </span>
        );
      }
      
      if (s.reason) {
        return (
          <span className="text-xs text-gray-400">
            Reason: {s.reason}
          </span>
        );
      }
    }
    
    return null;
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-8">
        <div className="animate-spin rounded-full h-6 w-6 border-b-2 border-white"></div>
      </div>
    );
  }

  if (events.length === 0) {
    return (
      <div className="text-center py-8 text-gray-500">
        No events yet
      </div>
    );
  }

  return (
    <div className="space-y-0">
      {events.map((event, index) => (
        <div key={event.id} className="relative pl-6 pb-4">
          {/* Vertical line */}
          {index < events.length - 1 && (
            <div className="absolute left-2 top-4 bottom-0 w-px bg-gray-700"></div>
          )}
          
          {/* Event dot */}
          <div className={`absolute left-0 top-1 w-4 h-4 rounded-full border-2 bg-gray-900 ${getEventColor(event.eventType)} flex items-center justify-center text-xs`}>
          </div>
          
          {/* Event content */}
          <div className="bg-gray-800 rounded p-3">
            <div className="flex items-center gap-2 mb-1">
              <span>{getEventIcon(event.eventType)}</span>
              <span className="font-medium text-sm">
                {event.eventType.replace(/_/g, ' ')}
              </span>
              <span className="text-xs text-gray-500 ml-auto">
                {formatDistanceToNow(new Date(event.createdAt))} ago
              </span>
            </div>
            
            {formatPayload(event.payload)}
          </div>
        </div>
      ))}
    </div>
  );
}
```

### Step 3: Add Event API Command

Add to `src-tauri/src/commands/mod.rs`:

```rust
#[tauri::command]
pub async fn get_run_events(
    run_id: String,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<crate::db::AgentEvent>, String> {
    db.get_events(&run_id).map_err(|e| e.to_string())
}
```

### Step 4: Create Spool Processing Service

Add to `src-tauri/src/api/spool.rs`:

```rust
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::interval;

use crate::db::{Database, NormalizedEvent, AgentEventPayload, EventType, AgentType};
use std::sync::Arc;

/// Process spooled events in the background
pub async fn start_spool_processor(db: Arc<Database>, spool_dir: PathBuf) {
    let mut ticker = interval(Duration::from_secs(30));
    
    loop {
        ticker.tick().await;
        
        if let Err(e) = process_spool(&db, &spool_dir).await {
            tracing::error!("Spool processing error: {}", e);
        }
    }
}

async fn process_spool(db: &Database, spool_dir: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if !spool_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(spool_dir)?;
    
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            match process_spool_file(db, &path) {
                Ok(()) => {
                    fs::remove_file(&path)?;
                    tracing::debug!("Processed spooled event: {:?}", path);
                }
                Err(e) => {
                    tracing::warn!("Failed to process spooled event {:?}: {}", path, e);
                }
            }
        }
    }

    Ok(())
}

fn process_spool_file(db: &Database, path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let content = fs::read_to_string(path)?;
    let event: serde_json::Value = serde_json::from_str(&content)?;
    
    let normalized = NormalizedEvent {
        run_id: event["runId"].as_str().unwrap_or("").to_string(),
        ticket_id: event["ticketId"].as_str().unwrap_or("").to_string(),
        agent_type: match event["agentType"].as_str() {
            Some("claude") => AgentType::Claude,
            _ => AgentType::Cursor,
        },
        event_type: EventType::from_str(event["eventType"].as_str().unwrap_or("")),
        payload: AgentEventPayload {
            raw: event["payload"]["raw"].as_str().map(|s| s.to_string()),
            structured: event["payload"]["structured"].as_object().map(|o| {
                serde_json::Value::Object(o.clone())
            }),
        },
        timestamp: chrono::DateTime::parse_from_rfc3339(
            event["timestamp"].as_str().unwrap_or("")
        )
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now()),
    };

    db.create_event(&normalized)?;
    Ok(())
}
```

### Step 5: Create Run Details Panel

Create `src/components/runs/RunDetailsPanel.tsx`:

```typescript
import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/tauri';
import { EventTimeline } from '../timeline/EventTimeline';
import type { AgentRun } from '../../types';

interface RunDetailsPanelProps {
  runId: string;
  onClose: () => void;
}

export function RunDetailsPanel({ runId, onClose }: RunDetailsPanelProps) {
  const [run, setRun] = useState<AgentRun | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [activeTab, setActiveTab] = useState<'timeline' | 'logs'>('timeline');

  useEffect(() => {
    loadRun();
  }, [runId]);

  const loadRun = async () => {
    try {
      const data = await invoke<AgentRun>('get_run', { runId });
      setRun(data);
    } catch (error) {
      console.error('Failed to load run:', error);
    } finally {
      setIsLoading(false);
    }
  };

  if (isLoading || !run) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
      </div>
    );
  }

  const statusColors = {
    queued: 'bg-gray-500',
    running: 'bg-yellow-500',
    finished: 'bg-green-500',
    error: 'bg-red-500',
    aborted: 'bg-gray-600',
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between p-4 border-b border-gray-700">
        <div>
          <h3 className="font-semibold">Run {run.id.substring(0, 8)}</h3>
          <div className="flex items-center gap-2 mt-1">
            <span className={`px-2 py-0.5 text-xs rounded ${statusColors[run.status]} text-white`}>
              {run.status}
            </span>
            <span className="text-sm text-gray-400">
              {run.agentType === 'cursor' ? 'Cursor' : 'Claude'}
            </span>
          </div>
        </div>
        <button onClick={onClose} className="p-1 text-gray-400 hover:text-white">
          ✕
        </button>
      </div>

      {/* Tabs */}
      <div className="flex border-b border-gray-700">
        <button
          onClick={() => setActiveTab('timeline')}
          className={`px-4 py-2 text-sm ${
            activeTab === 'timeline'
              ? 'border-b-2 border-blue-500 text-white'
              : 'text-gray-400 hover:text-white'
          }`}
        >
          Timeline
        </button>
        <button
          onClick={() => setActiveTab('logs')}
          className={`px-4 py-2 text-sm ${
            activeTab === 'logs'
              ? 'border-b-2 border-blue-500 text-white'
              : 'text-gray-400 hover:text-white'
          }`}
        >
          Logs
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {activeTab === 'timeline' ? (
          <EventTimeline runId={runId} />
        ) : (
          <div className="font-mono text-xs text-gray-300 whitespace-pre-wrap">
            {/* Logs would be populated here */}
            <p className="text-gray-500">Log output will appear here during execution.</p>
          </div>
        )}
      </div>

      {/* Summary */}
      {run.summaryMd && (
        <div className="p-4 border-t border-gray-700">
          <h4 className="text-sm font-medium text-gray-400 mb-2">Summary</h4>
          <p className="text-sm text-gray-300">{run.summaryMd}</p>
        </div>
      )}
    </div>
  );
}
```

## Event Type Reference

### Canonical Event Types

| Event Type | Description | Sources |
|------------|-------------|---------|
| `command_requested` | Before shell command executes | Cursor: `beforeShellExecution`, Claude: `PreToolUse(Bash)` |
| `command_executed` | After shell command completes | Claude: `PostToolUse(Bash)` |
| `file_read` | File was read | Cursor: `beforeReadFile`, Claude: `PreToolUse(Read)` |
| `file_edited` | File was created/modified | Cursor: `afterFileEdit`, Claude: `PostToolUse(Edit/Write)` |
| `run_started` | Agent run began | Cursor: start, Claude: `SessionStart` |
| `run_stopped` | Agent run ended | Cursor: `stop`, Claude: `Stop` |
| `error` | Error occurred | Claude: `PostToolUseFailure` |
| `prompt_submitted` | User submitted prompt | Cursor: `beforeSubmitPrompt`, Claude: `UserPromptSubmit` |

### Payload Structure Examples

**command_requested:**
```json
{
  "raw": "{\"command\":\"npm install\",\"cwd\":\"/project\"}",
  "structured": {
    "command": "npm install",
    "workingDirectory": "/project"
  }
}
```

**file_edited:**
```json
{
  "raw": "{\"path\":\"src/index.ts\",\"tool\":\"edit\"}",
  "structured": {
    "tool": "edit",
    "filePath": "src/index.ts"
  }
}
```

## Testing

### Test Unified Hook Script

```bash
# Set environment
export AGENT_KANBAN_RUN_ID="test-run"
export AGENT_KANBAN_TICKET_ID="test-ticket"
export AGENT_KANBAN_API_TOKEN="token"
export AGENT_KANBAN_AGENT_TYPE="cursor"

# Test command event
echo '{"command":"ls -la"}' | node scripts/agent-kanban-hook.js beforeShellExecution

# Test file edit
echo '{"path":"test.ts"}' | node scripts/agent-kanban-hook.js afterFileEdit

# Test with Claude type
export AGENT_KANBAN_AGENT_TYPE="claude"
echo '{"tool_name":"Bash","tool_input":{"command":"echo hello"}}' | \
  node scripts/agent-kanban-hook.js PreToolUse
```

### Verify Event Flow

1. Start an agent run
2. Watch the event timeline update in real-time
3. Check events are normalized correctly
4. Verify spooled events are processed when API becomes available

## Troubleshooting

### Events not appearing

1. Check hook script is receiving events (add logging)
2. Verify API is accessible from hook script
3. Check for spooled events in spool directory

### Incorrect event types

1. Verify agent type detection is correct
2. Check event type mapping tables
3. Review structured data extraction

### Spool not processing

1. Check spool directory permissions
2. Verify spool processor is running
3. Check for malformed JSON in spool files

## Next Steps

With the hook bridge complete, proceed to:

- **[09-worker-mode.md](./09-worker-mode.md)**: Implement queue semantics for automated agent runs
