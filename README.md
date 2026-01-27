# Bored

A local-first desktop application for managing coding tasks with AI agents. Create tickets on a Kanban board and let Cursor or Claude Code automatically work on them.

## What It Does

Bored provides a visual Kanban board where you can:

- **Create and manage coding tickets** with descriptions, priorities, and labels
- **Assign tickets to AI agents** (Cursor or Claude Code) that automatically work on your codebase
- **Track agent progress** in real-time through lifecycle hooks and events
- **Run workers** that continuously process tickets from the queue
- **Organize work across multiple projects** with per-project settings and agent preferences

## Screenshots

*Coming soon*

## Installation

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) 1.70+
- [Cursor](https://cursor.sh/) and/or [Claude Code](https://claude.ai/code) installed

### Build from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/bored.git
cd bored

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## Getting Started

1. **Create a Board** - Click "Create Your First Board" to set up a Kanban board
2. **Add a Project** - Go to Settings > Projects and add a local repository path
3. **Create a Ticket** - Click "New Ticket" and describe a coding task
4. **Run an Agent** - Open a ticket and click "Run with Cursor" or "Run with Claude"
5. **Watch it Work** - Monitor agent progress in the ticket timeline

## Features

### Kanban Board

Drag-and-drop tickets between columns:
- **Backlog** - Future work, not ready for agents
- **Ready** - Queued for agent pickup
- **In Progress** - Currently being worked by an agent
- **Blocked** - Failed or needs attention
- **Review** - Completed, awaiting approval
- **Done** - Finished

### Agent Integration

Spawn AI coding agents directly from tickets:
- **Cursor Agent** - Uses Cursor's agent mode to work on tasks
- **Claude Code** - Uses Claude's CLI to work on tasks

Agents receive the ticket description as their prompt and work in the associated project directory.

### Workers

Automated workers continuously process tickets:
- Poll for tickets in the Ready column
- Lock tickets during processing to prevent conflicts
- Send heartbeats to maintain locks
- Automatically transition tickets based on outcomes
- Recover expired locks for orphaned tickets

### Project Management

Register local repositories as projects:
- Set preferred agent (Cursor, Claude, or any)
- Configure safety settings (shell commands, file writes)
- Install agent hooks per-project
- Block specific file patterns

### Real-time Events

Track agent activity through the event timeline:
- File edits and reads
- Shell commands executed
- Run status changes
- Error messages

Events are streamed via Server-Sent Events (SSE) from the local API.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Desktop App                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │
│  │  React UI   │  │ Tauri/Rust  │  │   Local API     │ │
│  │  (Vite)     │◄─►│  Backend    │◄─►│   (Axum)       │ │
│  └─────────────┘  └─────────────┘  └─────────────────┘ │
│                          │                    ▲         │
│                          ▼                    │         │
│                    ┌──────────┐               │         │
│                    │  SQLite  │               │         │
│                    └──────────┘               │         │
└─────────────────────────────────────────────────────────┘
                           │
            ┌──────────────┴──────────────┐
            ▼                              ▼
    ┌───────────────┐              ┌───────────────┐
    │ Cursor Agent  │              │  Claude Code  │
    │               │              │               │
    │  Hook Script  │─────────────►│  Hook Script  │───────►
    └───────────────┘   POST to    └───────────────┘ Events
                        Local API
```

## Technology Stack

| Component | Technology |
|-----------|------------|
| Desktop Framework | Tauri 1.x |
| Frontend | React 18 + TypeScript |
| Build Tool | Vite |
| Styling | Tailwind CSS 4 |
| State Management | Zustand |
| Drag & Drop | dnd-kit |
| Backend Runtime | Rust |
| HTTP Server | Axum |
| Database | SQLite (rusqlite) |
| Async Runtime | Tokio |

## Project Structure

```
bored/
├── src/                      # React frontend
│   ├── components/
│   │   ├── board/           # Kanban board components
│   │   ├── common/          # Shared UI components
│   │   ├── layout/          # App layout (sidebar, header)
│   │   ├── runs/            # Agent run views
│   │   ├── settings/        # Settings panels
│   │   ├── timeline/        # Event timeline
│   │   └── workers/         # Worker management
│   ├── hooks/               # React hooks
│   ├── stores/              # Zustand state stores
│   ├── types/               # TypeScript types
│   └── lib/                 # Utilities and API
├── src-tauri/               # Rust backend
│   ├── src/
│   │   ├── agents/          # Agent orchestration
│   │   ├── api/             # HTTP API server
│   │   ├── commands/        # Tauri IPC commands
│   │   ├── db/              # Database layer
│   │   └── lifecycle/       # Ticket state machine
│   └── scripts/             # Hook scripts for agents
└── scripts/                 # Shared hook scripts
```

## Configuration

### Agent Hooks

Agent hooks intercept lifecycle events and report them to the application:

**Cursor Hooks:**
- `beforeShellExecution` - Before running shell commands
- `afterFileEdit` - After editing files
- `stop` - When the agent stops

**Claude Hooks:**
- `PreToolUse` / `PostToolUse` - Before/after tool calls
- `Stop` - When the agent stops
- `UserPromptSubmit` - When prompts are submitted

### Settings

Access settings through the sidebar:
- **General** - Theme (light/dark/system)
- **Projects** - Manage registered repositories
- **Cursor** - Cursor agent configuration
- **Claude Code** - Claude agent configuration
- **Data** - Database management

## Development

```bash
# Run tests
npm run test

# Run tests in watch mode
npm run test:watch

# Run the app in development
npm run tauri dev
```

## License

Copyright (c) 2026 Tanner Burns. All rights reserved.

This software is proprietary and confidential. See [LICENSE.txt](LICENSE.txt) for details.
