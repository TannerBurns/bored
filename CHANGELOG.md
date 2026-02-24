# Changelog

All notable changes to Bored are documented in this file.

## [0.1.0-beta.27] - 2026-02-24

Bug fix release improving spec agent JSON extraction reliability and model override cost attribution accuracy.

### Improvements

- Auto-pilot command selections persisted to run metadata and displayed in RunsHistory UI after workflow completion
- Agent config stored in sub-run metadata enabling cost backfill to apply local provider overrides retroactively
- StructuredSpec requirements and technical_notes fields changed from String to Vec<String> for discrete, actionable items that avoid embedded code fences
- Extracted bullet_list helper consolidating repeated formatting patterns in spec completion
- Dashboard model breakdown adds "Others" bucket when 8+ models are present

### Bug Fixes

- Fixed spec agent silently producing duplicate spec JSON without creating a spec — extract_json_code_block now uses brace-counting inside json fences instead of string-searching for closing backticks, which failed on code examples embedded in field values
- Fixed StructuredSpec deserialization failures when agents return a plain string instead of an array for requirements or technical_notes — added flexible string-or-array deserializer matching the existing depends_on pattern
- Fixed model override cost re-keying — when users configure local provider overrides (Ollama, vLLM), model_usage entries are now re-keyed to the user-configured override model so costs are attributed correctly in dashboards and reports
- Fixed dashboard cost aggregation inconsistencies — summary and trends now prefer per-model sums over total_cost_usd when model_usage entries disagree

### Testing

- Added JSON extraction tests for nested backticks, non-JSON fence fallback, and unbalanced brace recovery
- Added StructuredSpec schema tests for empty arrays, order preservation, string-or-array deserialization, and null handling
- Added cost re-keying tests for local provider model override scenarios
- Added merge_run_metadata and auto-pilot selection persistence tests

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.26, here is a summary of the major features introduced in recent releases:

**beta.26 — Dashboard & Board/List View**
Dashboard landing page with summary stats, trend charts (activity, cost, tokens), model cost breakdown, and per-ticket git stats. Board/list view toggle with column select dropdown. Dynamic Cursor model sync from CLI output.

**beta.25 — Auto-Pilot Workflow & Command-Based Tasks**
Auto-pilot workflow mode where the agent dynamically decides which commands to run after implementation. Extensible command-based task system backed by the command catalog. Codex reasoning effort and multi-agent toggle.

**beta.24 — System Tray & Notifications**
System tray integration with recent tickets list and native OS notifications when tickets move to Review or Blocked. Notification toggle in General Settings.

**beta.23 — Catalog-Driven Commands**
Custom and built-in workflow commands managed through a discoverable command catalog. Create, edit, and delete custom commands per agent with file-backed persistence.

---

## [0.1.0-beta.26] - 2026-02-23

Dashboard landing page, board/list view toggle, dynamic Cursor model sync, and dead code cleanup.

### New Features

- Dashboard landing page — summary stats, trend charts (activity, cost, tokens), model cost breakdown, agent distribution pie chart, and per-ticket git stats (commits, PRs, lines changed); default navigation changed from boards to dashboard
- Board/list view toggle — switch between kanban board and sortable table view per board with localStorage persistence
- Column select dropdown — move tickets between columns inline from list view rows or ticket modal header
- Dynamic Cursor model sync — model list sourced from `cursor agent --list-models` CLI output instead of a hardcoded list; syncs on first load and via Refresh Models button
- Cycle time, cost/ticket, and avg run time stat cards on the dashboard for efficiency visibility

### Improvements

- Sub-run cost aggregation for multi-stage parent runs now visible in the UI
- Shared column color constants (getColumnColors, getColumnBg, getColumnGlow) replacing duplicated switch statements
- Cursor thinking toggle removed — users now select thinking vs non-thinking model variants directly from the synced model dropdown
- REST API surface reduced to health + SSE stream endpoints (removed obsolete api.ts, useSSE.ts, useBoard.ts, useTauri.ts)
- Removed dead components (BoardProjectSelector, TicketProjectSelector) and orphaned Tauri command wrappers
- Tightened Rust export visibility — MULTI_STAGE_WORKFLOW scoped to #[cfg(test)], removed unnecessary re-exports from agents::mod
- DB migration v16 adds ticket_git_stats table and cleans up thinkingEnabled settings

### Bug Fixes

- Fixed git stats backfill repeatedly processing the same tickets when all stat values were zero — now always upserts a row
- Fixed null-to-undefined coercion in dashboard data hook breaking the "all time" time-range parameter contract
- Fixed git diff range syntax mismatch (three-dot vs two-dot) between commit count and line stats
- Fixed off-by-one in dashboard trends date bucketing dropping data from the boundary date
- Fixed model breakdown run_count inflation when a single run used multiple models

### Testing

- Added 19 Rust tests for dashboard queries (parse_cost, time_filter_clause, summary, trends, model/agent breakdown)
- Added 5 Rust tests for git_stats DB operations (upsert, increment_pr_count)
- Added 27 TypeScript tests for dashboard format helpers (formatCost, formatNumber, formatDuration, formatDateLabel)
- Added 8 TypeScript tests for useDashboardData hook
- Added 12 TypeScript tests for shared column color constants
- Added 9 TypeScript tests for ColumnSelect component
- Added 17 TypeScript tests for ListView component
- Added 6 Rust tests for sub-run cost aggregation
- Added Cursor model list parser tests covering empty, header-only, and multi-flag edge cases

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.25, here is a summary of the major features introduced in recent releases:

**beta.25 — Auto-Pilot Workflow & Command-Based Tasks**
Auto-pilot workflow mode where the agent dynamically decides which commands to run after implementation. Extensible command-based task system backed by the command catalog. Codex reasoning effort and multi-agent toggle.

**beta.24 — System Tray & Notifications**
System tray integration with recent tickets list and native OS notifications when tickets move to Review or Blocked. Notification toggle in General Settings.

**beta.23 — Catalog-Driven Commands**
Custom and built-in workflow commands managed through a discoverable command catalog. Create, edit, and delete custom commands per agent with file-backed persistence.

**beta.22 — Drag-and-Drop Stage Ordering**
Per-agent workflow stage ordering via drag-and-drop UI. Reorder optional stages independently for each agent. Preset selection resets ordering.

---

## [0.1.0-beta.25] - 2026-02-22

Auto-pilot workflow mode, extensible command-based tasks, and Codex CLI configuration options.

### New Features

- Auto-pilot workflow mode — the agent dynamically decides which commands (with model pairs) to run after implementation instead of following a static multi-stage pipeline; enabled via per-agent toggle in settings
- Extensible command-based task system — fixed preset task types (SyncWithMain, AddTests, etc.) replaced with a command-driven model backed by the command catalog, including custom commands
- Codex reasoning effort setting — configurable Low/Medium/High/xHigh selector in Codex CLI Options controlling how much reasoning the model performs before responding
- Codex multi-agent toggle — enable or disable the `features.multi_agent` Codex CLI flag from the settings UI
- Dynamic command discovery for auto-pilot — scans project-level and bundled command directories at runtime, deduplicates, and presents the full list to the agent for contextual selection
- Doc-sync command template added to the built-in command catalog

### Improvements

- Shared JSON extraction module consolidating duplicated JSON-from-agent-response parsing across brainstorm, planner, plan-validation, and auto-pilot subsystems
- StageRunner trait for dependency-injected agent spawning, enabling orchestrator integration testing without real child processes
- Run history shows "(Auto-Pilot)" vs "(Multi-Stage)" label based on workflow mode
- Stage configuration UI dims when auto-pilot is active
- Intent-aware command selection prompt with six example workflows (quick fix, standard feature, comprehensive, API change, trivial, refactor)
- DB migration v14→v15 automatically converts legacy preset task values to command format
- Frontend task dropdown now sources from the command catalog instead of a hardcoded backend endpoint

### Bug Fixes

- Fixed brainstorm parsing when `spec_complete` JSON isn't the first object in an unfenced agent response — added targeted key search with backward brace walk
- Fixed naive bracket extraction spanning across unrelated JSON objects — replaced with depth-counting `find_balanced` brace matching
- Fixed multi-byte UTF-8 character handling in `find_balanced` and bounds check in `skip_newline` preventing potential panics
- Fixed auto-pilot resume losing saved implementation output — now retrieves from `previous_stage_outputs` instead of returning an empty string
- Fixed `multiAgentEnabled` state sync when backend omits value — replaced conditional spread with nullish coalescing
- Fixed multi-agent toggle not actually disabling — now always sends the explicit `true`/`false` value to Codex CLI

### Testing

- Added 49 unit tests for shared JSON extraction module (code blocks, objects, arrays, multi-byte, balanced braces)
- Added 34 orchestrator integration tests covering mode derivation, resume logic, stage skip/enable, model override, retry, and cancellation
- Added 29 RunsHistory component tests and 26 AgentSettingsPage component tests
- Added settingsStore tests for auto-pilot toggle, v15 migration, and sync payload shape
- Added plan validation parsing tests for code-fence and bracket-finding fallback paths
- Added dynamic command discovery and filtering tests

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.24, here is a summary of the major features introduced in recent releases:

**beta.24 — System Tray & Notifications**
System tray integration with recent tickets list and native OS notifications when tickets move to Review or Blocked. Notification toggle in General Settings.

**beta.23 — Catalog-Driven Commands**
Custom and built-in workflow commands are now managed through a discoverable command catalog. Create, edit, and delete custom commands per agent with file-backed persistence. Built-in commands reconcile automatically on upgrade.

**beta.22 — Drag-and-Drop Stage Ordering**
Per-agent workflow stage ordering via drag-and-drop UI. Reorder optional stages (deslop, review, tests, cleanup) independently for each agent. Preset selection resets ordering; manual reorder switches to Custom preset.

**beta.21 — Local Provider Support**
Run Codex and Claude Code against self-hosted models via Ollama or LM Studio. Configurable base URL, model override, API key, and auth token fields. Zero-cost tracking for local provider runs.

---

## [0.1.0-beta.24] - 2026-02-21

System tray integration and native OS notifications for background ticket monitoring.

### New Features

- System tray with recent tickets list — shows the 3 most recent tickets directly in the menu with 5 more in a submenu, plus Open Bored, Open Settings, and Quit actions
- Native OS notifications when tickets move to Review or Blocked, so users are alerted to transitions requiring attention without keeping the app in the foreground
- Notification toggle in General Settings — persisted and synced to the backend via managed atomic state
- Branded B tray icon (template image for macOS menu bar)

### Improvements

- Tray menu rebuilds automatically on ticket move, create, and delete operations for live status updates
- Tray event listeners in the frontend handle navigation to settings and opening specific tickets from menu clicks
- Notification state gated by `NotificationsEnabled` atomic state in the backend, synced from the frontend toggle on hydration

### Bug Fixes

- Fixed `app_handle: None` in orchestrate.rs that was preventing tray refresh and notifications for UI-initiated runs

### Testing

- Added 6 unit tests for `truncate_title` edge cases
- Added 6 unit tests for `get_recent_tickets_with_columns` DB query in `query_tests.rs`

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.23, here is a summary of the major features introduced in recent releases:

**beta.23 — Catalog-Driven Commands**
Custom and built-in workflow commands are now managed through a discoverable command catalog. Create, edit, and delete custom commands per agent with file-backed persistence. Built-in commands reconcile automatically on upgrade.

**beta.22 — Drag-and-Drop Stage Ordering**
Per-agent workflow stage ordering via drag-and-drop UI. Reorder optional stages (deslop, review, tests, cleanup) independently for each agent. Preset selection resets ordering; manual reorder switches to Custom preset.

**beta.21 — Local Provider Support**
Run Codex and Claude Code against self-hosted models via Ollama or LM Studio. Configurable base URL, model override, API key, and auth token fields. Zero-cost tracking for local provider runs.

**beta.20 — Codex Agent & Per-Agent Settings**
Codex CLI agent available alongside Claude Code and Cursor with NDJSON output parsing and token-based cost tracking. Each agent now has independent workflow configuration, model selections, and sub-agent settings. Sonnet 4.6 model support added.

---

## [0.1.0-beta.23] - 2026-02-21

Catalog-driven command architecture replacing hardcoded workflow stages, with custom command support and numerous orchestrator fixes.

### New Features

- Catalog-driven command architecture — workflow stages are now powered by a discoverable command catalog with built-in and custom commands
- Custom command support — create, edit, and delete custom workflow commands per agent with file-backed persistence
- Built-in commands catalog reconciled on every store hydration so new built-in commands appear automatically on upgrade

### Improvements

- Toast notification reminds users to enable newly added commands in the catalog
- Disabled command files automatically removed from project directories when toggled off
- Drag-and-drop stage reordering refactored to use arrayMove for reliability
- Save errors in custom command forms are now surfaced to the user instead of silently failing

### Bug Fixes

- Fixed per-agent stage toggle not removing disabled commands from the workflow execution list
- Fixed custom command save triggering unnecessary app restart and delete not cleaning up project command files
- Fixed ticket column transitions — tickets now stay In Progress during active workflow and land in Review on completion instead of Done
- Prevented path traversal in command file read/write operations
- Prevented custom command IDs from colliding with reserved required workflow stage IDs
- Fixed legacy stage names not normalizing correctly, causing paused workflows to fail on resume
- Fixed workflow pause resume-point now derived from agent config instead of a hardcoded stage list
- Fixed unconfigured stages incorrectly defaulting to disabled — they now default to enabled
- Fixed unknown/unrecognized stages correctly defaulting to disabled

### Testing

- Added patch coverage for command mapping and workflow stage resolution
- Added orchestrator stage-enable/disable edge case tests

---

## [0.1.0-beta.22] - 2026-02-19

Customizable per-agent workflow stage ordering with drag-and-drop UI, and validation agent fix-task lifecycle improvements.

### New Features

- Customizable per-agent workflow stage ordering — drag-and-drop UI lets you reorder optional stages (deslop, review, tests, cleanup) independently for each agent (Claude, Cursor, Codex)
- Selecting a preset resets both stage configs and ordering; manual reorder switches to Custom preset

### Improvements

- Validation agent now waits for created fix tasks to complete their multi-stage workflows before returning to the user, with progress indicators during polling
- Post-fix system message summarizes fix task outcomes and asks about next steps instead of silently returning
- Agent lists sorted alphabetically by display name across Settings, Worker Panel, and Build-With picker
- Extracted fix-task helpers into dedicated validation_fix_tasks.rs module, reducing validation.rs from 1334 to 791 lines
- Removed dead updateSessionStatus frontend wrappers after Pass Validation button removal
- Zustand store version bumped to 13 with migration adding stageOrder to existing agent configs

### Bug Fixes

- Fixed validation agent returning immediately after creating fix tasks while the actual multi-stage workflow was still running in the background
- Fixed dropped fix tasks when an agent response contained both a run_command and create_fix_task block — fix tasks are now extracted before send_agent_followup overwrites the response
- Fixed duplicate fix-task creation when the command loop re-processed a response already handled by send_agent_followup
- Fixed timeout completion message producing misleading "0 completed and 0 failed" for tasks still running after timeout
- Fixed unnecessary 5-second delay on first poll iteration in wait_for_fix_tasks by moving sleep to end of loop
- Fixed required stage keys (branch, plan, implement, commit) leaking into optional stage ordering, causing duplicate add-and-commit execution and broken resume logic
- Removed Pass Validation button that had no effect on the validation workflow

### Testing

- Added 14 unit tests for validation fix-task wait and completion logic covering task ID extraction, completion messages, and terminal-state handling
- Added 2 tests for build_full_stage_order verifying no duplicate stages with full 9-key frontend input and equivalence with optional-only input

---

## [0.1.0-beta.21] - 2026-02-19

Local provider support for Codex and Claude Code (Ollama / LM Studio), and a drag-and-drop column targeting fix.

### New Features

- Local provider support for Codex — new OSS toggle in Codex settings enables running against self-hosted models via Ollama or LM Studio with configurable model override
- Local provider support for Claude Code — new "Use Local Provider" toggle in Claude Code settings reveals Base URL, Model Override, API Key, and Auth Token fields for custom endpoints
- Zero-cost tracking for local providers — runs using local model overrides now display $0 cost while still tracking token usage under the actual model name

### Improvements

- Agent provider trait extended with is_local_override and effective_cost_model methods for dynamic cost behavior across all providers
- New extract_cost_with_overrides helper consolidates the resolve-model / extract-cost / zero-if-local pattern used by all cost extraction call sites
- Agent config now threaded through branch name generation so local provider settings apply during the branch-gen stage
- Codex and Claude settings both hydrate from backend on mount, preventing stale defaults from overwriting persisted configuration
- Removed dead ClaudeSettings and CursorSettings components (899 lines)

### Bug Fixes

- Fixed tickets unable to be dragged to the Ready column — replaced closestCorners collision detection with a custom strategy that prioritizes column droppables via pointerWithin with rectIntersection and closestCenter fallbacks
- Fixed stale droppable rects after window resize by adding MeasuringStrategy.Always to DndContext
- Fixed credential-loss bug where toggling a Claude CLI option silently wiped previously saved API credentials from the backend
- Fixed build_env_vars / is_local_override inconsistency — both now require use_local_provider=true AND a non-empty base_url
- Fixed useLocalProvider toggle reverting to false on page reload due to missing backend hydration
- Fixed Codex settings not syncing to backend AgentSettingsManager (only updated frontend Zustand store)

### Testing

- Added 7 unit tests for CodexApiConfig parsing and command builder edge cases
- Added 5 tests for extract_cost_with_overrides covering pass-through, zero-cost, and model override scenarios
- Added use_local_provider assertions to Claude agent settings tests

---

## [0.1.0-beta.20] - 2026-02-18

Codex CLI agent with per-agent settings architecture, Sonnet 4.6 model support, and Codex agent bug fixes.

### New Features

- Codex CLI agent — OpenAI Codex is now available as a third agent alongside Claude Code and Cursor, invoked via codex exec --json with NDJSON output parsing and token-based cost tracking
- Per-agent settings architecture — each agent (Claude Code, Cursor, Codex) now has independent workflow configuration, model selections, and sub-agent settings instead of shared globals
- Sonnet 4.6 model support — added claude-sonnet-4-6 across all model selectors with cost tracking normalization

### Improvements

- Settings tabs reorganized to General | Claude Code | Cursor | Codex | Data, with each agent tab showing a self-contained configuration page
- All workflow preset defaults updated from Sonnet 4.5 to Sonnet 4.6
- Spec agent model selector replaced with dropdown using full MODEL_OPTIONS list so new models appear automatically
- available_models() API added to AgentProvider trait for dynamic model discovery across all providers
- Split settingsStore.ts into settingsStore.ts + settingsStore.types.ts for modularity
- Removed tagline subtitle from board headers for cleaner UI

### Bug Fixes

- Fixed Cursor command_instructions_subdir using "rules" instead of "commands" to match Cursor's actual .cursor/commands/ directory
- Fixed Worker not checking synced flag in resolve_workflow_settings, which could use default-constructed settings before frontend sync
- Fixed shallow-copy bug in getDefaultConfigForAgent that allowed mutations to leak into shared preset constants
- Fixed newly created projects not appearing in ticket project lookup
- Fixed Codex branch name generation: NDJSON-aware parsing with command_execution fallback and nested item.text scanning
- Fixed Codex cost tracking: correct pricing tiers, model normalization, and cost capture in branch-gen and plan-validation sub-runs
- Fixed branch-gen model lookup using get_for_agent() instead of get() for correct per-agent settings
- Fixed UTF-8 panic in branch-gen log preview caused by byte-index slicing
- Fixed brace counting in extract_branch_from_line ignoring braces inside JSON string literals

### Testing

- Added 17 unit tests covering Codex NDJSON parsing, cost tracking, and branch name extraction
- Added 4 tests for synced/unsynced/missing-agent/no-shared-state paths in Worker config resolution
- Added 4 tests for JSON string-aware brace counting in branch name extraction

---

## [0.1.0-beta.19] - 2026-02-17

Removed unused hooks system (~5,100 lines), unified Cursor and Claude output parsing via stream-json format.

### New Features

- Cursor agent now uses structured stream-json output format, unifying output parsing with Claude Code via a shared parser

### Improvements

- Removed hooks system entirely (~5,100 lines) — hooks never provided functional data; all useful run data comes from log streaming
- RunDetailsPanel now shows logs directly instead of a Timeline/Logs tab switcher
- Settings UI no longer shows "Supported Hooks" tables
- Validation no longer checks for hooks installation (one fewer setup requirement)
- DB migration 13 drops the hooks_installed_json column on upgrade

### Bug Fixes

- Fixed flaky CLI utils cache independence tests caused by parallel test interference with shared global state

### Testing

- Added 11 regression tests covering removed hook variants, validation flow, AppState shape, and provider trait surface
- Added Cursor stream-json parsing tests: assistant-only fallback, malformed JSON handling, and multi-turn output

---

## [0.1.0-beta.18] - 2026-02-16

Agent-agnostic provider architecture, Claude CLI settings UI, and bug fixes for ticket status, PR creation, and webhook event spooling.

### New Features

- Agent provider registry with pluggable architecture — Claude and Cursor are now registered dynamically through a provider trait instead of hardcoded logic
- Claude CLI settings UI with configurable command-line options (CliOptionsSection component in settings)
- Diagnostic agent settings panel for per-agent configuration
- Agent registry store on the frontend for dynamic agent discovery

### Improvements

- Refactored the entire agent system to be agent-agnostic using a provider pattern, replacing hardcoded agent logic with trait-based providers
- Extracted shared CLI utilities into a dedicated cli_utils module reused across agents
- Split monolithic cost module into focused submodules (estimation, extraction, tests)
- Extracted command templates into a shared module used by all agents
- Split monolithic runs command file into submodules (branch, orchestrate, queries, cost_commands)
- Create PR and Push to Remote buttons now automatically commit uncommitted changes before pushing
- Extracted diff parser into dedicated module, reducing next_steps from 868 to 496 lines
- SSE handler and board polling now reload tasks when ticket state changes for live status updates
- Backend creates diagnostic comments synchronously before moving tickets to Blocked

### Bug Fixes

- Fixed stale clarification banner showing when ticket was blocked for a different reason — now checks most recent non-user comment
- Fixed Create PR button failing with "No commits between main and branch" when changes weren't committed before pushing
- Fixed falsy API response from webhook causing spooled events to accumulate indefinitely (postToApi now returns an explicit ok/data wrapper)
- Fixed task statuses going stale while viewing a ticket modal

### Testing

- Added unit tests for Claude provider, Cursor provider, and CLI utilities
- Added agent registry and settings store tests on the frontend
- Added 9 unit tests for has_uncommitted_changes and commit_all_changes helpers
- Added CLI availability hook tests and ProjectsList component tests

---

## [0.1.0-beta.17] - 2026-02-15

Stage timeout setting now uses hours instead of minutes for more practical management of long-running workflows.

### Improvements

- Changed stage timeout setting from minutes to hours for more practical time management of long-running agent workflows

---

## [0.1.0-beta.16] - 2026-02-15

Pre-determined branch names in plans, rich workflow-complete summaries, and a race condition fix for ticket edits.

### New Features

- Planner now generates branch names for every ticket at planning time following a type/epic-slug/ticket-slug convention, visible during plan review
- Each ticket mini-spec includes a Branch section referencing its own branch and base branch for chain context
- Workflow-complete comments now include the full plan and implementation summary instead of a generic static message

### Improvements

- Create PR button now automatically pushes the branch to origin first if it hasn't been pushed yet, so you can go straight from Done to PR in one click
- Extracted comment assembly into a pure build_workflow_summary function for testability
- Implementation summary is safely truncated to 5 KB with a truncate_to_char_boundary helper

### Bug Fixes

- Fixed race condition where saving ticket edits overwrote the column status set by the orchestrator (e.g. Blocked reverting to In Progress)
- Removed column selector from the edit form — column moves are now exclusively via drag-and-drop or the Resolve & Move to Ready button

### Testing

- Added serde round-trip tests for branch_name (present, absent, null) and PlanViewer component tests for conditional branch name rendering
- Added 16 unit tests for build_workflow_summary and truncate_to_char_boundary covering all branches

---

## [0.1.0-beta.15] - 2026-02-14

Multi-dependency tracking fixes, active-child advancement guard, and planner now receives full conversation context.

### New Features

- Planner now receives full brainstorm conversation context (observations, clarifications, Q&A history) for more informed ticket generation
- New extract_latest_observations and build_conversation_context helpers surface codebase findings from the discovery phase into planning

### Improvements

- RunsHistory is now scrollable and displays all runs instead of capping at 5
- Renamed "runs" to "stages" in CostBadge and EpicProgressPanel for clarity with multi-stage workflows
- Simplified dependency checking by returning blocking dep info directly, eliminating duplicate DB queries

### Bug Fixes

- Fixed dependency tracker only checking the primary dependency for multi-dependency epics — now validates ALL dependencies before advancing
- Fixed dragging an epic back to Ready advancing a new child ticket when another child was already active (added active-child guard)
- Fixed legacy depends_on_epic_id field being ignored by multi-dependency code when depends_on_epic_ids_json was NULL

### Testing

- Added 16 unit tests for extract_latest_observations and build_conversation_context covering all branches
- Added 15 DB-level tests for has_active_epic_child, are_all_dependencies_complete, and get_epics_depending_on edge cases

---

## [0.1.0-beta.14] - 2026-02-14

Blocked ticket UX overhaul with guided clarification flow, one-click resolve, and automatic task syncing.

### New Features

- BlockedTicketBanner component with task-aware guidance that adapts messaging for initial vs follow-up task failures
- One-click "Resolve & Move to Ready" action to reset failed tasks and advance the ticket without manual drag-and-drop
- "Needs Input" badge on ticket cards in the Blocked column for at-a-glance visibility
- Ticket description edits now sync to the initial task so clarification updates propagate to the agent automatically

### Improvements

- CommentsSection auto-expands when clarification comments are present, including when clarification arrives after mount
- Clarification comment metadata now includes task_id and task_order_index so the frontend knows which task was blocked
- Backend resets failed tasks to pending on description update so the agent re-reads updated content

### Bug Fixes

- Fixed CommentsSection not expanding when a clarification comment arrived after the component had already mounted

---

## [0.1.0-beta.13] - 2026-02-13

Fix spec agent failing to parse pretty-printed raw JSON responses from AI agents.

### Bug Fixes

- Fixed spec agent failing to parse pretty-printed raw JSON when agents respond without code fences
- JSON extraction now locates the opening brace by walking backwards from the "spec_complete" key, handling both compact and multi-line formatted output
- Added serde alias so "technical_notes" (snake_case) is accepted alongside "technicalNotes" (camelCase) in structured specs

---

## [0.1.0-beta.12] - 2026-02-12

Validation chat for verifying ticket changes, next steps panel, file diff viewer, and comprehensive cost tracking fixes.

### New Features

- Validation Chat: interactive post-completion AI chat to verify ticket changes with Cursor or Claude agents
- Next Steps Panel on completed tickets with push branch, create PR, and view diff actions
- Validation view in sidebar for managing validation sessions
- File Diff Viewer with per-file collapsible sections, add/delete/context line coloring, and line numbers
- Agent selection for spec brainstorm conversations (choose Cursor or Claude)
- Validation agent backend with streaming log support

### Improvements

- Ticket modal widened to near full-width for easier navigation
- Work Complete layout consolidated into a single row with expandable View Diff section
- Cost tooltip now displays cache write tokens and derives counts from per-model data
- Removed misleading ~ prefix and amber styling from cost badges — all badges show plain cost
- Total cost always derived from sum of per-model costs for consistent display at every level
- Ticket-level cost badge only marks estimated when all runs are estimated

### Bug Fixes

- Fixed model name normalization in cost tracking (e.g. "claude-opus-4-6" mapped to "opus-4.6")
- Fixed duplicate cost counting for multi-stage parent runs
- Updated model pricing to match 2026 Anthropic rates (Opus $5/$25, Haiku $1/$5 per MTok)
- Fixed expanded run total mismatch with cost badge amount
- Fixed diff line numbering when parsing unified diff hunk headers
- Fixed missing agent_type column in fresh database schema
- Fixed validation message returning user message instead of assistant response
- Fixed legacy runs with empty model usage now attributed to "other" so totals always add up

---

## [0.1.0-beta.11] - 2026-02-10

Workflow presets, cost tracking, spec shortcuts, and release notes.

### New Features

- Per-stage AI workflow settings with presets (Comprehensive, Balanced, Quick Fix, etc.)
- Agent cost tracking with per-run and per-ticket cost summaries
- Backfill button in Data Settings for retroactive cost calculation
- Spec progress shortcut for quick access to epic progress view
- Opus 4.5 model support
- "What's New" release notes shown on version upgrade

### Improvements

- Spec Agent settings extracted into dedicated settings tab
- Themed confirmation dialogs replace native browser confirm()
- Simplified spec creation (removed redundant per-spec model selector)
- Versioned model identifiers for consistency across all settings
- Spec list cards show project name alongside board name

### Bug Fixes

- Fixed cost backfill retry logic for tickets with new runs
- Fixed CostBadge showing "Unavailable" for Cursor runs with estimated costs
- Fixed spec generating indicator race condition causing UI flicker
- Fixed output truncation boundary for multi-byte UTF-8 characters
- Fixed model mapping for unversioned model values
