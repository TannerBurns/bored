# Changelog

All notable changes to Bored are documented in this file.

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
