# Changelog

All notable changes to Bored are documented in this file.

## [0.1.0-beta.62] - 2026-03-25

Debug mode, automatic code review on ticket completion, and detour-sync stage. A new per-provider Debug Mode toggle causes every CLI subprocess invocation to emit a `bored_system` JSON log line containing the full command string, visible in the log timeline as system entries — giving full visibility into what exact commands Bored submits to agents. Sensitive environment variables (API keys, git identity) are automatically filtered from debug output. A new "Run on Ticket Complete" setting triggers the code review loop automatically after the last task of a ticket finishes, using a dedicated `CodeReview` task type so the orchestrator schedules it as a discrete task rather than running it inline. Per-project push and PR creation now route correctly for workspace tickets. Chat UX gains optimistic user message insertion and stale-chat guards to prevent showing messages from a previously selected chat.

### New Features

- Debug Mode — new per-provider toggle that emits `bored_system` JSON log lines for every CLI subprocess invocation, showing the full command string in the log timeline as system entries; works in both agent workflows (orchestrator stages) and chat mode
- Auto Code Review on Ticket Complete — new "Run on Ticket Complete" setting triggers the code review loop automatically after the last task of a ticket finishes successfully, followed by a commit stage; skipped in code-review-only mode since that is already a review workflow
- `TaskType::CodeReview` variant with full `to_db_string`/`parse`/`display_name`/`command_id` support, enabling the orchestrator to schedule auto code review as a discrete task
- Detour-sync stage — new reserved internal stage ID (`detour-sync`) that merges agent work back to the target branch, mapped to the "Commit" stage group in the UI stepper

### Improvements

- Debug environment filtering via `debug_env_prefix()` strips sensitive environment variables (API keys, git identity) from CLI command output in debug mode using `SENSITIVE_ENV_PREFIXES`
- Per-project push and PR creation — `resolve_ticket_project_dir()` dispatches to per-project or fallback resolution so push/PR commands work correctly for workspace tickets with multiple projects
- `ProjectBranchStatus` extended with `check_has_unpushed` and `has_uncommitted` fields for accurate button state in the NextStepsPanel
- Chat live event filter broadened from a 3-variant allowlist to an SSE skip-list (denylist), allowing `bored_system` events to flow through without explicit listing
- Optimistic user message insertion in `chatStore` — messages appear immediately in the UI before the backend responds
- Stale-chat guards in `loadMessages` and `loadChatEvents` prevent overwriting the current chat's messages when async loads from a previously selected chat resolve late
- Debug command building extracted from duplicated call sites in `chat/mod.rs` and `stages.rs` into shared `build_debug_command_line` and `build_debug_log_line` helpers in `agents/mod.rs`
- `finish_workflow` caches `get_task()` result instead of calling it twice, avoiding a redundant `RwLock` read guard acquisition and `Task` clone
- Worktree `branch_name` used for detour branch resolution in worker and branch setup so the orchestrator gets the actual checkout name instead of the ticket branch name
- Debug Mode toggle moved from WorkflowSection to per-agent CLI Options sections in the settings UI
- `extendedContext` added as frontend alias for `extended_context_enabled` with inline lookup

### Bug Fixes

- Fixed NextStepsPanel to always call `getWorkspaceBranchStatus` — backend now handles non-workspace tickets, removing the need for the frontend to guard the call
- Fixed `buildSyncPayload` containing redundant `?? false` defaults

### Testing

- cargo clippy --all-targets -- -D warnings: 0 warnings
- cargo test --lib: 1969 passed, 0 failed
- tsc --noEmit: 0 errors
- vitest run: 47 files, 1026 passed, 0 failed
- 6 new Rust unit tests for `debug_env_prefix` covering empty, safe, sensitive, mixed, ordering, and single-var cases
- New Rust unit tests for `TaskType::CodeReview` roundtrip (`to_db_string`/`parse`/`display_name`/`command_id`)
- 321-line orchestrator integration test suite for auto code review and detour-sync workflows
- New TypeScript tests for `getTaskTypeLabel("code_review")`, `getCommandId("code_review")`, and `debugMode`/`autoCodeReviewOnComplete` sync payload round-trips
- New `chatStore` tests for optimistic message insertion and stale-chat guards on `loadMessages`/`loadChatEvents`
- New `settingsStore` migration v24→v25 tests with version assertion fix
- New `parseLogEvents` tests for `bored_system` event rendering in the log timeline

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.61, here is a summary of the major features introduced in recent releases:

**beta.61 — Extended Context Model Suffix & Auto-Pilot Model Filtering**
Claude Code extended context now uses the `[1m]` model suffix instead of the deprecated `--betas` flag, with eligibility gated to claude-opus-4-6 and claude-sonnet-4-6. The `--effort` CLI argument moves to the `CLAUDE_CODE_EFFORT_LEVEL` environment variable. Auto-pilot settings gain per-model enable/disable toggles so users can restrict which models the auto-pilot command selector can choose. The code-review fallback parser now accepts pass-status JSON where LLMs use `review_status` or `status` fields instead of `issues_found: 0`.

**beta.60 — Robust Code-Review Parsing & Auto-Clarification Routing**
The review agent's structured output parser now handles common LLM deviations — wrapper objects, missing `issues_found`, `files` arrays instead of `file` strings, and numeric `lines` values — via a best-effort fallback when strict deserialization fails. The code-review prompt is tightened with an explicit schema table, concrete examples, and DO-NOT rules to reduce deviations at the source. Auto-clarification `DeleteTask` now correctly routes tickets to Ready, Review, or Done based on remaining tasks and auto-complete settings.

**beta.59 — Multi-Project Workspaces**
Projects can now be grouped into workspaces for coordinated multi-repo agent work. Tickets and chats can be scoped to a workspace instead of a single project, letting agents read, write, and branch across all workspace repos simultaneously. Sidebar renames "Projects" to "Scopes" with a unified list. Per-project branch status, diffs, push, and PR creation in a collapsible accordion. Cursor, Claude Code, and Codex agents receive multi-root context via `.code-workspace` files and `--add-dir` flags.

**beta.58 — Robust Fix-Task Parsing**
Multi-strategy fallback chain for the review agent's `create_fix_tasks` parser: primary JSON extraction, direct key-search with balanced brace matching, tail-parse, last-brace truncation, and malformed-JSON fallback. Both Rust and TypeScript parsers accept the singular `create_fix_task` form, and the frontend strips malformed JSON blocks from displayed review messages.

---

## [0.1.0-beta.61] - 2026-03-25

Extended context model suffix, auto-pilot model filtering, and code-review pass-status parsing. Claude Code extended context now uses the `[1m]` model suffix instead of the deprecated `--betas` flag, with eligibility gated to claude-opus-4-6 and claude-sonnet-4-6. The `--effort` CLI argument moves to the `CLAUDE_CODE_EFFORT_LEVEL` environment variable. Auto-pilot settings gain per-model enable/disable toggles so users can restrict which models the auto-pilot command selector can choose. The code-review fallback parser now accepts pass-status JSON where LLMs use `review_status` or `status` fields instead of `issues_found: 0`.

### New Features

- Auto-pilot model filtering — new `autoPilotEnabledModels` field end-to-end (types, store migration v24, Rust backend serde, orchestrator filtering) restricts which models auto-pilot can select for command execution
- `AutoPilotRow` component with collapsible per-model enable/disable panel replacing the inline toggle, showing selection model dropdown and available model count badge

### Improvements

- Extended context uses `[1m]` model suffix instead of the deprecated `--betas` CLI flag, with eligibility gated to claude-opus-4-6 and claude-sonnet-4-6 via `is_1m_eligible()`
- Claude Code effort level moved from `--effort` CLI argument to `CLAUDE_CODE_EFFORT_LEVEL` environment variable, set before the early return so it applies regardless of local provider config
- Code-review fallback parser (`parse_structured_review_fallback`) extended with a 5-condition guard that accepts JSON blocks containing an explicit `issues` key or a pass-like `review_status`/`status` field (pass/clean/approved, case-insensitive)
- Code-review prompt adds `review_status` and `status` to the disallowed field rename list to reduce future LLM schema deviations

### Bug Fixes

- Fixed code-review fallback parser rejecting valid pass-status JSON (e.g. `{"review_status":"pass","issues":[]}`) — the guard returned `None` when it saw no `issues_found` key and no issues in the array, causing the orchestrator to treat a passing review as unparseable and unnecessarily run the fix phase
- Fixed extended context `--betas` flag being sent to Claude CLI which no longer supports it — replaced with `[1m]` model suffix on eligible models

### Testing

- npx tsc --noEmit: 0 errors
- cargo clippy --all-targets -- -D warnings: 0 warnings
- cargo test --lib: 1941 passed, 0 failed
- npx vitest run: 47 files, 994 passed, 0 failed
- 16 new Rust unit tests for pass-status recognition, case insensitivity, non-string value resilience, wrapper objects, contradictory status+issues, and public API integration
- New Rust tests for `[1m]` suffix eligibility gating on opus, sonnet, and ineligible models
- New TypeScript tests for `AutoPilotRow` model count badge, expand/collapse, and store migration v24

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.60, here is a summary of the major features introduced in recent releases:

**beta.60 — Robust Code-Review Parsing & Auto-Clarification Routing**
The review agent's structured output parser now handles common LLM deviations — wrapper objects, missing `issues_found`, `files` arrays instead of `file` strings, and numeric `lines` values — via a best-effort fallback when strict deserialization fails. The code-review prompt is tightened with an explicit schema table, concrete examples, and DO-NOT rules to reduce deviations at the source. Auto-clarification `DeleteTask` now correctly routes tickets to Ready, Review, or Done based on remaining tasks and auto-complete settings.

**beta.59 — Multi-Project Workspaces**
Projects can now be grouped into workspaces for coordinated multi-repo agent work. Tickets and chats can be scoped to a workspace instead of a single project, letting agents read, write, and branch across all workspace repos simultaneously. Sidebar renames "Projects" to "Scopes" with a unified list. Per-project branch status, diffs, push, and PR creation in a collapsible accordion. Cursor, Claude Code, and Codex agents receive multi-root context via `.code-workspace` files and `--add-dir` flags.

**beta.58 — Robust Fix-Task Parsing**
Multi-strategy fallback chain for the review agent's `create_fix_tasks` parser: primary JSON extraction, direct key-search with balanced brace matching, tail-parse, last-brace truncation, and malformed-JSON fallback. Both Rust and TypeScript parsers accept the singular `create_fix_task` form, and the frontend strips malformed JSON blocks from displayed review messages.

**beta.57 — Real-Time Title Bar Status**
Title bar queued/active status pills now update instantly when tickets move via user drag-and-drop or backend agent workflows, instead of waiting for the 5-second polling interval. The Tauri `ticket-moved` event listener lifecycle is ref-counted alongside the existing polling interval, with promise-based cleanup to prevent leaks under React strict mode.

---

## [0.1.0-beta.60] - 2026-03-25

Robust code-review JSON parsing with fallback extraction and smarter auto-clarification ticket routing. The review agent's structured output parser now handles common LLM deviations — wrapper objects, missing `issues_found`, `files` arrays instead of `file` strings, and numeric `lines` values — via a best-effort fallback when strict deserialization fails. The code-review prompt is tightened with an explicit schema table, concrete examples, and DO-NOT rules to reduce deviations at the source. Auto-clarification `DeleteTask` now correctly routes tickets to Ready, Review, or Done based on remaining tasks and auto-complete settings instead of always landing in Ready.

### Improvements

- Fallback parser in `parse_structured_review` handles common LLM deviations when strict `CodeReviewOutput` deserialization fails — unwraps single-key wrapper objects (e.g. `{"review": {…}}`), derives `issues_found` from `issues` array length when missing, and tolerates `files` (array) in place of `file` (string)
- Code-review.md prompt tightened with explicit field-level schema table, two concrete examples (issues found / no issues), and DO-NOT rules against wrapping in extra keys, renaming fields, or using arrays for the `file` field
- Auto-clarification `DeleteTask` now routes tickets based on remaining tasks — Ready (pending tasks remain), Review (no pending, no auto-complete), or Done (no pending, auto-complete enabled) — matching the routing logic used by `finish_workflow`

### Bug Fixes

- Fixed code-review structured output parser silently returning `None` when LLMs wrap JSON in an extra key (e.g. `{"review": {...}}`), omit `issues_found`, or use `files` array instead of `file` string — `parse_structured_review` now falls back to `parse_structured_review_fallback` which extracts what it can from the raw JSON value
- Fixed `issue_from_value` dropping numeric `lines` values — `as_str()` returns `None` for JSON numbers like `"lines": 42`, now falls back to `as_u64()` stringified
- Fixed auto-clarification `DeleteTask` always moving tickets to Ready regardless of remaining tasks — tickets with no pending tasks now go to Review (or Done with auto-complete), preventing tickets from getting stuck in Ready with nothing to execute

### Testing

- npx tsc --noEmit: 0 errors
- cargo clippy --all-targets -- -D warnings: 0 warnings
- cargo test --lib: 1922 passed, 0 failed
- npx vitest run: 47 files, 987 passed, 0 failed
- 14 new Rust tests: 11 code-review fallback parser branch/path coverage tests, 2 integration tests for fallback flows, 1 auto-clarification DeleteTask→Done routing test

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.59, here is a summary of the major features introduced in recent releases:

**beta.59 — Multi-Project Workspaces**
Projects can now be grouped into workspaces for coordinated multi-repo agent work. Tickets and chats can be scoped to a workspace instead of a single project, letting agents read, write, and branch across all workspace repos simultaneously. Sidebar renames "Projects" to "Scopes" with a unified list. Per-project branch status, diffs, push, and PR creation in a collapsible accordion. Cursor, Claude Code, and Codex agents receive multi-root context via `.code-workspace` files and `--add-dir` flags.

**beta.58 — Robust Fix-Task Parsing**
Multi-strategy fallback chain for the review agent's `create_fix_tasks` parser: primary JSON extraction, direct key-search with balanced brace matching, tail-parse, last-brace truncation, and malformed-JSON fallback. Both Rust and TypeScript parsers accept the singular `create_fix_task` form, and the frontend strips malformed JSON blocks from displayed review messages.

**beta.57 — Real-Time Title Bar Status**
Title bar queued/active status pills now update instantly when tickets move via user drag-and-drop or backend agent workflows, instead of waiting for the 5-second polling interval. The Tauri `ticket-moved` event listener lifecycle is ref-counted alongside the existing polling interval, with promise-based cleanup to prevent leaks under React strict mode.

**beta.56 — Custom Title Bar & Task Execution UI**
Native OS title bar replaced with a custom bar showing live worker count, queue depth, and active ticket counts with a Workers dropdown for starting/stopping workers from any view. Task execution in chat redesigned with a structured TaskExecutionCard showing real-time task status and a workflow stage progress stepper (Branch → Plan → Implement → Code Review → Commit). Dashboard trend charts now bucket events by local date instead of UTC.

---

## [0.1.0-beta.59] - 2026-03-24

Multi-project workspace support. Projects can now be grouped into workspaces for coordinated multi-repo agent work. Tickets and chats can be scoped to a workspace instead of a single project, letting agents read, write, and branch across all workspace repos simultaneously. The sidebar navigation renames "Projects" to "Scopes" with a unified list showing both projects and workspaces. Per-project branch status, diffs, push, and PR creation are shown in a collapsible accordion in the ticket detail panel. Cursor, Claude Code, and Codex agents all receive multi-root context via `.code-workspace` files and `--add-dir` flags. Also adds Claude Code `--effort` flag support, macOS native Edit menus for Cmd+C/V/X and dictation, and a "Validate with" button in the ticket sidebar.

### New Features

- Multi-project workspaces — new `workspaces` and `workspace_projects` tables (schema v21) that group multiple projects for coordinated agent execution across repositories
- Workspace-scoped tickets and chats — tickets and chats can be assigned to a workspace instead of a single project, enabling agents to work across all workspace repos simultaneously
- "Scopes" navigation — sidebar renames "Projects" to "Scopes" with a unified ScopesList showing both projects and workspaces with inline create/edit/delete UI
- ScopeSelector component — shared dropdown listing both projects and workspaces with optgroup headers, used in CreateTicketModal, NewChatModal, TicketEditForm, and TicketDetailSidebar
- Per-project accordion in NextStepsPanel — workspace tickets show expandable rows per project with independent branch status, diff viewer, push, and PR creation
- `.code-workspace` file generation — workspace chats auto-generate a VS Code workspace file so Cursor agents get multi-root context
- `--add-dir` support for Claude Code and Codex — workspace projects beyond the primary repo are passed as `--add-dir` flags so agents can read/write across all repos
- Claude Code `--effort` flag — new effort setting (low/medium/high) exposed through agent settings UI and wired through ClaudeApiConfig and command builder
- "Validate with" button — moved from NextStepsPanel to Agent Actions in TicketDetailSidebar and TicketModalFooter, visible when a ticket has a branch
- macOS native Edit menus — Undo/Redo/Cut/Copy/Paste/Select All via PredefinedMenuItem, enabling Cmd+C/V/X and dictation support
- macOS Info.plist for microphone/speech recognition permissions via build.rs linker args
- Fullscreen diff viewer shows project name badge for workspace ticket diffs
- `get_workspace_branch_status` Tauri command for per-project branch/diff status in workspace tickets

### Improvements

- Removed `default_project_id` from boards — boards are now scope-agnostic containers; scope lives on tickets and chats
- Schema v22 migration makes `chats.project_id` nullable for workspace-only chats (detects NOT NULL constraint via `pragma_table_info` and rebuilds only if needed)
- `reserve_next_ticket` matches workspace tickets via `workspace_projects` join so workers with a project filter pick up workspace tickets containing that project
- `can_move_to_ready` accepts workspace tickets by checking the first workspace project path instead of returning NoProject
- Review mode aggregates diffs from all workspace projects for workspace-scoped tickets
- `set_ticket_project` clears `workspace_id` and `set_ticket_workspace` clears `project_id` for mutual exclusivity at the DB level
- Extracted `CreateWorkspaceForm` and `EditWorkspaceForm` into `WorkspaceForm.tsx`, reducing ScopesList from 613 to 532 lines
- Replaced silent `.ok()` on workspace file I/O in `send_chat_message` with proper `map_err` error propagation
- Extracted duplicated worktree cleanup into a closure in `worktree_setup.rs`
- Replaced silent `unwrap_or_default()` with proper error propagation in `commands/chat.rs` and `ticket_builder.rs`
- Downgraded noisy startup log to debug level

### Bug Fixes

- Fixed migration v21 creating `idx_tickets_workspace` index before the `workspace_id` column exists, causing "no such column" error and migration rollback on existing databases upgrading from v20
- Fixed `reserve_next_ticket` not matching workspace tickets — workers with a project filter skipped tickets scoped to a workspace containing that project
- Fixed `can_move_to_ready` rejecting workspace tickets with NoProject error instead of checking workspace project paths
- Fixed `get_ticket_working_dir` failing for workspace tickets when `ticket.project_id` is None — now falls back to the first workspace project
- Fixed `chats.project_id` NOT NULL constraint blocking workspace chat creation on databases that ran v21 before the table rebuild was added (v22 migration)
- Fixed workspace worktree path collision for multi-project tickets — each project in a workspace received the same `run_id`, causing "Worktree path already exists" on the second project; now appends a per-project index
- Fixed workspace diff accordion stuck on loading forever — `diffLoading` state in the `useEffect` dependency array caused an infinite re-run/cancel loop; replaced with a ref-based guard
- Fixed workspace diff rows all showing the same project's changes — `get_branch_diff_files` lacked a `projectId` parameter, so every row loaded the first project's diff
- Fixed broken NextStepsPanel and ListView tests after component refactor — updated mocks for `ProjectBranchRow` and changed "No project" assertion to "No scope"

### Testing

- npx tsc --noEmit: 0 errors
- cargo clippy --all-targets -- -D warnings: 0 warnings
- cargo test --lib: 1894 passed, 0 failed
- npx vitest run: 47 files, 987 passed, 0 failed
- 27 new Rust tests covering workspace context in prompts, effort parsing/defaults, workspace_id create/update/clear semantics
- 12 new workspace DB tests covering CRUD operations for workspaces and workspace_projects

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.58, here is a summary of the major features introduced in recent releases:

**beta.58 — Robust Fix-Task Parsing**
Multi-strategy fallback chain for the review agent's `create_fix_tasks` parser: primary JSON extraction, direct key-search with balanced brace matching, tail-parse, last-brace truncation, and malformed-JSON fallback. Both Rust and TypeScript parsers accept the singular `create_fix_task` form, and the frontend strips malformed JSON blocks from displayed review messages.

**beta.57 — Real-Time Title Bar Status**
Title bar queued/active status pills now update instantly when tickets move via user drag-and-drop or backend agent workflows, instead of waiting for the 5-second polling interval. The Tauri `ticket-moved` event listener lifecycle is ref-counted alongside the existing polling interval, with promise-based cleanup to prevent leaks under React strict mode.

**beta.56 — Custom Title Bar & Task Execution UI**
Native OS title bar replaced with a custom bar showing live worker count, queue depth, and active ticket counts with a Workers dropdown for starting/stopping workers from any view. Task execution in chat redesigned with a structured TaskExecutionCard showing real-time task status and a workflow stage progress stepper (Branch → Plan → Implement → Code Review → Commit). Dashboard trend charts now bucket events by local date instead of UTC.

**beta.55 — Code-Review-Only Agent Workflow**
Standalone "Review with" workflow that iteratively runs code-review and code-review-fix stages on completed feature branches without re-running plan/implement. Dedicated Code Review Agent settings with per-provider control over model, timeout, retries, and max iterations. Structured review timeline with severity badges and iteration tracking.

---

## [0.1.0-beta.58] - 2026-03-23

Robust fix-task parsing with multi-strategy fallback chain. The review agent's `create_fix_tasks` parser was silently failing when model responses contained markdown code blocks (triple backticks) inside JSON string values. The parser now cascades through five extraction strategies: primary JSON code block extraction, direct key-search with balanced brace matching, tail-parse from the opening brace to end-of-response, last-brace truncation, and a malformed-JSON fallback that tolerates unescaped quotes entirely. Both Rust and TypeScript parsers also accept the singular `create_fix_task` form, and the frontend strips malformed JSON blocks from displayed review messages.

### Improvements

- Added `create_fix_task` (singular) fallback in both Rust and TS parsers, normalizing to the plural array form for backward compatibility with model output variations
- Added direct key-search fallback in `parse_create_fix_tasks_from_response` that bypasses `extract_all_json_code_blocks` and uses balanced brace matching directly on raw text
- Added tail-parse and last-brace fallback strategies when balanced extraction fails due to unescaped quotes in JSON strings
- Added `extract_fix_tasks_from_malformed()` as last-resort fallback that scans for `"title":` and `"description":` patterns using string operations tolerant of invalid JSON
- Added `find_balanced_from_offset` public API in `json_extraction.rs`
- Added structured tracing (debug/warn/info) throughout the fix task parsing pipeline for diagnostics
- Workers dropdown uses popover theme tokens (`bg-board-popover`, `hover:bg-board-popover-hover`) for correct theming

### Bug Fixes

- Fixed review agent's `create_fix_tasks` parser silently failing when model responses contained markdown code blocks inside JSON string values — the `extract_all_json_code_blocks` scanner misidentified backtick-quoted code as fence openings, skipped to end of text, and the bare fallback couldn't recover
- Fixed `\\n` corruption in malformed description extraction — chained `.replace()` calls incorrectly converted literal `\\n` (escaped backslash + n) to backslash + newline because later passes reinterpreted output from earlier ones
- Fixed malformed JSON blocks appearing in review message display — added `extractFixTaskFromMalformed()` on the frontend to strip invalid JSON blocks when `JSON.parse` fails

### Testing

- npx tsc --noEmit: 0 errors
- cargo clippy --tests -- -D warnings: 0 warnings
- cargo test --lib: 1857 passed, 0 failed
- npx vitest run: 47 files, 991 passed, 0 failed
- 23 new tests: 7 TS (singular via `<json>` tag, camelCase acceptanceCriteria, empty title, malformed title-only, tab/CR escapes, `\\n` preservation, production-exact cases), 3 Rust json_extraction (`find_balanced_from_offset`), 2 Rust parsing (malformed title-only, forward-slash unescape), plus production-exact reproduction tests

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.57, here is a summary of the major features introduced in recent releases:

**beta.57 — Real-Time Title Bar Status**
Title bar queued/active status pills now update instantly when tickets move via user drag-and-drop or backend agent workflows, instead of waiting for the 5-second polling interval. The Tauri `ticket-moved` event listener lifecycle is ref-counted alongside the existing polling interval, with promise-based cleanup to prevent leaks under React strict mode.

**beta.56 — Custom Title Bar & Task Execution UI**
Native OS title bar replaced with a custom bar showing live worker count, queue depth, and active ticket counts with a Workers dropdown for starting/stopping workers from any view. Task execution in chat redesigned with a structured TaskExecutionCard showing real-time task status and a workflow stage progress stepper (Branch → Plan → Implement → Code Review → Commit). Dashboard trend charts now bucket events by local date instead of UTC.

**beta.55 — Code-Review-Only Agent Workflow**
Standalone "Review with" workflow that iteratively runs code-review and code-review-fix stages on completed feature branches without re-running plan/implement. Dedicated Code Review Agent settings with per-provider control over model, timeout, retries, and max iterations. Structured review timeline with severity badges and iteration tracking.

**beta.54 — Strengthened Agent Test Commands**
Integration-test command expanded from 5 steps to 8, adding service dependency discovery, service startup with health checks, mandatory run-and-verify with 3-retry loop, and service cleanup. Unit-tests command gains mandatory execution with structured failure diagnosis, assertion quality review, and regression checks.

---

## [0.1.0-beta.57] - 2026-03-22

Real-time title bar status updates. The title bar queued/active status pills now update instantly when tickets move — both from user drag-and-drop and backend agent workflows — instead of waiting for the 5-second polling interval. The Tauri `ticket-moved` event listener lifecycle is ref-counted alongside the existing polling interval, and the listener uses promise-based cleanup to prevent leaks under React strict mode.

### Bug Fixes

- Fixed title bar queued/active chips not updating in real time — `boardStore.moveTicket()` now calls `useWorkerStatusStore.refresh()` after a successful invoke, and `useWorkerStatus` subscribes to the Tauri `ticket-moved` event so backend-initiated moves also trigger an immediate refresh
- Fixed leaked `ticket-moved` event listener on early unmount — replaced async-stored `_unlisten` callback with `_listenPromise` (the Promise itself) so cleanup chains `.then(fn => fn())` whether the promise already resolved or is still pending, preventing orphaned listeners under React strict mode mount-unmount-remount cycles
- Fixed clippy `cloned_ref_to_slice_refs` lint in `get_tasks_by_ids` test — replaced `&[task.id.clone()]` with `std::slice::from_ref(&task.id)`

### Testing

- npx tsc --noEmit: 0 errors
- cargo clippy --tests -- -D warnings: 0 warnings
- cargo test --lib: 1834 passed, 0 failed
- npx vitest run: 47 files, 977 passed, 0 failed
- 5 new useWorkerStatusStore tests covering ticket-moved listener subscription, cleanup, and strict-mode race

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.56, here is a summary of the major features introduced in recent releases:

**beta.56 — Custom Title Bar & Task Execution UI**
Native OS title bar replaced with a custom bar showing live worker count, queue depth, and active ticket counts with a Workers dropdown for starting/stopping workers from any view. Task execution in chat redesigned with a structured TaskExecutionCard showing real-time task status and a workflow stage progress stepper (Branch → Plan → Implement → Code Review → Commit). Dashboard trend charts now bucket events by local date instead of UTC.

**beta.55 — Code-Review-Only Agent Workflow**
Standalone "Review with" workflow that iteratively runs code-review and code-review-fix stages on completed feature branches without re-running plan/implement. Dedicated Code Review Agent settings with per-provider control over model, timeout, retries, and max iterations. Structured review timeline with severity badges and iteration tracking.

**beta.54 — Strengthened Agent Test Commands**
Integration-test command expanded from 5 steps to 8, adding service dependency discovery, service startup with health checks, mandatory run-and-verify with 3-retry loop, and service cleanup. Unit-tests command gains mandatory execution with structured failure diagnosis, assertion quality review, and regression checks.

**beta.53 — Dynamic Model Discovery, Auto-Pilot Commands & Performance**
Dynamic Cursor model discovery via CLI instead of a hardcoded list. Auto-pilot required commands with before/after phasing and iterative code-review loop support. Major performance optimization replacing broad Zustand store subscriptions with individual selectors, memoizing expensive computations, and eliminating N+1 database queries across 25+ components.

---

## [0.1.0-beta.56] - 2026-03-22

Custom title bar with live worker controls, redesigned task execution UI with stage tracking, and timezone-aware dashboard charts. The native OS title bar is replaced with a custom title bar that shows live worker status, queue depth, and in-progress ticket counts, with a dropdown to start/stop workers from any view. Task execution in chat is redesigned with a structured TaskExecutionCard showing real-time task status (pending/running/done/failed) and a workflow stage progress stepper (Branch → Plan → Implement → Code Review → Commit). Dashboard trend charts now bucket events by the user's local date instead of UTC.

### New Features

- Custom title bar with live system status — replaces native OS title bar with a custom bar showing worker count, queue depth, and active ticket counts visible from every view
- Workers dropdown — "Workers (N)" button with per-provider +/- controls to start/stop workers without navigating to the Workers panel
- Queued and active status pills in the title bar (purple for queued, blue for active) with pulse animation when tasks are actively processing
- Task execution stage tracking in chat — structured TaskExecutionCard replaces the old "Fix Tasks Created" box and streaming timeline with real-time task status and a workflow stage progress stepper (Branch → Plan → Implement → Code Review → Commit)
- `get_tasks_by_ids` Tauri command for efficient batch task lookup by ID

### Improvements

- `useWorkerStatus` zustand store with ref-counted polling — first consumer to mount starts a 5s polling interval, last to unmount stops it, all consumers (TitleBar and WorkerPanel) share the same state
- macOS uses TitleBarStyle::Overlay keeping native traffic lights; Windows/Linux uses custom window control buttons (minimize, maximize, close)
- Bare inline JSON parser for review messages anchors regex to known action keys and uses string-aware brace matching, preventing false matches on `${REPO}` or `{owner}` in explanation text
- TaskExecutionCard status-color logic extracted into a variant lookup table
- Dashboard trend queries apply SQLite time modifier (`date(column, '{offset}')`) to shift UTC timestamps to local time before date extraction, with `utc_offset_minutes` parameter defaulting to 0 for backward compatibility

### Bug Fixes

- Fixed dashboard trend charts bucketing events by UTC date instead of local date — a ticket completed at 6pm PST appeared on the wrong day because the stored UTC timestamp crossed midnight
- Fixed duplicate task rendering in assistant review messages — ReviewMessage rendered its own FixTaskCard alongside the new TaskExecutionCard
- Fixed completed task card reappearing during new conversation turns — activeFixTaskIds found stale fix_tasks_created messages across turn boundaries
- Fixed streaming thinking view rendering alongside TaskExecutionCard during fix task execution
- Fixed TaskExecutionCard showing empty "0/0 done" card when taskIds is empty instead of falling back to parsed titles
- Fixed stage stepper showing spinner on final stage after task completion because agentRuns were only fetched while tasks were in_progress
- Fixed RunStatus values like 'aborted', 'queued', 'paused' silently falling through as unrecognized StageGroupStatus strings
- Fixed progress counter only counting 'completed' tasks, missing 'failed' tasks that are also terminal

### Testing

- npx tsc --noEmit: 0 errors
- cargo clippy --tests -- -D warnings: 0 warnings
- npx vitest run: 972 tests passed
- 16 new useWorkerStatusStore tests, 18 stageLabels tests, 5 parseReviewBlocks tests, 6 get_tasks_by_ids Rust tests

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.55, here is a summary of the major features introduced in recent releases:

**beta.55 — Code-Review-Only Agent Workflow**
Standalone "Review with" workflow that iteratively runs code-review and code-review-fix stages on completed feature branches without re-running plan/implement. Dedicated Code Review Agent settings with per-provider control over model, timeout, retries, and max iterations. Structured review timeline with severity badges and iteration tracking.

**beta.54 — Strengthened Agent Test Commands**
Integration-test command expanded from 5 steps to 8, adding service dependency discovery, service startup with health checks, mandatory run-and-verify with 3-retry loop, and service cleanup. Unit-tests command gains mandatory execution with structured failure diagnosis, assertion quality review, and regression checks.

**beta.53 — Dynamic Model Discovery, Auto-Pilot Commands & Performance**
Dynamic Cursor model discovery via CLI instead of a hardcoded list. Auto-pilot required commands with before/after phasing and iterative code-review loop support. Major performance optimization replacing broad Zustand store subscriptions with individual selectors, memoizing expensive computations, and eliminating N+1 database queries across 25+ components.

**beta.52 — Consolidated Review Task Creation**
Standardized on a single `create_fix_tasks` JSON format, removing the deprecated singular `create_fix_task` variant. Review agent prompt rewritten to explicitly state that only the JSON tool block creates tasks.

---

## [0.1.0-beta.55] - 2026-03-21

Code-review-only agent workflow, updater restart resilience, and test stability. Adds a standalone "Review with" workflow that iteratively runs code-review and code-review-fix stages on completed feature branches without re-running plan/implement. Dedicated Code Review Agent settings give per-provider control over model, timeout, retries, and max iterations. A structured review timeline with severity badges and iteration tracking provides visibility into what the review agent finds and fixes. On the stability side, the API server now retries port binding with exponential backoff to survive the brief overlap during Tauri updater restarts, and residual broad store subscriptions in useChatSync and useBoardSync are replaced with targeted selectors and stable refs.

### New Features

- Code-review-only workflow mode — new `CodeReviewOnly` workflow variant that iteratively runs code-review and code-review-fix stages until no issues are found or the user cancels/pauses, without re-running plan/implement stages
- "Review with" button in ticket sidebar (visible when a branch exists) with magnifying-glass icon, distinct from the "Build with" lightning bolt
- Dedicated Code Review Agent settings section per agent provider with independent model, timeout, retries, and max iterations (toggle for "Run until clean" vs capped)
- Expandable code-review stage group in the workflow timeline showing per-iteration details with structured issue cards (severity badges, file paths, descriptions)
- Badge progression: reviewing → N issues → fixing → N issues fixed (amber hazard icon for iterations with issues, green checkmark for clean)
- Structured JSON output format for code-review and code-review-fix commands with robust parser (prefers JSON, falls back to legacy `ISSUES_FOUND:` line with markdown-bold handling)
- Real-time `agent-code-review-update` events for live iteration tracking
- Shared `aggregateRunCosts` utility for correct per-iteration cost display

### Improvements

- API server port binding now retries up to 10 times with exponential backoff (150ms base, 2s cap, ~15s total window), covering the brief port overlap during Tauri updater restarts
- Converted `useChatSync` from full `useChatStore()` subscription to individual selectors, matching the pattern established in beta.53 across other hooks
- Wrapped `handleBoardSelect` and `requestDeleteBoard` in `useCallback` with refs in `useBoardSync`, stabilizing props for `React.memo`'d children
- Sidebar reorganized into Agent Actions / Ticket Actions sections for clearer hierarchy

### Bug Fixes

- Fixed app unresponsiveness after Tauri updater restart — when `relaunch()` is called, the new process spawns before the old one fully exits, causing `TcpListener::bind` on port 7432 to fail silently; the API server never started and SSE connections failed, making the app appear frozen
- Fixed `useChatSync` subscribing the root App component to every chatStore state change, causing unnecessary re-renders on every SSE event
- Fixed `handleBoardSelect` and `requestDeleteBoard` being recreated on every render, defeating `React.memo` on Sidebar and other children
- Fixed 23 pre-existing test failures: settingsStore persist version (22→23), AgentSettingsPage section counts (5→6, 6→7), RunsHistory `CostBadge` mock missing `aggregateRunCosts` export, and code-review grouping assertion

### Testing

- npx tsc --noEmit: 0 errors
- cargo clippy --tests -- -D warnings: 0 warnings
- cargo test --lib: 1828 passed, 0 failed
- npx vitest run: 45 files, 915 passed, 0 failed

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.54, here is a summary of the major features introduced in recent releases:

**beta.54 — Strengthened Agent Test Commands**
Integration-test command expanded from 5 steps to 8, adding service dependency discovery, service startup with health checks, mandatory run-and-verify with 3-retry loop, and service cleanup. Unit-tests command gains mandatory execution with structured failure diagnosis, assertion quality review, and regression checks.

**beta.53 — Dynamic Model Discovery, Auto-Pilot Commands & Performance**
Dynamic Cursor model discovery via CLI instead of a hardcoded list. Auto-pilot required commands with before/after phasing and iterative code-review loop support. Major performance optimization replacing broad Zustand store subscriptions with individual selectors, memoizing expensive computations, and eliminating N+1 database queries across 25+ components.

**beta.52 — Consolidated Review Task Creation**
Standardized on a single `create_fix_tasks` JSON format, removing the deprecated singular `create_fix_task` variant. Review agent prompt rewritten to explicitly state that only the JSON tool block creates tasks.

**beta.51 — Robust JSON Parsing & Agent Completion Stability**
Rewrote JSON code block extractor to handle nested markdown fences inside JSON strings. String-aware brace matching for correct parsing of braces inside string values. Deduplicated agent-completion event handling to prevent cascading re-renders.

---

## [0.1.0-beta.54] - 2026-03-21

Strengthen agent test commands to require actual test execution and verification. The integration-test command is expanded from 5 steps to 8, adding service dependency discovery, service startup with health checks, a mandatory run-and-verify loop with up to 3 retries, and service cleanup. The unit-tests command gains a mandatory execution step with structured failure diagnosis, assertion quality review, and regression checks. Both commands now require test output snippets and fix iteration descriptions in their final reports.

### Improvements

- Rewrote integration-test command from 5 steps to 8 steps, adding service dependency discovery (Step 3), service startup and health checks (Step 4), mandatory run-and-verify with 3-retry loop (Step 6), and service cleanup (Step 7)
- Strengthened unit-tests command Step 5 with mandatory test execution, structured failure diagnosis with 3-retry loop, assertion quality review for coincidental passes, and regression suite run
- Updated both commands' final output sections to require test run output snippets and fix iteration descriptions
- Updated integration-test catalog description to reflect the new end-to-end verification scope

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.53, here is a summary of the major features introduced in recent releases:

**beta.53 — Dynamic Model Discovery, Auto-Pilot Commands & Performance**
Dynamic Cursor model discovery via CLI instead of a hardcoded list. Auto-pilot required commands with before/after phasing and iterative code-review loop support. Major performance optimization replacing broad Zustand store subscriptions with individual selectors, memoizing expensive computations, and eliminating N+1 database queries across 25+ components.

**beta.52 — Consolidated Review Task Creation**
Standardized on a single `create_fix_tasks` JSON format, removing the deprecated singular `create_fix_task` variant. Review agent prompt rewritten to explicitly state that only the JSON tool block creates tasks.

**beta.51 — Robust JSON Parsing & Agent Completion Stability**
Rewrote JSON code block extractor to handle nested markdown fences inside JSON strings. String-aware brace matching for correct parsing of braces inside string values. Deduplicated agent-completion event handling to prevent cascading re-renders.

**beta.50 — Review Transition Crash Fix & Plan Decomposition**
Fixed app crash when tickets move to Review while the Overview tab is open. Strengthened plan decomposition prompt to require at least 2 todos for non-trivial tasks with concrete splitting guidelines.

---

## [0.1.0-beta.53] - 2026-03-21

Dynamic Cursor model discovery, auto-pilot required commands with phasing, and major performance optimization. Cursor models are now discovered dynamically via CLI instead of a hardcoded list. Auto-pilot mode gains per-agent required commands with before/after phasing and iterative code-review loop support. App responsiveness is dramatically improved by replacing broad Zustand store subscriptions with individual selectors, memoizing expensive computations, eliminating N+1 database queries, and adding targeted state updates across 25+ components.

### New Features

- Dynamic Cursor model discovery — CursorProvider discovers available models via `cursor agent --list-models` at startup instead of returning a hardcoded list, adapting to user subscription and model availability changes
- Auto-pilot required commands — per-agent `autoPilotRequiredCommands` with before/after phasing so commands like code-review always run regardless of LLM selection, with control over execution order
- Auto-pilot code-review loop — code-review composite loop (review + fix iterations) now works in auto-pilot mode via `run_code_review_loop_with_model`, matching the iterative behavior of multi-stage mode
- Auto-pilot settings UI — interactive command toggles with segmented before/after control and emerald visual treatment when auto-pilot is enabled

### Improvements

- Replaced broad Zustand store subscriptions with individual selectors across 25+ components and hooks to prevent cascading re-renders when navigating between views
- Switched action-only store consumers to `getState()` pattern (CreateBoardModal, RenameBoardModal, BlockedTicketBanner, etc.)
- Wrapped Column, Ticket, Sidebar, ChatListItem, MarkdownViewer, EditableUserMessage, CopyMarkdownButton in `React.memo`
- Hoisted MarkdownViewer's `remarkPlugins` and `components` to module scope to prevent ReactMarkdown re-initialization on every render
- Stabilized App.tsx sidebar callbacks with `useCallback` and added `useMemo` to Board, ChatMessageList, ColumnSelect, CommentsSection, CommandsCatalog, VersionsList
- Paused `useBoardSync` polling when board view is not active to eliminate background state churn
- Replaced `loadChats()` on `chat_cost_updated` with targeted `refreshChat()` to prevent SSE refetch storms
- Added per-chat state pruning to limit memory growth to 5 recent chats
- Replaced N+1 spec queries with single JOIN queries in Rust backend, reducing O(N) DB round-trips to O(1)
- Wrapped batch deletes in SQLite transactions for atomic operation
- Added `idx_runs_started` index on `agent_runs(started_at)` (migration v20)
- Changed SSE event type filter from `Vec` to `HashSet` for O(1) lookups
- Auto-pilot command-selection prompt excludes forced commands from the available list with explicit exclusion note
- Model discovery runs off the main thread via `tokio::task::block_in_place()` to prevent UI blocking
- Orphaned `autoPilotRequiredCommands` entries are now cleaned up when a command is removed

### Bug Fixes

- Fixed app unresponsiveness when swapping pages due to every board store change re-rendering the entire component tree from App.tsx
- Fixed O(n^2) chat event filtering and unmemoized column/ticket computations causing slowness on tickets with many tasks
- Fixed SSE refetch storms and unbounded per-chat state growth causing chat usage slowness
- Fixed dashboard time-range queries performing full table scans without index
- Fixed auto-pilot settings grid column header misalignment between header row and command rows
- Fixed pre-existing test failures in AgentSettingsPage and ticket prompt tests
- Fixed clippy warnings (bool_assert_comparison, useless_vec)

### Testing

- 908 vitest tests passing across 44 files
- 1796 Rust lib tests passing
- Clean cargo clippy and TypeScript compilation

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.52, here is a summary of the major features introduced in recent releases:

**beta.52 — Consolidated Review Task Creation**
Standardized on a single `create_fix_tasks` JSON format, removing the deprecated singular `create_fix_task` variant. Review agent prompt rewritten to explicitly state that only the JSON tool block creates tasks.

**beta.51 — Robust JSON Parsing & Agent Completion Stability**
Rewrote JSON code block extractor to handle nested markdown fences inside JSON strings. String-aware brace matching for correct parsing of braces inside string values. Deduplicated agent-completion event handling to prevent cascading re-renders.

**beta.50 — Review Transition Crash Fix & Plan Decomposition**
Fixed app crash when tickets move to Review while the Overview tab is open. Strengthened plan decomposition prompt to require at least 2 todos for non-trivial tasks with concrete splitting guidelines.

**beta.49 — Auto-Clarification, Task Progress & Session Threading**
Auto-clarification for workflow agents that autonomously resolves plan clarification questions. Task progress counts on ticket cards in board and list views. Workflow session threading across all stages so each stage has full context of prior stages.

---

## [0.1.0-beta.52] - 2026-03-19

Consolidate review task creation to a single JSON format and strengthen prompt guardrails. Removes the deprecated singular `create_fix_task` format from both the Rust backend and TypeScript frontend, standardizing on the plural `create_fix_tasks` array format as the only mechanism for creating tasks. The review agent prompt is rewritten to explicitly state that prose descriptions are silently ignored and only the JSON tool block creates tasks.

### Improvements

- Consolidated review task creation to a single `create_fix_tasks` JSON format with a `tasks` array, removing the deprecated singular `create_fix_task` variant that added unnecessary parsing ambiguity
- Rewrote the review agent `create_fix_task` prompt section to lead with the canonical JSON format and explicitly state it is the ONLY mechanism that creates tasks — prose, markdown headers, or formatted text descriptions are silently ignored
- Simplified review agent prompt tool listing to show a single `create_fix_tasks` format instead of separate singular and plural examples
- Removed the singular `create_fix_task` parsing path from `processJsonMatch` in the frontend, reducing the function to a single code path

### Testing

- Updated all review parsing tests (Rust and TypeScript) to use the canonical `create_fix_tasks` format exclusively
- Removed tests for the deprecated singular `create_fix_task`, bare JSON, mixed singular/plural, and multiple bare JSON block scenarios
- Added `fix_tasks_single_item_array` test to verify single-task creation through the array format

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.51, here is a summary of the major features introduced in recent releases:

**beta.51 — Robust JSON Parsing & Agent Completion Stability**
Rewrote JSON code block extractor to handle nested markdown fences inside JSON strings. String-aware brace matching for correct parsing of braces inside string values. Deduplicated agent-completion event handling to prevent cascading re-renders.

**beta.50 — Review Transition Crash Fix & Plan Decomposition**
Fixed app crash when tickets move to Review while the Overview tab is open. Strengthened plan decomposition prompt to require at least 2 todos for non-trivial tasks with concrete splitting guidelines.

**beta.49 — Auto-Clarification, Task Progress & Session Threading**
Auto-clarification for workflow agents that autonomously resolves plan clarification questions. Task progress counts on ticket cards in board and list views. Workflow session threading across all stages so each stage has full context of prior stages.

**beta.48 — Chat Message Editing, Stop Generation & Cross-Project Isolation**
Chat message editing with inline regeneration and mid-generation cancellation. Cross-project context isolation preventing agents from seeing unrelated project data. Server-side model resolution so settings changes take effect on existing chats.

---

## [0.1.0-beta.51] - 2026-03-17

Robust JSON parsing for nested backticks and agent completion stability. Rewrites the JSON code block extractor to handle LLM responses containing nested markdown fences inside JSON string values (e.g. task descriptions with embedded code examples), which previously caused tasks to be silently dropped. Adds string-aware brace matching, deduplicates agent-completion event handling to prevent cascading re-renders, and adds a guardrail ensuring the review agent actually invokes the create_fix_task tool instead of only claiming to create tasks in natural language.

### Improvements

- Rewrote `extract_all_json_code_blocks` to scan for fence openings with balanced brace-matching so nested backtick sequences inside JSON string values are correctly ignored instead of splitting the input into garbage segments
- Added `<json>` XML-style tag support in both backend (Rust) and frontend (TypeScript) parsers as an alternative JSON extraction format
- String-aware `find_balanced_from()` — brace/bracket matching now tracks `in_string`/`escape_next` state so JSON values containing braces (e.g. `{"msg": "missing }"}`) parse correctly
- Non-JSON fenced blocks (e.g. ` ```sql `) are now skipped entirely including their closing fence, preventing the closing backticks from being misinterpreted as a new opening fence
- Expanded `create_fix_task` tool description to cover user-requested tasks (not just agent-identified issues) and added a CRITICAL guardrail requiring the model to output the JSON tool block to actually create a task
- Improved bare JSON fallback to handle multi-line objects via `find_balanced`

### Bug Fixes

- Fixed `extract_all_json_code_blocks` breaking on nested backtick fences in JSON strings — LLM task descriptions containing markdown code examples inside a JSON string value caused the naive `text.split("```")` to produce garbage segments, silently dropping tasks that the review agent claimed to create
- Fixed `find_balanced_from()` returning truncated matches when JSON strings contained braces/brackets, breaking downstream parsing (e.g. task extraction)
- Fixed agent completion re-render cascade — after an agent run finished, both the Tauri event listener and the poll could fire `handleAgentComplete`, doubling state updates and triggering cascading re-renders that froze the UI; added `completionHandledForRunRef` keyed on `runId` so only the first arrival runs side-effects
- Fixed `useBoardSync` poll interval resetting on every `selectedTicket` update — replaced the closure capture with a ref read, removing it from the `useEffect` dependency array
- Fixed atomic ticket state update — `tickets` and `selectedTicket` are now updated in a single synchronous `setState` call so downstream effects see `lockedByRunId=null` immediately, preventing `isAgentRunning` from bouncing back to true
- Fixed React hooks violation in `NextStepsPanel` — moved `useCallback` above the early `return null` guard to comply with rules of hooks
- Fixed review agent claiming task creation without using tool — the agent would respond with "Task created..." in natural language without emitting the required `create_fix_task` JSON block, resulting in no task being created despite the user being told one was

### Testing

- 87 passing tests for `json_extraction` module covering nested fences, string-aware brace matching, XML-style tags, and bare JSON fallback
- Frontend `parseReviewBlocks` test coverage for XML-style `<json>` tag extraction

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.50, here is a summary of the major features introduced in recent releases:

**beta.50 — Review Transition Crash Fix & Plan Decomposition**
Fixed app crash when tickets move to Review while the Overview tab is open. Strengthened plan decomposition prompt to require at least 2 todos for non-trivial tasks with concrete splitting guidelines.

**beta.49 — Auto-Clarification, Task Progress & Session Threading**
Auto-clarification for workflow agents that autonomously resolves plan clarification questions. Task progress counts on ticket cards in board and list views. Workflow session threading across all stages so each stage has full context of prior stages.

**beta.48 — Chat Message Editing, Stop Generation & Cross-Project Isolation**
Chat message editing with inline regeneration and mid-generation cancellation. Cross-project context isolation preventing agents from seeing unrelated project data. Server-side model resolution so settings changes take effect on existing chats.

**beta.47 — Pause/Resume Reliability & Chat Timeout Notifications**
Pause/resume reliability for todo-based implementation preserving agent session context across pause/resume. Chat agent timeouts now surface a red error bubble with the full event timeline showing what the agent accomplished before the timeout.

---

## [0.1.0-beta.50] - 2026-03-12

Review transition crash fix and improved plan decomposition. Fixes an app crash when tickets move to Review while the Overview tab is open, caused by stale Zustand store state triggering cascading re-renders. Plan decomposition is now more opinionated about splitting work into multiple todos, preventing session-threaded agents from consolidating tasks into monolithic implementations.

### Improvements

- Strengthened plan decomposition prompt to require at least 2 todos for non-trivial tasks with concrete splitting guidelines (types vs logic, backend vs frontend, new code vs refactoring, tests vs implementation)
- Enhanced tracing in decompose_plan_into_todos to log raw output length when the single-todo guard triggers

### Bug Fixes

- Fixed crash when ticket moves to Review while on Overview tab — ticket-moved backend event now syncs selectedTicket and store tickets immediately instead of waiting for the 3-second poll cycle; lockedByRunId normalized to null (matching DB) instead of undefined to prevent false-change detection in polling comparison
- Fixed plan-decompose consolidating work into a single todo after session threading — session continuation with full plan context caused the LLM to collapse multi-step work into one todo, frequently triggering the monolithic implementation fallback

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.49, here is a summary of the major features introduced in recent releases:

**beta.49 — Auto-Clarification, Task Progress & Session Threading**
Auto-clarification for workflow agents that autonomously resolves plan clarification questions. Task progress counts on ticket cards in board and list views. Workflow session threading across all stages so each stage has full context of prior stages.

**beta.48 — Chat Message Editing, Stop Generation & Cross-Project Isolation**
Chat message editing with inline regeneration and mid-generation cancellation. Cross-project context isolation preventing agents from seeing unrelated project data. Server-side model resolution so settings changes take effect on existing chats.

**beta.47 — Pause/Resume Reliability & Chat Timeout Notifications**
Pause/resume reliability for todo-based implementation preserving agent session context across pause/resume. Chat agent timeouts now surface a red error bubble with the full event timeline showing what the agent accomplished before the timeout.

**beta.46 — Idle-Based Agent Timeout & Git Identity Attribution**
Idle-based agent timeout replacing absolute wall-clock deadline so active agents are not killed while producing output. User git identity attribution with Bored co-author trailer on all agent commits.

---

## [0.1.0-beta.49] - 2026-03-11

Auto-clarification for workflow agents, task progress counts on ticket cards, and workflow session threading across all stages. Agents can now autonomously resolve plan clarification questions instead of blocking for user input when enabled. Ticket cards display done/total task progress badges on both board and list views. The agent session is threaded across the entire workflow (plan, decompose, implement, commit, code-review) so each stage has full context of prior stages.

### New Features

- Auto-clarification setting — new per-agent `autoClarification` toggle that lets the agent autonomously resolve plan clarification questions using the plan-phase model; the agent can update a task (rewrite to resolve ambiguity), delete a task (if already completed or no longer needed), or fall back to blocking if it cannot resolve
- Task progress on ticket cards — done/total task progress badge displayed on ticket cards in both board and list views; backed by a new `get_board_task_counts` command that fetches counts for all tickets on a board in a single SQL query
- Workflow session threading — agent session ID is now threaded across the entire workflow (plan, plan-decompose, implement, commit, code-review, custom commands) so each stage has full context of prior stages; session ID is captured after each successful stage and restored on resume

### Improvements

- Extracted clarification logic from execute.rs into dedicated clarification.rs submodule (execute.rs reduced from 706 to 480 lines)
- Auto-clarification prompt includes full context: plan, clarification reason, ticket description, task content, and completed task summaries
- Plan regeneration loop — when auto-clarification updates a task, the plan is regenerated from the refreshed task content before proceeding to decomposition
- Auto-clarification posts an informational comment on the ticket explaining the agent's decision
- Reduced main content padding, set 14px root font size, added 900x600 minimum window size

### Bug Fixes

- Fixed chat input field not resetting when switching between chats — added key-based remount on ChatPanel so MessageInput local state does not leak across chats
- Fixed resolve_clarification rewriting ticket description instead of the specific task content that triggered the clarification
- Fixed auto-clarification silently proceeding when DB write fails or task is missing — now falls back to user-blocking clarification path
- Fixed plan not posted before clarification comment — users now see the plan alongside the clarification questions for context
- Fixed stale plan used after auto-clarification updates task content — validation loop regenerates plan from refreshed task
- Fixed resolve_clarification failing for taskless tickets (legacy workflow) — restores fallback path that reads from ticket description
- Fixed duplicate plan comments when auto-clarification triggered a plan regeneration
- Fixed auto-clarification delete routed to user clarification handler — now has a dedicated handler with correct summary and no spurious fail_task call
- Fixed only diagnostic comments suppress the clarification banner (error comments no longer incorrectly suppress it)
- Fixed clippy too-many-arguments in send_chat_message by consolidating State params into AppHandle

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.48, here is a summary of the major features introduced in recent releases:

**beta.48 — Chat Message Editing, Stop Generation & Cross-Project Isolation**
Chat message editing with inline regeneration and mid-generation cancellation. Cross-project context isolation preventing agents from seeing unrelated project data. Server-side model resolution so settings changes take effect on existing chats.

**beta.47 — Pause/Resume Reliability & Chat Timeout Notifications**
Pause/resume reliability for todo-based implementation preserving agent session context across pause/resume. Chat agent timeouts now surface a red error bubble with the full event timeline showing what the agent accomplished before the timeout.

**beta.46 — Idle-Based Agent Timeout & Git Identity Attribution**
Idle-based agent timeout replacing absolute wall-clock deadline so active agents are not killed while producing output. User git identity attribution with Bored co-author trailer on all agent commits.

**beta.45 — User-Driven Review Mode & Session Resumption**
User-driven review mode presenting tools as available capabilities instead of prescribing a rigid testing workflow. Session resumption extended to all chat modes so follow-up turns use lightweight prompts. Multi-task parsing collects all fix tasks from a single agent response.

---

## [0.1.0-beta.48] - 2026-03-10

Chat message editing with inline regeneration and mid-generation cancellation, cross-project context isolation, and server-side model resolution. Users can now edit any past message to regenerate from that point and stop an in-progress generation with a cancel button. Chat agents are now prevented from seeing tickets and context from unrelated projects, and model resolution is handled server-side with a clear priority chain so settings changes take effect on existing chats.

### New Features

- Chat message editing — edit any past user message with inline editing that truncates the conversation from that point and re-sends with the updated content; users can also re-send unchanged messages to retry a cancelled or failed generation
- Stop generation — a cancel button replaces the send button while the agent is generating, allowing the user to cancel mid-generation via CancelHandle; backend tracks running chat agents in RunningChatAgents state for clean cancellation
- Backend edit_chat_message and cancel_chat_generation commands with DB methods for message truncation

### Improvements

- Server-side model resolution — new resolve_chat_model() with clear priority chain (synced workflow settings > chat.model > mode defaults) so changing settings takes effect on existing chats without requiring chat recreation
- ensureAgentConfigsSynced() called before sending chat messages so agents always use current settings
- Consolidated 5 duplicate agent section components into a single generic AgentSection component in settings UI
- Added timeout/retries configuration to general, ticket builder, validation, and diagnostic agent settings for consistent controls across all agent types
- Renamed "Validation Agent" to "Review Agent" in the settings UI
- Removed deprecated plannerMaxExplorations from config
- Bumped settings store to version 20 with migration

### Bug Fixes

- Fixed cross-project context leakage — review agent now rejects mismatched tickets via project_id guard, and ticket_builder board context is filtered to only include tickets belonging to the chat's project
- Fixed ticket project ownership not validated at chat creation for review mode
- Fixed cancel handle lost on retry — OnSpawnCallback was FnOnce and consumed by .take() on the first iteration of the executor retry loop; changed to FnMut so the callback is invoked on every spawn attempt, keeping the handle map current
- Fixed editAndResend sending to wrong chat on navigation — sets isAgentThinking immediately and invokes send_chat_message directly with the captured chatId instead of re-reading currentChat from the store
- Removed frontend model passing from chat creation (NewChatModal, CreateSpecModal, NextStepsPanel) to eliminate stale model selection at creation time

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.47, here is a summary of the major features introduced in recent releases:

**beta.47 — Pause/Resume Reliability & Chat Timeout Notifications**
Pause/resume reliability for todo-based implementation preserving agent session context across pause/resume. Chat agent timeouts now surface a red error bubble with the full event timeline showing what the agent accomplished before the timeout.

**beta.46 — Idle-Based Agent Timeout & Git Identity Attribution**
Idle-based agent timeout replacing absolute wall-clock deadline so active agents are not killed while producing output. User git identity attribution with Bored co-author trailer on all agent commits.

**beta.45 — User-Driven Review Mode & Session Resumption**
User-driven review mode presenting tools as available capabilities instead of prescribing a rigid testing workflow. Session resumption extended to all chat modes so follow-up turns use lightweight prompts. Multi-task parsing collects all fix tasks from a single agent response.

**beta.44 — Review Session Management, Ticket Builder Model & GPT-5.4**
Review chat session management, ticket builder model independence, hide-done board filter, GPT-5.4 model support, and React rendering stability fixes. Review chat uses lightweight prompts on session resumption. Ticket builder has its own model selector. GPT-5.4 is now the default Codex model.

---

## [0.1.0-beta.47] - 2026-03-10

Pause/resume reliability for todo-based implementation and user-facing timeout notifications in chat. Agent session context is now preserved across pause/resume so the agent retains prior file edits and understanding instead of starting fresh. The frontend correctly waits for all todos to complete before advancing past the implement stage. Chat agent timeouts now surface a red error bubble with the full event timeline showing what the agent accomplished before the timeout, replacing the previous silent failure that left the chat blank.

### Bug Fixes

- Fixed implement pause/resume losing agent context — the agent session ID is now persisted in run metadata so the chat context survives pause/resume instead of starting a fresh session that loses all prior file edits and understanding
- Fixed frontend pause handler advancing past implement prematurely — the handler now checks for incomplete todos before deciding to move to the next stage, preventing skipped work when a single todo sub-run finishes but others remain
- Added warning logs throughout load_todos_from_metadata so silent failures (missing metadata, deserialization issues) are visible in traces instead of silently falling back to monolithic implement
- Fixed initial todo progress not shown on resume — the full todo checklist (including previously completed items) is now emitted when the implement stage starts, eliminating a race window where the UI could briefly show no todos
- Fixed chat agent timeout producing a blank chat with no notification — timeout is now detected as a distinct RunOutcome::Timeout variant, log events are persisted, and a system message with chat_error metadata is saved so the timeline is preserved
- Fixed frontend not reloading messages after agent error — send_chat_message now catches all agent errors and returns Ok with an error system message instead of Err, so the frontend reload flow always runs; a catch block in chatStore sendMessage reloads messages/events as a safety net
- Added ChatErrorBubble component — left-aligned red assistant-style bubble with warning icon, rendered with the ChatEventTimeline above it, so users see exactly what the agent did before the failure

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.46, here is a summary of the major features introduced in recent releases:

**beta.46 — Idle-Based Agent Timeout & Git Identity Attribution**
Idle-based agent timeout replacing absolute wall-clock deadline so active agents are not killed while producing output. User git identity attribution with Bored co-author trailer on all agent commits.

**beta.45 — User-Driven Review Mode & Session Resumption**
User-driven review mode presenting tools as available capabilities instead of prescribing a rigid testing workflow. Session resumption extended to all chat modes so follow-up turns use lightweight prompts. Multi-task parsing collects all fix tasks from a single agent response.

**beta.44 — Review Session Management, Ticket Builder Model & GPT-5.4**
Review chat session management, ticket builder model independence, hide-done board filter, GPT-5.4 model support, and React rendering stability fixes. Review chat uses lightweight prompts on session resumption. Ticket builder has its own model selector. GPT-5.4 is now the default Codex model.

**beta.43 — Per-Mode Chat Models, Auto-Complete Tickets & In-Session Plans**
Per-mode chat model selection — General, Spec Builder/Ticket Builder, and Review each have an independent model selector. Auto-complete tickets setting moves tickets directly to Done. In-session plan generation preserves full conversation context.

---

## [0.1.0-beta.46] - 2026-03-10

Idle-based agent timeout and user git identity attribution. Agent timeouts now reset on every line of output instead of using an absolute wall-clock deadline, preventing active agents from being killed while producing results. All agent commits are now attributed to the user with Bored credited as co-author via commit message trailer.

### Improvements

- Idle-based agent timeout — timeout now resets on every line of stdout/stderr output from the agent process instead of using an absolute wall-clock deadline, so active agents producing logs, tool calls, or streaming responses are no longer killed while making progress; timeout only fires when the process goes silent for the configured duration
- Shared last_activity tracker (Arc<Mutex<Instant>>) updated by stream reader threads on each line read, with per-attempt fresh idle timers replacing the global deadline logic
- Title generation timeout increased from 60s to 120s (now idle-based)
- User git identity attribution — agent commits now show the user as author/committer by injecting GIT_AUTHOR_*/GIT_COMMITTER_* env vars read from the repo's git config chain (repo → global → system), matching the pattern used by GitHub Copilot and Claude Code
- Bored co-author trailer — add-and-commit instructions require a "Co-authored-by: Bored <agent@bored.local>" trailer on every agent commit so Bored is credited as co-author
- Initial commits in new repos no longer pollute repo-level git config — switched from git config user.name/user.email writes to per-invocation env vars for author/committer identity

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.45, here is a summary of the major features introduced in recent releases:

**beta.45 — User-Driven Review Mode & Session Resumption**
User-driven review mode presenting tools as available capabilities instead of prescribing a rigid testing workflow. Session resumption extended to all chat modes (Ticket Builder, Spec Builder, General) so follow-up turns use lightweight prompts. Multi-task parsing collects all fix tasks from a single agent response.

**beta.44 — Review Session Management, Ticket Builder Model & GPT-5.4**
Review chat session management, ticket builder model independence, hide-done board filter, GPT-5.4 model support, and React rendering stability fixes. Review chat uses lightweight prompts on session resumption. Ticket builder has its own model selector. GPT-5.4 is now the default Codex model.

**beta.43 — Per-Mode Chat Models, Auto-Complete Tickets & In-Session Plans**
Per-mode chat model selection — General, Spec Builder/Ticket Builder, and Review each have an independent model selector. Auto-complete tickets setting moves tickets directly to Done. In-session plan generation preserves full conversation context.

**beta.42 — Unified Chat System**
Unified chat system replacing separate validation and conversation flows with four modes — General, Spec Builder, Ticket Builder, and Review — with consistent SSE streaming, cost tracking, and agent log display. Multi-task ticket routing correctly queues remaining tasks.

---

## [0.1.0-beta.45] - 2026-03-09

User-driven review mode, session resumption for all chat modes, and multi-task parsing fix. Review mode now presents tools as available capabilities instead of prescribing a rigid testing workflow, responding to what the user actually asked for. Session resumption extended to Ticket Builder, Spec Builder, and General chat modes so follow-up turns use lightweight prompts instead of re-sending full conversation history. Multi-task parsing now collects all fix tasks from a single agent response.

### Improvements

- User-driven review mode — review prompts now present tools (run_command, start_app, stop_app, create_fix_task) as available capabilities instead of prescribing a rigid 9-step testing workflow, letting the agent decide which tools to use based on the user's request
- Review mode passes the user's first message into the initial prompt so the agent addresses the user's actual intent instead of following a fixed testing script
- Frontend review preset prompts updated to reflect broader review use cases beyond testing
- Session resumption for all chat modes — Ticket Builder, Spec Builder, and General modes now use lightweight resumption prompts on follow-up turns with an existing session, matching the pattern introduced for Review mode in beta.44
- Shared extract_new_chat_messages() and build_chat_resumption_prompt() utilities extracted to chat agent mod.rs so all modes use the same message extraction and resumption prompt format
- has_session() helper added to ChatAgent for modes that don't load the chat, enabling session-aware prompt selection
- Fix-task lifecycle helpers extracted into review_tasks.rs for modularity, separating concerns from the main review module

### Bug Fixes

- Fixed prompt-too-long errors in Ticket Builder, Spec Builder, and General chat modes — all three modes rebuilt and re-sent full conversation history plus system instructions every turn even when the agent already had context via --resume, causing context limit exhaustion after a few exchanges
- Fixed review mode multi-task parsing dropping all but the first fix task — parse_create_fix_tasks_from_response returned early on the first create_fix_task block instead of collecting all blocks from the agent response

### Testing

- Added 6 unit tests for shared session resumption utilities covering extract_new_chat_messages, build_chat_resumption_prompt, and has_session edge cases

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.44, here is a summary of the major features introduced in recent releases:

**beta.44 — Review Session Management, Ticket Builder Model & GPT-5.4**
Review chat session management, ticket builder model independence, hide-done board filter, GPT-5.4 model support, and React rendering stability fixes. Review chat uses lightweight prompts on session resumption. Ticket builder has its own model selector. GPT-5.4 is now the default Codex model.

**beta.43 — Per-Mode Chat Models, Auto-Complete Tickets & In-Session Plans**
Per-mode chat model selection — General, Spec Builder/Ticket Builder, and Review each have an independent model selector. Auto-complete tickets setting moves tickets directly to Done. In-session plan generation preserves full conversation context.

**beta.42 — Unified Chat System**
Unified chat system replacing separate validation and conversation flows with four modes — General, Spec Builder, Ticket Builder, and Review — with consistent SSE streaming, cost tracking, and agent log display. Multi-task ticket routing correctly queues remaining tasks.

**beta.41 — Full Run Tracking for All CLI Agents**
Run tracking and cost capture extended to all CLI agents (planner, brainstorm, validation chat) so every agent invocation appears in the Runs tab and dashboard stats. Removed in-app toast notifications in favor of native OS notifications.

---

## [0.1.0-beta.44] - 2026-03-07

Review chat session management, ticket builder model independence, hide-done board filter, GPT-5.4 model support, and React rendering stability fixes. Review chat now uses lightweight prompts on session resumption to avoid exceeding context limits. Ticket builder chat has its own model selector independent from spec builder. Boards support a hide-done toggle to declutter completed work. GPT-5.4 is now the default Codex model. Two React error #310 crashes fixed across ticket transitions and Agent tab rendering.

### New Features

- Separate ticket builder chat model — ticket builder conversations now use an independent model selector (ticketBuilderModel) separate from spec builder, threaded through AgentConfig, WorkflowSettings, settings UI, and sync payload so users can pick the appropriate model for each chat mode
- Hide-done board filter — per-board toggle in board and list views that filters out the Done column and its tickets, persisted in localStorage for quick decluttering of completed work
- Review message JSON block parsing — review agent responses containing run_command, start_app, stop_app, and create_fix_task JSON blocks are now rendered as styled CommandCard and FixTaskCard components instead of raw markdown
- GPT-5.4 model support — added to all model option lists across frontend and backend with cost estimation pricing ($3.00/$12.00/$0.30/$3.75 per MTok), now the default Codex model for plan, implement, code-review, unit-tests, review-changes, and deslop stages

### Improvements

- Review chat lightweight session resumption — follow-up turns with an existing Claude CLI session now send only new messages since the last assistant response instead of re-sending full system instructions, ticket description, branch diff, and entire conversation history, preventing context limit exhaustion after 1-2 exchanges
- 20-message history cap as a fallback for review chats without an active session, preventing unbounded prompt growth
- Chat UI state refresh after agent responds — loadMessages and loadChatEvents are called after the command returns, fixing cases where the sidebar stayed in thinking state or messages were invisible until navigation
- Chat title reliability — refreshChat called after sendMessage so the chat title and status are always reloaded from the DB regardless of SSE event delivery; broadcast channel capacity increased from 256 to 1024 to reduce lag-induced event drops during burst activity
- Validation agent prompts updated to explore project structure (docker-compose, Makefile, README) before starting apps, instead of assuming npm/node projects
- Post-send refresh guarded against chat navigation — loadMessages and loadChatEvents check that the user hasn't navigated to a different chat during long agent executions, preventing stale data from overwriting the currently-viewed chat
- GPT-5.4 set as the default for all Codex workflow stages, autoPilotModel, plannerModel, and generalModel; existing user configs with gpt-5.3-codex are preserved unchanged

### Bug Fixes

- Fixed review chat "prompt too large" error — session resumption was re-sending full system instructions, 80KB branch diff, and entire conversation history on every follow-up turn even when Claude CLI session resumption (--resume) already retains prior context
- Fixed chat UI not updating after agent responds — run_agent set status back to Active without broadcasting ChatUpdated, so the frontend never learned the agent finished
- Fixed chat title not appearing — title generation SSE event was silently dropped when the broadcast channel lagged during burst activity from extra ChatUpdated broadcasts
- Fixed ListView receiving unfiltered columns when hide-done was active — the Done column appeared in the list view's ColumnSelect dropdown even when hidden on the board
- Fixed React error #310 when tickets move to Review after agent completion — optimistic ticket state updates used JavaScript Date objects instead of ISO strings, which React cannot render as children; all date values in ticket state are now consistently ISO strings matching the backend
- Fixed React error #310 on Agent tab during run completion — agent error event payloads, run summaryMd, auto-pilot selection metadata, log timeline entry.content, entry.rawJson, and raw log payloads carrying non-string values (objects, nested JSON) now defensively coerced to strings before rendering
- Fixed Ticket type using `Date` annotation when the runtime type from the Rust/serde backend is always an ISO string — updated to `Date | string`

### Testing

- Added 16 parseReviewBlocks unit tests covering CommandCard and FixTaskCard extraction from review agent responses
- Added chatStore post-send navigation guard test verifying loadMessages/loadChatEvents are skipped when user navigated away
- Added settingsStore tests for ticketBuilderModel defaults, per-agent isolation, and sync payload shape
- Updated cost estimation, provider, and settings tests across 6 files for GPT-5.4 model addition

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.43, here is a summary of the major features introduced in recent releases:

**beta.43 — Per-Mode Chat Models, Auto-Complete Tickets & In-Session Plans**
Per-mode chat model selection — General, Spec Builder/Ticket Builder, and Review each have an independent model selector. Auto-complete tickets setting moves tickets directly to Done instead of Review for trusted workflows. In-session plan generation preserves full conversation context.

**beta.42 — Unified Chat System**
Unified chat system replacing separate validation and conversation flows with four modes — General, Spec Builder, Ticket Builder, and Review — with consistent SSE streaming, cost tracking, and agent log display. Multi-task ticket routing correctly queues remaining tasks instead of prematurely moving tickets to Review.

**beta.41 — Full Run Tracking for All CLI Agents**
Run tracking and cost capture extended to all CLI agents (planner, brainstorm, validation chat) so every agent invocation appears in the Runs tab and dashboard stats. Removed in-app toast notifications in favor of native OS notifications.

**beta.40 — FK Constraint Fix for Clarification Rewrite**
Fix FK constraint error when resolving clarification via rewrite — the resolve_clarification handler now creates a proper parent run before spawning the spec-rewrite child run.

---

## [0.1.0-beta.43] - 2026-03-05

Per-mode chat model selection, auto-complete tickets, and in-session plan generation. Chat modes now use independent model configurations — General, Spec Builder/Ticket Builder, and Review each have their own model selector. Auto-complete setting moves tickets directly to Done instead of Review for trusted workflows. Plan generation runs in-session with full conversation context instead of spawning a separate background agent.

### New Features

- Per-mode chat model selection — General, Spec Builder/Ticket Builder (planner), and Review chat modes each have an independent model selector synced from frontend agent configs to backend WorkflowSettings, so users can pick the appropriate model for each workload (e.g. Opus for general Q&A, Sonnet for reviews)
- Auto-complete tickets setting — per-agent toggle that moves tickets directly to Done instead of Review when the orchestrator finishes work, eliminating the manual review step for trusted workflows
- In-session plan generation — spec builder now generates plans using the same chat agent session via run_agent instead of spawning a separate PlannerAgent, preserving full conversation context and showing progress in the chat timeline
- PlanBuilderMessage component for rendering plan JSON inline in chat with collapsible epic/ticket cards, overview summary, and ticket count badges
- SpecFinalizedCard for displaying spec finalization details (requirements, decisions, constraints, technical notes) as structured metadata inline in the chat timeline
- PlanTicketTask schema for structured task breakdown in plans — tickets now contain an explicit tasks array with self-contained per-task specs instead of relying solely on the ticket description
- GeneralSection in agent settings page for configuring the general chat model per agent

### Improvements

- Planning prompt restructured — ticket description is now shared context for all tasks, with the tasks array containing the executable units of work; each task's content must be a self-contained implementation spec (150–400 words) that includes everything the agent needs
- Planning prompt branch handling clarified — agents are told the branch is already created and checked out, with explicit instructions never to create, checkout, or switch branches
- Consolidation merge ticket prompt updated to instruct the agent to merge listed branches into the current branch rather than creating a new one
- Chat log events now persist with original timestamps (TimestampedLine) captured during agent streaming instead of using the time of persistence, fixing event timeline ordering for long-running responses
- Agent sorting in NewChatModal and CreateSpecModal memoized to avoid re-sorting on every render
- Removed dead PlannerConfigWithEvents struct, unused stage parameter from create_log_callback, and redundant tracing::debug calls in command handlers
- Zustand store version bumped to 19 with migrations v18 (generalModel) and v19 (autoCompleteTickets) that add correct defaults per agent

### Bug Fixes

- Fixed plan generation losing conversation context — spawning a separate PlannerAgent discarded the spec builder's session history; in-session generation now preserves the full exploration context
- Fixed pending tasks check taking priority over auto-complete — finish_workflow now checks pending tasks first, then auto-complete, then falls through to Review

### Testing

- Added settingsStore migration v18 and v19 tests covering generalModel defaults (per-agent), autoCompleteTickets defaults, and preservation of existing values through migration
- Added generalModel settings tests for per-agent isolation, default values, and round-trip updates
- Added auto-complete tickets settings tests for default state, toggle behavior, and per-agent isolation
- Added sync payload shape tests verifying autoCompleteTickets, generalModel, plannerModel, and validationModel are included for all agents
- Added looksLikePlanResponse utility tests for PlanBuilderMessage
- Added WorkflowSettings serde tests for general_model, planner_model, validation_model fields (defaults, custom values, round-trips, camelCase serialization)
- Added orchestrator integration tests for auto-complete: finish_workflow moves to Done when enabled, and pending tasks still override auto-complete by routing to Ready

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.42, here is a summary of the major features introduced in recent releases:

**beta.42 — Unified Chat System**
Unified chat system replacing separate validation and conversation flows with four modes — General, Spec Builder, Ticket Builder, and Review — with consistent SSE streaming, cost tracking, and agent log display. Multi-task ticket routing correctly queues remaining tasks instead of prematurely moving tickets to Review.

**beta.41 — Full Run Tracking for All CLI Agents**
Run tracking and cost capture extended to all CLI agents (planner, brainstorm, validation chat) so every agent invocation appears in the Runs tab and dashboard stats. Removed in-app toast notifications in favor of native OS notifications.

**beta.40 — FK Constraint Fix for Clarification Rewrite**
Fix FK constraint error when resolving clarification via rewrite — the resolve_clarification handler now creates a proper parent run before spawning the spec-rewrite child run.

**beta.39 — Session Tracking Across Implementation Todos**
Session tracking across implementation todo steps — each todo resumes the same agent session, preserving codebase context and conversation history across sequential steps.

---

## [0.1.0-beta.42] - 2026-03-04

Unified chat system replacing the separate validation and conversation flows with a single interface for all agent interactions. Four chat modes — General, Spec Builder, Ticket Builder, and Review — with consistent SSE streaming, cost tracking, and agent log display. Multi-task ticket routing now correctly queues remaining tasks instead of prematurely moving tickets to Review. Dashboard metrics accuracy improvements for run time and lines changed.

### New Features

- Unified chat system with four modes — General (freeform Q&A), Spec Builder (guided spec creation), Ticket Builder (ticket generation from conversation), and Review (post-completion code review) — replacing the separate validation and conversation subsystems with a single consistent interface
- ChatView, ChatPanel, ChatHeader, ChatList, ChatMessageList, ChatThinkingView, ChatEventTimeline, and NewChatModal components forming the full chat UI
- Chat store (Zustand) with full CRUD, message loading, agent log management, and per-chat state isolation for thinking indicators and log timelines
- SSE-based real-time chat event streaming via useChatSync hook with per-chat scoping that prevents cross-chat event leaking
- Chat backend with five mode handlers — general, spec_builder, ticket_builder, review, and title generation — in the new agents/chat module
- Agent log timeline view on completed assistant messages — persisted ChatEvent records enable viewing the full agent tool-call timeline after a response completes
- Lightweight agent config abstraction in AgentProvider trait — each provider strips expensive features (thinking, chrome, multi-agent) for single-turn tasks like title generation
- TaskDraftList component in CreateTicketModal for inline task creation at ticket creation time
- Copy-as-markdown button on assistant chat messages for copying raw markdown source
- Delete chat with confirmation flow via hover trash icon on chat list items
- TicketBuilderMessage with task content field — agent can provide detailed, self-contained specs per task with JSON repair for malformed responses
- Multi-task ticket routing — tickets with pending tasks route to Ready instead of Review after task completion, so the next task is picked up automatically

### Improvements

- Brainstorm agent renamed to spec_discovery agent for clarity
- Title generation runs in parallel with the agent response instead of waiting for it to finish
- Per-chat maps for agent logs, thinking state, and app logs — switching chats restores the correct state without losing accumulated logs from background chats
- Agent brand icons in NewChatModal and ChatHeader matching the icon style used across Build-With, settings, and spec creation
- Ticket description decoupled from tasks — description is now shared context included in all task prompts rather than being auto-created as Task 0
- At least one task required before moving a non-epic ticket to the Ready column (frontend and backend enforcement)
- MessageInput textarea increased from 1 row to 4 rows default with 300px max auto-resize height for composing multi-line messages
- Conversation history wrapped in XML tags in ticket builder prompt to prevent the model from confusing user text with its own context
- SpecBuilder chat mode requires board_id at both the command and DB layer to prevent runtime panics
- Default 600s timeout applied when send_chat_message omits it, preventing agent processes from running indefinitely
- ConfirmModal resets loading and error state on open, fixing stuck spinners on consecutive deletions
- Markdown table and code block overflow handled with horizontal scrolling instead of clipping past the glass background
- Removed: validation/ components, ConversationView, MessageList, validationStore, useValidationSync, conversations and validation commands, ~5,900 lines of replaced code
- Dashboard avg run time now uses monotonic duration_secs from run metadata instead of DB timestamps inflated by setup overhead
- Git stats collected at run completion after detour merge so lines changed are captured before branches are deleted
- Backfill refreshes all ticket stats on every call instead of skipping tickets with existing stats
- upsert_git_stats preserves prs_created via MAX to prevent backfill from resetting PR counts

### Bug Fixes

- Fixed duplicate assistant message in review mode start_app branch — review_agent_followup already persists the message internally
- Fixed stale conversation context in spec builder auto-completion — messages are now re-fetched from DB after the agent saves its response
- Fixed AgentRegistry state type mismatch (plain vs Arc) causing runtime panic when chat commands were invoked
- Fixed spec builder using wrong prompt on first turn — initial prompt was dead code because the user message was always present in conv_messages
- Fixed title generation spawning a full agent with thinking and tools enabled, causing timeouts and silent failures
- Fixed --max-turns flag (nonexistent in Claude CLI) replaced with --tools "" for tool-free title generation
- Fixed cross-chat event leaking when switching chats — stale React ref replaced with synchronous Zustand state reads
- Fixed malformed JSON in ticket builder responses — regex-based repair handles missing opening quotes on string values
- Fixed ConfirmModal mountedRef not restored after React StrictMode double-invoke, leaving the modal spinner stuck forever
- Fixed board_id validation missing for SpecBuilder at the DB layer, allowing callers bypassing the command layer to create specs without a board
- Fixed multi-task tickets stuck in Review after first task completion — finish_workflow now checks for pending tasks and routes to Ready
- Fixed sub-runs that fail during spawn left stuck in Running status — now properly marked Error with duration metadata
- Fixed avg run time inflated by setup overhead between sub-run creation and agent execution
- Fixed lines changed showing zero for detour branches merged and deleted before dashboard backfill could capture stats
- Fixed stale git stats never refreshed after initial backfill

### Testing

- Added chatStore.test.ts with 39 tests covering CRUD, message loading, per-chat state isolation, and agent log buffering
- Added 20 parseAgentLogToEntries tests for chat-based log timeline parsing
- Added 7 formatRelativeTime utility tests
- Added ticket_builder repair tests for malformed JSON handling (9 total)
- Added 3 finish_workflow integration tests covering all routing branches (Review, Ready with pending tasks, Ready after completion)
- Added lightweight_config_produces_valid_command test verifying Claude and Codex provider lightweight configs
- Added TransitionGuard test for task requirement enforcement on Ready transitions

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.41, here is a summary of the major features introduced in recent releases:

**beta.41 — Full Run Tracking for All CLI Agents**
Run tracking and cost capture extended to all CLI agents (planner, brainstorm, validation chat) so every agent invocation appears in the Runs tab and dashboard stats. Removed in-app toast notifications in favor of native OS notifications.

**beta.40 — FK Constraint Fix for Clarification Rewrite**
Fix FK constraint error when resolving clarification via rewrite — the resolve_clarification handler now creates a proper parent run before spawning the spec-rewrite child run.

**beta.39 — Session Tracking Across Implementation Todos**
Session tracking across implementation todo steps — each todo resumes the same agent session, preserving codebase context and conversation history across sequential steps.

**beta.38 — Notification Banners, In-App Toasts & Todo Cost Badges**
Native OS notification banners with sound, in-app toast notifications via sonner for Review/Blocked transitions, per-todo cost badges in the implementation checklist, and improved SafetyCommitNotice with three contextual visual variants.

---

## [0.1.0-beta.41] - 2026-03-02

Run tracking and cost capture extended to all CLI agents (planner, brainstorm, validation chat) so every agent invocation appears in the Runs tab and dashboard stats. Removed redundant in-app toast notifications in favor of native OS notifications.

### New Features

- Full run tracking for all CLI agents — planner, brainstorm, and validation chat agents now create run records with cost extraction and metadata persistence, appearing in the Agents > Runs tab and dashboard cost/usage stats alongside workflow and diagnostic runs
- Schema migration v17 removes the FK constraint on `agent_runs.ticket_id` so runs can reference specs in addition to tickets
- Stage-based run labels — Runs tab shows Planner, Brainstorm, Validation, and Diagnostic labels for non-ticket runs with context resolved via LEFT JOIN across tickets and specs

### Improvements

- Removed in-app toast notifications (sonner) in favor of native OS notifications — ticket status changes to Review/Blocked now trigger only OS-level notifications via `tauri-plugin-notification`, eliminating the duplicate notification path
- Planner creates a parent run with sub-runs per phase and cost aggregation on the parent, matching the hierarchical run pattern used by the workflow orchestrator

### Bug Fixes

- Fixed missing cost tracking for clarification-gen and spec-rewrite stages — both now call `extract_cost_with_overrides`, persist timing and cost metadata, and correctly handle local provider overrides, matching all other stages
- Fixed planner, brainstorm, and validation chat agent invocations invisible in the Runs tab and excluded from dashboard cost/usage stats

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.40, here is a summary of the major features introduced in recent releases:

**beta.40 — FK Constraint Fix for Clarification Rewrite**
Fix FK constraint error when resolving clarification via rewrite — the resolve_clarification handler now creates a proper parent run before spawning the spec-rewrite child run.

**beta.39 — Session Tracking Across Implementation Todos**
Session tracking across implementation todo steps — each todo resumes the same agent session, preserving codebase context and conversation history across sequential steps.

**beta.38 — Notification Banners, In-App Toasts & Todo Cost Badges**
Native OS notification banners with sound, in-app toast notifications via sonner for Review/Blocked transitions, per-todo cost badges in the implementation checklist, and improved SafetyCommitNotice with three contextual visual variants.

**beta.37 — Implementation Todo Workflow & Clarification Rewrite**
Implementation todo workflow that decomposes plans into focused, independently implementable todos with live UI progress tracking, and a clarification rewrite-and-resolve flow for answering agent questions inline.

---

## [0.1.0-beta.40] - 2026-03-02

Fix FK constraint error when resolving clarification via rewrite — the resolve_clarification handler now creates a proper parent run in the database before spawning the spec-rewrite child run, preventing orphaned runs.

### Bug Fixes

- Fixed FK constraint error when resolving clarification via rewrite — the `resolve_clarification` handler generated a random UUID for `parent_run_id` without inserting it into `agent_runs` first, causing a FOREIGN KEY constraint failure when the child spec-rewrite run was created
- Parent run lifecycle now properly managed (Queued → Running → Finished/Error) to avoid orphaned runs in the database

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.39, here is a summary of the major features introduced in recent releases:

**beta.39 — Session Tracking Across Implementation Todos**
Session tracking across implementation todo steps — each todo resumes the same agent session, preserving codebase context and conversation history across sequential steps.

**beta.38 — Notification Banners, In-App Toasts & Todo Cost Badges**
Native OS notification banners with sound, in-app toast notifications via sonner for Review/Blocked transitions, per-todo cost badges in the implementation checklist, and improved SafetyCommitNotice with three contextual visual variants.

**beta.37 — Implementation Todo Workflow & Clarification Rewrite**
Implementation todo workflow that decomposes plans into focused, independently implementable todos with live UI progress tracking, and a clarification rewrite-and-resolve flow for answering agent questions inline.

**beta.36 — Smart Detour Merge**
Smart detour merge that updates the user's working tree in-place when the target branch is checked out, with outcome-specific ticket notifications and TOCTOU data-loss race elimination.

---

## [0.1.0-beta.39] - 2026-03-02

Session tracking across implementation todo steps so each todo resumes the same agent session instead of starting from scratch, preserving codebase context and conversation history across sequential steps.

### New Features

- Session tracking across implementation todos — each implementation todo now resumes the same agent session instead of starting from scratch, preserving codebase context and conversation history across sequential steps
- `session_id` field added to AgentRunConfig and `extract_session_id` to the AgentProvider trait for provider-agnostic session extraction
- Claude/Cursor session resumption — extracts session_id from stream-json init message and passes `--resume` flag on subsequent invocations
- Codex session resumption — extracts thread_id from `thread.started` event and uses `exec resume` subcommand for continuation

### Improvements

- Session ID threaded through orchestrator stage execution via new `run_stage_with_session` method, propagating across the todo loop in `run_implement_stage_capturing`
- Graceful fallback when session extraction fails — if the provider returns None for extract_session_id, todos proceed without session resumption instead of failing

### Testing

- Added extract_session_id_from_stream_json edge case tests (malformed JSON, system-type priority over fallback, whitespace-only lines)
- Added extract_thread_id_from_codex_json edge case tests (malformed JSON, missing thread_id field)
- Added default AgentProvider::extract_session_id returns None test
- Added orchestrator integration tests for session_id capture, propagation across todos, graceful degradation, and session_id cleared on retry

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.38, here is a summary of the major features introduced in recent releases:

**beta.38 — Notification Banners, In-App Toasts & Todo Cost Badges**
Native OS notification banners with sound, in-app toast notifications via sonner for Review/Blocked transitions, per-todo cost badges in the implementation checklist, and improved SafetyCommitNotice with three contextual visual variants.

**beta.37 — Implementation Todo Workflow & Clarification Rewrite**
Implementation todo workflow that decomposes plans into focused, independently implementable todos with live UI progress tracking, and a clarification rewrite-and-resolve flow for answering agent questions inline.

**beta.36 — Smart Detour Merge**
Smart detour merge that updates the user's working tree in-place when the target branch is checked out, with outcome-specific ticket notifications and TOCTOU data-loss race elimination.

**beta.35 — Detour Branch Worktree & UI Consistency**
Detour branch worktree for active branch conflicts — the agent creates a temporary detour branch and works in an isolated worktree instead of failing when the user has the target branch checked out. UI consistency improvements across 11 files with Button/Input component standardization, WAI-ARIA tabs accessibility, and unsaved-changes guards on modals.

---

## [0.1.0-beta.38] - 2026-03-02

Native OS notification banners with sound, in-app toast notifications for ticket transitions, per-todo cost badges in the implementation checklist, and improved SafetyCommitNotice with three contextual visual variants.

### New Features

- In-app toast notifications via sonner — toast appears top-right with a "View" action when tickets move to Review or Blocked, respecting the existing notifications toggle in settings
- Per-todo CostBadge in ImplementationChecklist — each completed/failed todo shows its individual cost, matched to its implement sub-run by sorted start time order
- Nested implementation todos inside the expandable stage row — clicking the grouped "Implementation (X/Y)" row reveals individual todos with status icons, cost badges, and expandable descriptions

### Improvements

- Native OS notifications now pop up as banners with sound (`.sound("default")`) instead of silently appearing in the notification center
- `ticketTitle` field added to the ticket-moved event payload so toasts display the ticket name
- Reusable `aggregateRunCosts` helper extracted from `getParentRunDisplayCost` — grouped implementation rows now aggregate all RunCostData fields (tokens, cache counts, per-model breakdown, isEstimated) instead of only summing totalCostUsd
- Consistent column layout across normal and implementation stage rows — fixed-width name/status columns with flex-1 spacer for visual alignment
- SafetyCommitNotice redesigned with three contextual visual variants: blue (info) for non-detour safety commits with branch name, green (success) for clean detour merges, amber (warning) only when manual merge action is required
- Branch name stored in safety_commit metadata so the UI can display which branch the auto-saved commit landed on

### Bug Fixes

- Fixed native macOS notifications delivered silently to notification center without banner popups because no sound was set
- Fixed grouped implementation cost badge showing "0 tokens" despite sub-runs having correct data — `aggregateRunCosts` now merges all token, cache, and per-model fields instead of constructing a skeleton RunCostData
- Fixed implementation stage row name indentation not matching other stage rows — background styling moved from wrapper div to button element
- Fixed SafetyCommitNotice showing alarming amber/warning styling for all safety commit cases, even when no work was lost and no action was needed
- Fixed 6 stale test assertions in RunsHistory and RunDetailsPanel that still expected old SafetyCommitNotice text

### Testing

- Added 11 `aggregateRunCosts` tests covering legacy "other" bucket, isEstimated propagation, cache tokens, mixed runs, multi-model merge, and no-cost runs
- Added per-todo CostBadge tests for completed, pending, failed, no sub-runs, no cost, sort order, and more-todos-than-sub-runs edge cases
- Added 249-line `useNotificationToasts` test suite covering event listening, settings respect, toast content, and cleanup

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.37, here is a summary of the major features introduced in recent releases:

**beta.37 — Implementation Todo Workflow & Clarification Rewrite**
Implementation todo workflow that decomposes plans into focused, independently implementable todos with live UI progress tracking, and a clarification rewrite-and-resolve flow for answering agent questions inline.

**beta.36 — Smart Detour Merge**
Smart detour merge that updates the user's working tree in-place when the target branch is checked out, with outcome-specific ticket notifications and TOCTOU data-loss race elimination.

**beta.35 — Detour Branch Worktree & UI Consistency**
Detour branch worktree for active branch conflicts — the agent creates a temporary detour branch and works in an isolated worktree instead of failing when the user has the target branch checked out. UI consistency improvements across 11 files with Button/Input component standardization, WAI-ARIA tabs accessibility, and unsaved-changes guards on modals.

**beta.34 — Full-Page Ticket Detail View**
Full-page ticket detail view replacing the modal overlay with tabbed content (Overview, Task, Agent, Activity), a persistent sidebar with metadata and quick actions, and keyboard navigation with Alt+Arrow prev/next and Escape to go back.

---

## [0.1.0-beta.37] - 2026-03-01

Implementation todo workflow that decomposes plans into focused, independently implementable todos with live UI progress tracking, and a clarification rewrite-and-resolve flow that lets users answer agent questions inline and have the spec automatically rewritten.

### New Features

- Implementation todo workflow — new plan-decompose stage breaks implementation plans into 3–10 focused, independently implementable todos via an agent call; the implement stage then iterates over each todo with scoped prompts instead of running a single monolithic implementation
- ImplementationChecklist UI component — live-updating checklist with per-todo status icons (pending, in-progress, completed, failed), progress bar, and expandable descriptions showing what the agent is working on
- Grouped implementation sub-runs — multiple implement sub-runs are collapsed into a single "Implementation (X/Y)" row in the stages list instead of showing individual entries
- Clarification rewrite-and-resolve flow — when a ticket is blocked for clarification, users can type answers to the agent's questions inline in the BlockedTicketBanner and select an agent to automatically rewrite the ticket spec incorporating the answers, replacing the previous manual description-editing workflow
- `get_implementation_todos` Tauri command for loading todos from run metadata (previous runs and initial state)
- `resolve_clarification` Tauri command that spawns an agent to merge the original description, clarification questions, and user answers into a rewritten spec

### Improvements

- Todo-based implementation produces higher-quality output by breaking large plans into small, scoped steps with individual agent calls instead of a single monolithic prompt
- Resume support for todo-based implementation — completed, failed, and in-progress todos are correctly handled on resume; failed and in-progress todos are retried, completed todos are skipped with their output preserved
- Combined implementation output accumulated across all todos for auto-pilot command selection, so the QA agent sees the full implementation context instead of only the last todo's output
- Duplicate stage outputs concatenated instead of overwritten — `get_completed_stage_outputs` now appends outputs when the same stage name appears in multiple sub-runs, fixing incomplete workflow summaries and auto-pilot resume context
- `mark_todo_status` returns a boolean indicating persistence success; `completed_count` only increments when the status is actually written to the database
- Graceful fallback — if plan decomposition fails or returns 1 or fewer todos, the original single-implement behavior is preserved
- BlockedTicketBanner updated with a response textarea and BuildWithDropdown for agent selection, keeping "Resolve & Move to Ready" as a manual fallback

### Bug Fixes

- Fixed implementation sub-run progress showing inconsistent counts between SubRunsList and ImplementationChecklist — both now derive progress from todo statuses instead of sub-run counts
- Fixed `current_todo_title` emitting the next todo's title after completion instead of an empty string, creating a momentary mismatch between title and status
- Fixed resumed workflows re-executing already-completed todos — saved statuses are loaded before the implementation loop and completed/failed todos are skipped
- Fixed `completed_count` using loop index instead of actual completed count on resume, inflating progress when skipping completed or failed todos
- Fixed todo statuses not copying to the current run when resuming from a previous run, causing all todos to appear as Pending
- Fixed `combined_output` not seeding from previous implement output on resume, causing auto-pilot command selection to lose context from earlier implementation work
- Fixed in-progress todos silently re-executing on resume without logging or state cleanup — now explicitly reset to Pending with a warning log
- Fixed failed todos permanently skipped on resume instead of being retried
- Fixed `plan-decompose` missing from frontend `RESERVED_INTERNAL_STAGE_IDS`, allowing custom command ID collision with the backend stage name
- Fixed `mark_todo_status` silently failing when metadata load returned None — now logs a warning with run ID, status, and todo index

### Testing

- Added 9 unit tests for `extract_clarification_body` covering all edge cases
- Added 4 unit tests for `build_spec_rewrite_prompt` covering description, questions, and answer formatting
- Added 9 BlockedTicketBanner tests for the new rewrite-and-resolve UI flow
- Added ImplementationChecklist component tests (116 lines) covering status rendering, progress bar, and expandable descriptions
- Added RunsHistory tests for grouped implementation sub-run display
- Added 2 integration tests for `get_completed_stage_outputs` concatenation with duplicate stage keys
- Added integration test for in-progress todo reset-to-Pending on resume
- Added integration test for failed todo retry on resume
- Added integration test for resumed output including both previous and new todo outputs

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.36, here is a summary of the major features introduced in recent releases:

**beta.36 — Smart Detour Merge**
Smart detour merge that updates the user's working tree in-place when the target branch is checked out, with outcome-specific ticket notifications and TOCTOU data-loss race elimination.

**beta.35 — Detour Branch Worktree & UI Consistency**
Detour branch worktree for active branch conflicts — the agent creates a temporary detour branch and works in an isolated worktree instead of failing when the user has the target branch checked out. UI consistency improvements across 11 files with Button/Input component standardization, WAI-ARIA tabs accessibility, and unsaved-changes guards on modals.

**beta.34 — Full-Page Ticket Detail View**
Full-page ticket detail view replacing the modal overlay with tabbed content (Overview, Task, Agent, Activity), a persistent sidebar with metadata and quick actions, and keyboard navigation with Alt+Arrow prev/next and Escape to go back.

**beta.33 — Visual Log Timeline & Real-Time Streaming**
Visual log timeline view replacing the raw text log dump with categorized, color-coded entries from all three agents. Real-time log streaming with subagent context badges. 51 parseLogEvents tests.

---

## [0.1.0-beta.36] - 2026-02-28

Smart detour merge that updates the user's working tree in-place when the target branch is checked out, with outcome-specific ticket notifications and TOCTOU data-loss race elimination.

### New Features

- Smart detour merge — when the user has the target branch checked out and the working tree is clean, the agent uses `git merge --ff-only` to update files in-place instead of `git update-ref` which only moved the branch pointer without updating the working tree; falls back to `update-ref` when the user has uncommitted changes
- Outcome-specific detour notifications — ticket comments are posted for each merge outcome (clean merge, dirty working tree with stash instructions, stale working tree needing only a reset) so the user always knows what happened and what to do next
- MergedWorkingTreeStale variant distinguishes clean-tree ff-merge failures from dirty-tree fallbacks, preventing misleading "uncommitted changes" advice when only `git reset --hard HEAD` is needed

### Improvements

- SafetyCommitNotice shows branch-aware messaging with emerald success styling and a checkmark icon for clean detour merges, distinct from the amber warning used for safety-commit scenarios
- Safety commit metadata enriched with detour context (target_branch, detour_branch, merged_to_target) so the UI can render context-specific messaging for all detour outcomes
- Safety commit hash and original created_at timestamp preserved when the first metadata DB write fails — the hash is returned from `safety_commit_and_record` and re-used in fallback metadata construction
- Detour merge metadata recording guard aligned with the merge guard to require both target_branch and detour_fork_point, preventing misleading `merged_to_target: false` for unattempted merges
- Used `if let Some(ref target_branch)` destructuring instead of `is_some()` + raw `Option<String>` serialization for robust metadata JSON construction

### Bug Fixes

- Fixed `git update-ref` not updating the user's working tree when the target branch was checked out — files were invisible until manual `git reset --hard HEAD`
- Eliminated TOCTOU data-loss race in detour merge by removing `git reset --hard HEAD` from the automated path — destructive operations are now left to the user via system comment instructions
- Fixed false "uncommitted changes" message when ff-merge fails with a clean tree — now correctly posts stale-tree instructions without unnecessary stash advice
- Fixed safety commit hash being permanently lost when the first `set_run_metadata` DB write failed — the hash is now returned from the function and merged into fallback metadata
- Fixed detour merge metadata recording when `detour_fork_point` was missing — no merge metadata is recorded for unattempted merges instead of showing a misleading `merged_to_target: false`
- Fixed detour merge recording creating incomplete safety_commit metadata when no actual safety commit existed — now writes merged_to_target even when the agent committed all its work cleanly

### Testing

- Added 2 new Rust tests: test_merge_detour_dirty_worktree_returns_dirty_variant and test_merge_detour_clean_worktree_updates_files
- Added 3 new SafetyCommitNotice frontend tests for branch-aware and outcome-specific messaging
- Added RunDetailsPanel test for clean detour merge rendering

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.35, here is a summary of the major features introduced in recent releases:

**beta.35 — Detour Branch Worktree & UI Consistency**
Detour branch worktree for active branch conflicts — the agent creates a temporary detour branch and works in an isolated worktree instead of failing when the user has the target branch checked out. UI consistency improvements across 11 files with Button/Input component standardization, WAI-ARIA tabs accessibility, and unsaved-changes guards on modals.

**beta.34 — Full-Page Ticket Detail View**
Full-page ticket detail view replacing the modal overlay with tabbed content (Overview, Task, Agent, Activity), a persistent sidebar with metadata and quick actions, and keyboard navigation with Alt+Arrow prev/next and Escape to go back.

**beta.33 — Visual Log Timeline & Real-Time Streaming**
Visual log timeline view replacing the raw text log dump with categorized, color-coded entries from all three agents. Real-time log streaming with subagent context badges. 51 parseLogEvents tests.

**beta.32 — Safety Commits & Dashboard Tokens/Cost Toggle**
Safety commit before worktree removal automatically saves uncommitted agent work before removing worktrees, preventing silent data loss. SafetyCommitNotice surfaces auto-saved commits in the UI. Tokens/cost toggle on Dashboard Top Models chart.

---

## [0.1.0-beta.35] - 2026-02-27

Detour branch worktree for active branch conflicts and UI consistency improvements across modals, accessibility, and component standardization.

### New Features

- Detour branch worktree — when the user has a ticket's branch checked out, the agent now creates a temporary "detour" branch and works in an isolated worktree instead of failing with an "already checked out" error; a detour-sync stage merges the target branch before completion, and the ticket's branch is fast-forwarded to the detour HEAD during cleanup
- Detour merge recovery comments — when the detour merge-back fails or the target branch has diverged, a system comment is posted on the ticket with the detour branch name and git commands for manual recovery

### Improvements

- WAI-ARIA Tabs pattern in TicketDetailView — added role, aria-selected, aria-controls, tabpanel attributes, and keyboard navigation with Arrow/Home/End keys for accessibility
- Unsaved-changes guard on CreateTicketModal and CreateSpecModal — escape, backdrop click, and cancel now show a discard confirmation when form fields have been modified instead of silently discarding input
- ConfirmModal extended to support async onConfirm with loading state, error display, and close prevention during in-flight operations
- Button component extended with a loading prop that shows an animated spinner and disables interaction during async operations
- Replaced raw `<button>` and `<input>` elements with Button and Input components across 11 files for consistent styling (rounded-xl, focus-visible:ring, active:scale, hover shadows, loading states)
- Renamed API auth header from X-AgentKanban-Token to X-Bored-Token to align with the current project name
- Removed duplicate "Boards" nav item from sidebar that appeared twice with different semantics

### Bug Fixes

- Fixed ConfirmModal state updates after parent unmount — added mountedRef guard to prevent React warnings and potential memory leaks when async onConfirm triggers a parent modal to close
- Fixed detour merge leaving orphaned branches when the target branch diverged — detour branch is now preserved for manual merge with recovery instructions posted as a ticket comment

### Testing

- Added 11 new Rust tests covering merge_detour_into_target (fast-forward, diverged, nothing-to-merge, error paths, post-sync merge), delete_branch, WorktreeInfo field defaults, and detour worktree creation

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.34, here is a summary of the major features introduced in recent releases:

**beta.34 — Full-Page Ticket Detail View**
Full-page ticket detail view replacing the modal overlay with tabbed content (Overview, Task, Agent, Activity), a persistent sidebar with metadata and quick actions, and keyboard navigation with Alt+Arrow prev/next and Escape to go back.

**beta.33 — Visual Log Timeline & Real-Time Streaming**
Visual log timeline view replacing the raw text log dump with categorized, color-coded entries from all three agents. Real-time log streaming with subagent context badges. 51 parseLogEvents tests.

**beta.32 — Safety Commits & Dashboard Tokens/Cost Toggle**
Safety commit before worktree removal automatically saves uncommitted agent work before removing worktrees, preventing silent data loss. SafetyCommitNotice surfaces auto-saved commits in the UI. Tokens/cost toggle on Dashboard Top Models chart.

**beta.31 — CLI Model Identifiers & Production Command Fix**
Standardized all model identifiers to full CLI names (claude-opus-4-6 instead of opus-4.6) with v17 settings migration. Fixed bundled command templates not resolving in production builds. Core workflow stages now configurable even when auto-pilot is enabled.

---

## [0.1.0-beta.34] - 2026-02-26

Full-page ticket detail view replacing the single-column modal overlay with tabbed content, a persistent sidebar, and keyboard navigation.

### New Features

- Full-page ticket detail view — replaces the single-column TicketModal overlay with a tabbed layout and persistent sidebar, giving each ticket a dedicated page instead of a constrained modal
- Four content tabs: Overview (description + next steps), Task (task queue + epic panel), Agent (status + runs + logs), Activity (comments) — information is organized by concern instead of stacked in a single scroll
- Right sidebar with ticket metadata (project, branch, labels, status), quick actions (build, edit, delete), and cost summary always visible alongside tab content
- Breadcrumb navigation (Back / Board / Ticket) with prev/next ticket navigation within the same column via Alt+Arrow keys, and Escape to go back
- Fullscreen diff overlay on the NextStepsPanel with Escape to close for reviewing changes at full viewport size
- Branch name copy-to-clipboard button in the sidebar for quick terminal use
- Tab badges: pending task count on the Task tab, running indicator on the Agent tab, and clarification dot on the Activity tab for blocked tickets

### Improvements

- Removed max-height constraints from RunsHistory and LogTimelineView so agent logs use the full viewport in the new layout instead of being capped at 90vh
- NextStepsPanel fullscreen overlay uses a capture-phase Escape handler to avoid conflicting with the parent view's Escape-to-close behavior
- DescriptionSection and CommentsSection accept a defaultExpanded prop so the detail view starts with content visible instead of collapsed

### Bug Fixes

- Fixed hook ordering in CommentsSection — useState and useEffect calls were scattered among non-hook variable calculations and function definitions, making future conditional-logic additions error-prone
- Fixed keydown listener re-registering on every render in TicketDetailView — useTicketEdit returned a fresh object literal on every render, causing the useEffect to tear down and re-add the keydown listener; destructured to stable primitives in the dependency array

### Testing

- Added 32 new tests: DescriptionSection (6), CommentsSection defaultExpanded (3), NextStepsPanel fullscreen (3), TicketDetailHeader (11), AgentTab (4), TasksTab (5)

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.33, here is a summary of the major features introduced in recent releases:

**beta.33 — Visual Log Timeline & Real-Time Streaming**
Visual log timeline view replacing the raw text log dump with categorized, color-coded entries from all three agents. Real-time log streaming with subagent context badges. 51 parseLogEvents tests.

**beta.32 — Safety Commits & Dashboard Tokens/Cost Toggle**
Safety commit before worktree removal automatically saves uncommitted agent work before removing worktrees, preventing silent data loss. SafetyCommitNotice surfaces auto-saved commits in the UI. Tokens/cost toggle on Dashboard Top Models chart.

**beta.31 — CLI Model Identifiers & Production Command Fix**
Standardized all model identifiers to full CLI names (claude-opus-4-6 instead of opus-4.6) with v17 settings migration. Fixed bundled command templates not resolving in production builds. Core workflow stages now configurable even when auto-pilot is enabled.

**beta.29 — Provider-Specific Auto-Pilot Models**
Auto-pilot command selection now sources models from the active provider instead of a hardcoded global list. Prompt constraint prevents agents from hallucinating model names.

---

## [0.1.0-beta.33] - 2026-02-26

Visual log timeline view with real-time streaming, subagent context, and multi-agent log format support.

### New Features

- Visual log timeline view — structured timeline replaces the raw text log dump, parsing NDJSON log events from all three agents (Claude, Cursor, Codex) into categorized, color-coded entries (system, assistant, tool calls, tool results, user input, cost/result) with a Timeline/Raw Logs tab toggle and expandable entries showing full content and raw JSON
- Real-time log streaming — auto-expands the current run on start and polls for new events while active, so logs appear in real-time instead of requiring a close-and-reopen of the ticket modal
- Subagent context in log timeline — distinguishes main agent entries from subagent entries using parent_tool_use_id, with a purple "subagent" badge showing subagent type (Explore, Plan) and task description (e.g. "subagent · Find usage · Haiku 4.5")

### Improvements

- Subagent task descriptions mapped from Task tool calls and displayed in timeline badges with the subagent_type field preferred over description for labeling
- Cost parsing reads from top-level total_cost_usd in Claude result events and includes cache_read/cache_creation tokens in the input token count
- Cursor agent tool_call and thinking log formats now parsed — handles the different JSON shape Cursor CLI emits compared to Claude, extracting tool names from keys (e.g. shellToolCall -> Shell) and stripping worktree prefixes from paths
- Non-string tool result content (array of content blocks) handled in log parser alongside string and object forms
- Consolidated duplicate getEventTypeString into parseLogEvents module and corrected handleRunClick prop type from Promise<void> to void

### Bug Fixes

- Fixed logs only appearing after closing and reopening the ticket modal — useAgentEvents polled but was never consumed, and useRunsHistory loaded once; replaced with integrated polling in useRunsHistory when the expanded run is active
- Fixed loadingEvents state not resetting when collapsing a run, causing a stale loading spinner to flash on the next expand
- Fixed poll tick errors clearing previously-loaded events from the UI — added hasFetchedOnce flag to distinguish initial load errors from transient poll failures
- Fixed cost parsing reading from wrong location in Claude result events (usage.total_cost_usd vs top-level total_cost_usd) and missing cache tokens from input count
- Fixed Cursor agent tool_call events not being parsed due to different JSON shape from Claude
- Fixed non-string tool result content causing parse failures when the content field was an array of content blocks

### Testing

- Added 51 parseLogEvents tests covering Claude/Cursor/Codex format parsing, event filtering, tool summary extraction, malformed JSON handling, and agent type routing
- Added 7 new useRunsHistory tests (19 total) covering auto-expand on lockedByRunId change, polling at interval for active runs, poll error resilience, and polling cleanup on collapse
- Updated RunsHistory component test assertions to match LogTimelineView output

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.32, here is a summary of the major features introduced in recent releases:

**beta.32 — Safety Commits & Dashboard Tokens/Cost Toggle**
Safety commit before worktree removal automatically saves uncommitted agent work before removing worktrees, preventing silent data loss. SafetyCommitNotice surfaces auto-saved commits in the UI. Tokens/cost toggle on Dashboard Top Models chart.

**beta.31 — CLI Model Identifiers & Production Command Fix**
Standardized all model identifiers to full CLI names (claude-opus-4-6 instead of opus-4.6) with v17 settings migration. Fixed bundled command templates not resolving in production builds. Core workflow stages now configurable even when auto-pilot is enabled.

**beta.29 — Provider-Specific Auto-Pilot Models**
Auto-pilot command selection now sources models from the active provider instead of a hardcoded global list. Prompt constraint prevents agents from hallucinating model names.

**beta.28 — App-Internal Commands & Worktree Fixes**
Commands are now app-internal instead of file-managed in projects. Fixed worktree lock failures leaving runs permanently queued and auto-pilot command selection returning zero stages.

---

## [0.1.0-beta.32] - 2026-02-26

Safety commits before worktree removal to prevent silent data loss, and a tokens/cost toggle on the Dashboard Top Models chart.

### New Features

- Safety commit before worktree removal — automatically saves uncommitted agent work before removing worktrees, preventing silent data loss when the add-and-commit stage fails, times out, is disabled, or the workflow errors before reaching it
- SafetyCommitNotice UI component — surfaces auto-saved commits with commit hash in both the ticket modal run history and run details panel so users know when a safety commit was needed
- Tokens/cost toggle on Dashboard Top Models chart — persisted segmented control lets users sort and scale bars by total token consumption or cost in USD, surfacing different insights (e.g. high-token low-cost models)

### Improvements

- ChartCard component extended with a headerActions prop for custom header controls
- Safety commit metadata (commit hash, timestamp) stored in run metadata_json for audit trail and UI rendering
- Git identity set via environment variables (GIT_AUTHOR_NAME/EMAIL) for safety commits, scoped to the single process invocation to avoid modifying shared repo config on systems without global git user settings

### Bug Fixes

- Fixed silent data loss when remove_worktree ran with --force and permanently discarded all uncommitted agent work after the commit stage failed or was skipped
- Fixed safety commit returning Ok(None) instead of Err when git status showed uncommitted changes but git commit reported "nothing to commit" — anomalous state is now surfaced as an error for log visibility

### Testing

- Added 9 Rust tests for safety_commit_if_needed covering clean worktrees, dirty worktrees, staged changes, modified files, deleted files, mixed changes, idempotency, nonexistent paths, and commit message format
- Added 8 frontend tests for SafetyCommitNotice rendering in RunsHistory and RunDetailsPanel components

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.31, here is a summary of the major features introduced in recent releases:

**beta.31 — CLI Model Identifiers & Production Command Fix**
Standardized all model identifiers to full CLI names (claude-opus-4-6 instead of opus-4.6) with v17 settings migration. Fixed bundled command templates not resolving in production builds. Core workflow stages now configurable even when auto-pilot is enabled.

**beta.29 — Provider-Specific Auto-Pilot Models**
Auto-pilot command selection now sources models from the active provider instead of a hardcoded global list. Prompt constraint prevents agents from hallucinating model names.

**beta.28 — App-Internal Commands & Worktree Fixes**
Commands are now app-internal instead of file-managed in projects. Fixed worktree lock failures leaving runs permanently queued and auto-pilot command selection returning zero stages.

**beta.27 — Spec JSON & Cost Attribution Fixes**
Bug fix release improving spec agent JSON extraction reliability, StructuredSpec deserialization, and model override cost re-keying accuracy for local providers.

---

## [0.1.0-beta.31] - 2026-02-25

Core stage model configuration fixes, CLI model identifier standardization across all agents, and production bundled command template resolution.

### Improvements

- Core workflow stages (branch-gen, plan, implement, commit) now have configurable model selectors in the settings UI even when auto-pilot is enabled — core stages always run and need configurable models regardless of auto-pilot mode
- CLI model identifiers standardized to full names (claude-opus-4-6 instead of opus-4.6) across all agents with v17 settings migration that upgrades persisted configs for both Claude and Cursor agents
- Agent-specific model option lists — Claude uses full CLI identifiers (CLAUDE_MODEL_OPTIONS), Cursor pulls from dynamic CLI sync, Codex uses GPT-specific names (CODEX_MODEL_OPTIONS)
- Bundled command templates now resolve correctly in production builds via OnceLock-based Tauri resource path fallback initialized during app setup, replacing compile-time CARGO_MANIFEST_DIR that only worked in dev mode
- Removed dead mapModelForCodex/mapStagesForCodex functions and 10 associated tests — Codex gets its own defaults via getDefaultConfigForAgent instead of runtime mapping from Claude names
- Diagnostic agent now stores cost data, agent_config, and stage_model in run metadata for accurate cost backfill with local provider overrides

### Bug Fixes

- Fixed bundled command templates returning None in production builds — env!("CARGO_MANIFEST_DIR") baked the developer's absolute source path into the binary; auto-pilot discovered zero commands and prompt generation fell back to hardcoded stubs covering only 5 of 16 commands
- Fixed TS2783 duplicate 'model' property in workflow stage config causing TypeScript strict-mode build failures
- Fixed Claude CLI rejecting short model names (opus-4.6) by adding normalize_model_for_cli mapping to full CLI identifiers (claude-opus-4-6)
- Fixed model dropdown showing no selected value for Claude agents — config values used full CLI names but dropdown options only had short names
- Fixed mapModelForCodex matching claude-prefixed model names — includes('opus') incorrectly matched 'claude-opus-4-6'; reverted to startsWith for legacy migration only
- Fixed cost attribution using wrong model when self-hosted overrides are active — sub-runs now store resolved stage_model in metadata for accurate backfill
- Fixed custom command fallback hardcoding Claude model for all agents — now derives default from agent-specific getDefaultConfigForAgent
- Fixed disabling a non-existent stage key producing invalid WorkflowStageConfig without a model field
- Fixed branchGen/commit default model using claude-sonnet-4-5 instead of claude-sonnet-4-6 — unintentional version downgrade affecting new users

### Testing

- Added normalize_model_for_cli tests for Claude command builder covering short name to CLI identifier mapping
- Added v17 migration tests covering both Claude and Cursor config upgrade paths
- Added addCommandToAllAgents tests verifying Codex gets gpt-5.2-codex and Claude gets claude-sonnet-4-6 defaults
- Added edge case test for disabling non-existent stage config keys
- Added orchestrator integration test helpers updated to use model constants instead of hardcoded short names

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.29, here is a summary of the major features introduced in recent releases:

**beta.29 — Provider-Specific Auto-Pilot Models**
Auto-pilot command selection now sources models from the active provider instead of a hardcoded global list. Prompt constraint prevents agents from hallucinating model names.

**beta.28 — App-Internal Commands & Worktree Fixes**
Commands are now app-internal instead of file-managed in projects. Fixed worktree lock failures leaving runs permanently queued and auto-pilot command selection returning zero stages.

**beta.27 — Spec JSON & Cost Attribution Fixes**
Bug fix release improving spec agent JSON extraction reliability, StructuredSpec deserialization, and model override cost re-keying accuracy for local providers.

**beta.26 — Dashboard & Board/List View**
Dashboard landing page with summary stats, trend charts (activity, cost, tokens), model cost breakdown, and per-ticket git stats. Board/list view toggle with column select dropdown. Dynamic Cursor model sync from CLI output.

---

## [0.1.0-beta.29] - 2026-02-25

Bug fix release making auto-pilot command selection use provider-specific models instead of the hardcoded global model list, so each agent's prompt examples and model constraints match its actual provider.

### Improvements

- Auto-pilot command selection prompt now sources available models from the active provider via `available_models()` instead of the hardcoded global `MODEL_ENTRIES` list — Codex runs see Codex models in examples, Claude runs see Claude models
- `pick_example_models` helper selects the most capable (first) and most efficient (last) model from the provider's list for prompt example workflows, falling back gracefully for single-model or empty lists
- Prompt examples and instructions now include an explicit "ONLY use model names from the Available Models list" constraint preventing the agent from hallucinating model names not available to the current provider
- Simplified prompt example workflows from 6 to 4 (removed API change and refactor-with-observability examples) for a more focused and concise prompt

### Bug Fixes

- Fixed auto-pilot command selection prompt always showing Claude model names (opus, sonnet) regardless of which agent provider was active — Codex runs would reference models the provider couldn't use, leading to invalid selections

### Testing

- Added 3 Claude provider end-to-end tests for real CLI output parsing covering non-streaming responses, streaming delta responses, and prose-wrapped JSON extraction
- Added 7 auto-pilot prompt unit tests verifying provider-specific model names appear in the prompt, Codex models replace Claude models when the Codex provider is active, and example workflows use the correct model names
- Added 4 `pick_example_models` unit tests covering empty, single, and multi-model provider lists
- Added ~310 lines of integration tests with a `PromptCapturingRunner` and `CodexStubProvider` verifying that `run_command_selection_stage` builds truly dynamic prompts with provider-specific models

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.28, here is a summary of the major features introduced in recent releases:

**beta.28 — App-Internal Commands & Worktree Fixes**
Commands are now app-internal instead of file-managed in projects. Fixed worktree lock failures leaving runs permanently queued and auto-pilot command selection returning zero stages.

**beta.27 — Spec JSON & Cost Attribution Fixes**
Bug fix release improving spec agent JSON extraction reliability, StructuredSpec deserialization, and model override cost re-keying accuracy for local providers.

**beta.26 — Dashboard & Board/List View**
Dashboard landing page with summary stats, trend charts (activity, cost, tokens), model cost breakdown, and per-ticket git stats. Board/list view toggle with column select dropdown. Dynamic Cursor model sync from CLI output.

**beta.25 — Auto-Pilot Workflow & Command-Based Tasks**
Auto-pilot workflow mode where the agent dynamically decides which commands to run after implementation. Extensible command-based task system backed by the command catalog. Codex reasoning effort and multi-agent toggle.

---

## [0.1.0-beta.28] - 2026-02-24

Bug fix and refactor release fixing worktree lock failures leaving runs permanently queued, auto-pilot command selection never choosing stages, and making commands app-internal instead of file-managed in projects.

### Improvements

- Commands are now app-internal — command content is read from bundled files and app-data custom commands at prompt-generation time and injected directly into agent prompts, eliminating all file-based command installation into project directories
- Removed 5 AgentProvider trait methods, 5 Tauri IPC commands, and associated frontend bindings related to command file installation and checking
- Projects can be added/removed without any file system side effects; command toggling in the catalog is instant
- Diagnostic logging and error-level stage event emission added to the command-selection pipeline for better visibility into failures

### Bug Fixes

- Fixed worktree lock error leaving runs permanently queued — when a git worktree was already locked (branch checked out by user), the manual-run path now marks the run as Error, moves the ticket to Blocked, spawns a diagnostic agent, and unlocks the ticket instead of leaving the run Queued forever with a 30-minute lock
- Fixed worktree failure in the worker path causing infinite re-queue loop — ticket is now conditionally unlocked only when move-to-Blocked succeeds, preventing immediate re-queuing when the Blocked column doesn't exist
- Fixed auto-pilot command selection returning zero stages 100% of the time — three root causes: (1) extract_text_from_stream_json appended result event summary to streaming delta text, corrupting JSON output; (2) find_balanced only matched the first bracket pair, so prose brackets before the JSON caused parse failures; (3) stage failures were silently swallowed
- Added multi-position bracket matching in parse_json_response so prose brackets (e.g. "[the analysis]") before the actual JSON array no longer cause parse failures
- Fixed extract_text_from_stream_json result event text now stored separately and used as fallback only when no streaming deltas exist

### Testing

- Added unit tests for move_ticket_to_blocked return values and the conditional unlock behavior in both manual-run and worker paths
- Added multi-position bracket matching tests for parse_json_response covering prose brackets before JSON arrays
- Added orchestrator integration tests for auto-pilot command selection with corrupted and valid JSON output
- Added Claude provider stream-json extraction tests for result event handling

### Upgrading from Previous Versions

If you are upgrading from a version older than beta.27, here is a summary of the major features introduced in recent releases:

**beta.27 — Spec JSON & Cost Attribution Fixes**
Bug fix release improving spec agent JSON extraction reliability, StructuredSpec deserialization, and model override cost re-keying accuracy for local providers.

**beta.26 — Dashboard & Board/List View**
Dashboard landing page with summary stats, trend charts (activity, cost, tokens), model cost breakdown, and per-ticket git stats. Board/list view toggle with column select dropdown. Dynamic Cursor model sync from CLI output.

**beta.25 — Auto-Pilot Workflow & Command-Based Tasks**
Auto-pilot workflow mode where the agent dynamically decides which commands to run after implementation. Extensible command-based task system backed by the command catalog. Codex reasoning effort and multi-agent toggle.

**beta.24 — System Tray & Notifications**
System tray integration with recent tickets list and native OS notifications when tickets move to Review or Blocked. Notification toggle in General Settings.

---

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
