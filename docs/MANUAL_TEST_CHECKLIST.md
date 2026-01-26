# Manual End-to-End Test Checklist

This document provides a comprehensive manual testing checklist for Agent Kanban, starting from a fresh application state.

---

## Prerequisites

Before starting, ensure:
- [X] Application has been built (`npm run tauri build` or `npm run tauri dev`)
- [X] Cursor IDE is installed (for Cursor agent tests)
- [X] Claude Code CLI is installed (for Claude agent tests) - optional

---

## Phase 0: Test Project Setup (File System)

Before launching the app, create a controlled test project on your file system. This gives agents real code to work on with verifiable outcomes.

> **Do this FIRST** before launching Agent Kanban.

### 0.1 Create the Test Project

1. Create a new directory for testing:
```bash
mkdir -p ~/agent-kanban-test-project
cd ~/agent-kanban-test-project
git init
```

2. Create the initial project structure:
```bash
# Create package.json
cat > package.json << 'EOF'
{
  "name": "agent-kanban-test-project",
  "version": "1.0.0",
  "description": "Test project for Agent Kanban verification",
  "main": "src/index.js",
  "scripts": {
    "start": "node src/index.js",
    "test": "node src/test.js"
  }
}
EOF

# Create src directory
mkdir -p src

# Create main file with intentional issues
cat > src/index.js << 'EOF'
// Main entry point for the test application

function greet(name) {
  console.log("Hello, " + name)
}

function add(a, b) {
  return a + b
}

function subtract(a, b) {
  return a - b
}

// TODO: Add a multiply function

// TODO: Add a divide function with zero-check

module.exports = { greet, add, subtract }
EOF

# Create a test file with a failing test
cat > src/test.js << 'EOF'
const { greet, add, subtract } = require('./index.js')

console.log('Running tests...')

// Test add function
const addResult = add(2, 3)
if (addResult !== 5) {
  console.error('FAIL: add(2, 3) should be 5, got', addResult)
  process.exit(1)
}
console.log('PASS: add(2, 3) = 5')

// Test subtract function
const subResult = subtract(5, 3)
if (subResult !== 2) {
  console.error('FAIL: subtract(5, 3) should be 2, got', subResult)
  process.exit(1)
}
console.log('PASS: subtract(5, 3) = 2')

console.log('All tests passed!')
EOF

# Create README
cat > README.md << 'EOF'
# Test Project

A simple test project for verifying Agent Kanban functionality.

## Usage

```bash
npm start
npm test
```
EOF
```

3. Commit the initial state:
```bash
git add .
git commit -m "Initial test project setup"
```

### 0.2 Test Tickets Reference

> **Note:** Don't create these tickets yet! This is a reference section. You will create these tickets in **Phase 4: Ticket Management** after setting up the app, project, and board.

The following tickets are designed to verify agent work with specific, verifiable outcomes:

---

#### Ticket 1: Add Multiply Function (Simple Code Addition)

**Title:** Add multiply function to index.js

**Description:**
```markdown
Add a `multiply(a, b)` function to `src/index.js` that multiplies two numbers and returns the result.

Requirements:
- Function should be named `multiply`
- Should take two parameters: `a` and `b`
- Should return `a * b`
- Export the function in module.exports

Also add a test for it in `src/test.js` that verifies `multiply(3, 4) === 12`.
```

**Priority:** Medium  
**Expected Outcome:**
- [ ] `src/index.js` contains a `multiply` function
- [ ] `multiply` is exported in `module.exports`
- [ ] `src/test.js` contains a test for multiply
- [ ] Running `npm test` passes (exits with code 0)
- [ ] Git shows changes to both files

---

#### Ticket 2: Add Divide Function with Safety Check (Logic Implementation)

**Title:** Add divide function with zero-division protection

**Description:**
```markdown
Add a `divide(a, b)` function to `src/index.js` that divides two numbers.

Requirements:
- Function should be named `divide`
- Should take two parameters: `a` and `b`
- If `b` is 0, throw an Error with message "Cannot divide by zero"
- Otherwise return `a / b`
- Export the function in module.exports

Add tests in `src/test.js`:
1. Test that `divide(10, 2) === 5`
2. Test that `divide(5, 0)` throws an error
```

**Priority:** High  
**Expected Outcome:**
- [ ] `src/index.js` contains a `divide` function
- [ ] `divide` throws an error when dividing by zero
- [ ] `divide` is exported in `module.exports`
- [ ] `src/test.js` contains tests for both cases
- [ ] Running `npm test` passes

---

#### Ticket 3: Fix Code Style (Formatting Task)

**Title:** Add semicolons to all statements in index.js

**Description:**
```markdown
The current `src/index.js` is missing semicolons at the end of some statements.

Review the file and ensure all statements end with semicolons for consistency.
```

**Priority:** Low  
**Expected Outcome:**
- [ ] All statements in `src/index.js` end with semicolons
- [ ] Code still runs correctly (`npm test` passes)

---

#### Ticket 4: Update README (Documentation Task)

**Title:** Update README with function documentation

**Description:**
```markdown
Update `README.md` to document all the available functions in the project.

Include:
- A "Functions" or "API" section
- List each function with a brief description
- Show example usage for each function
```

**Priority:** Low  
**Expected Outcome:**
- [ ] README.md contains a Functions/API section
- [ ] All functions (greet, add, subtract, multiply, divide) are documented
- [ ] Example usage is provided

---

### 0.3 Verification Workflow (Reference)

> **Note:** Use this workflow in **Phase 5** and **Phase 6** when verifying agent work.

After each agent completes a ticket:

1. **Check Git Status:**
```bash
cd ~/agent-kanban-test-project
git status
git diff
```

2. **Run Tests:**
```bash
npm test
```

3. **Verify Specific Changes:**
   - Open the modified files
   - Confirm the expected changes were made
   - Check for any unintended side effects

4. **Commit if Satisfied:**
```bash
git add .
git commit -m "Completed: [ticket title]"
```

### 0.4 Reset Test Project (Reference)

To reset the project between test runs or for re-testing:
```bash
cd ~/agent-kanban-test-project
git checkout .
git clean -fd
```

Or to fully reset to initial state:
```bash
git reset --hard HEAD~N  # where N is number of commits to undo
```

### 0.5 Verify Test Project Setup

Before proceeding, confirm the test project is ready:

- [X] Directory exists: `~/agent-kanban-test-project`
- [X] Git repo initialized: `git status` works
- [X] Initial commit exists: `git log` shows one commit
- [X] Test passes: `npm test` outputs "All tests passed!"

**You are now ready to launch Agent Kanban and proceed with Phase 1.**

---

## Phase 1: Fresh Application State

### 1.1 Initial Launch
- [X] Launch the application
- [X] Verify the app opens without errors
- [X] Verify dark theme is applied by default
- [X] Verify sidebar shows: Boards, Agent Runs, Workers, Settings

### 1.2 Empty State Verification
- [X] Navigate to **Boards** - should show "No boards yet" message
- [X] Verify "Create Your First Board" button is displayed
- [X] Navigate to **Agent Runs** - should show "No active runs" message
- [X] Navigate to **Workers** - should show "No workers running" and queue counts of 0
- [X] Navigate to **Settings** - should display General tab by default

---

## Phase 2: Settings Configuration

### 2.1 General Settings
- [X] Navigate to **Settings > General**
- [X] Verify default agent preference options: Any, Cursor, Claude
- [X] Verify theme options: Light, Dark, System
- [X] Switch to **Light** theme - verify UI updates
- [X] Switch to **System** theme - verify it follows OS preference
- [X] Switch back to **Dark** theme

### 2.2 Projects Setup

> **Important:** Use the test project created in the "Test Scenario" section (`~/agent-kanban-test-project`)

- [X] Navigate to **Settings > Projects**
- [X] Verify "No projects added yet" message
- [X] Click **+ Add Project** button
- [X] Verify the add project form appears with:
  - Name input field
  - Path input field with Browse button
  - Preferred Agent dropdown
- [X] Click **Browse** and navigate to `~/agent-kanban-test-project`
- [X] Verify path is populated: `/Users/[you]/agent-kanban-test-project`
- [X] Verify name is auto-filled: "agent-kanban-test-project"
- [X] Rename to "Test Project" for clarity
- [X] Select a preferred agent (or leave as "No preference")
- [X] Click **Add Project**
- [X] Verify project appears in the list with:
  - Project name: "Test Project"
  - Path displayed in monospace font
  - Any configured preferences shown as badges

### 2.3 Cursor Settings
- [X] Navigate to **Settings > Cursor**
- [X] Review the Cursor configuration options
- [X] Note the hook script path if displayed
- [X] Verify any validation status indicators

### 2.4 Claude Code Settings
- [X] Navigate to **Settings > Claude Code**
- [X] Review the Claude configuration options
- [X] Note the hook script path if displayed
- [X] Verify any validation status indicators

### 2.5 Data Settings
- [X] Navigate to **Settings > Data**
- [X] Review available data management options
- [X] Note database location if displayed

---

## Phase 3: Board Management

### 3.1 Create First Board
- [ ] Navigate to **Boards** (sidebar shows empty boards section with "No boards yet")
- [ ] Click **Create Your First Board** button in the main content area
- [ ] Verify **Create Board** modal appears with:
  - Board Name input field
  - Create and Cancel buttons
- [ ] Enter board name: "Test Board"
- [ ] Click **Create**
- [ ] Verify:
  - Board is created and selected automatically
  - Board name "Test Board" appears in the header
  - Board appears in the sidebar under "Boards" section
  - Kanban columns appear:
    - Backlog
    - Ready
    - In Progress
    - Blocked
    - Review
    - Done
  - **New Ticket** button appears in header

### 3.2 Create Additional Boards
- [ ] Click the **+** button next to "Boards" in the sidebar
- [ ] Verify **Create Board** modal appears
- [ ] Enter board name: "Second Board"
- [ ] Click **Create**
- [ ] Verify:
  - New board is created and automatically selected
  - "Second Board" appears in the header
  - Both boards appear in the sidebar

### 3.3 Switch Between Boards
- [ ] Click on "Test Board" in the sidebar
- [ ] Verify:
  - "Test Board" becomes highlighted in the sidebar
  - Header shows "Test Board"
  - Any tickets created in Test Board are displayed
- [ ] Click on "Second Board" in the sidebar
- [ ] Verify:
  - "Second Board" becomes highlighted
  - Header shows "Second Board"
  - Board shows empty columns (no tickets yet)

### 3.4 Board Persistence
- [ ] Close and restart the application
- [ ] Verify:
  - Both boards still appear in the sidebar
  - First board is automatically selected on load
  - Board data is preserved

### 3.5 Rename Board
- [ ] Hover over a board in the sidebar
- [ ] Verify a three-dot menu icon appears
- [ ] Click the three-dot menu icon
- [ ] Verify dropdown menu appears with "Rename" and "Delete" options
- [ ] Click **Rename**
- [ ] Verify **Rename Board** modal appears with:
  - Input field pre-filled with current board name
  - Save and Cancel buttons
- [ ] Change the name to "Renamed Board"
- [ ] Click **Save**
- [ ] Verify:
  - Modal closes
  - Board name updates in the sidebar
  - Header shows new board name (if this board is selected)

### 3.6 Delete Board
- [ ] Create a test board named "To Delete" (using the + button)
- [ ] Click the three-dot menu on "To Delete" board
- [ ] Click **Delete**
- [ ] Verify confirmation dialog appears
- [ ] Click **Cancel** (or dismiss the dialog)
- [ ] Verify board is NOT deleted
- [ ] Click **Delete** again and confirm
- [ ] Verify:
  - Board is removed from the sidebar
  - If it was the current board, another board is automatically selected
  - If no boards remain, empty state is shown

### 3.7 Delete Board with Tickets
- [ ] Select a board that has tickets
- [ ] Click the three-dot menu and select **Delete**
- [ ] Verify confirmation dialog mentions the number of tickets that will be deleted
- [ ] Confirm deletion
- [ ] Verify board and all its tickets are deleted

---

## Phase 4: Ticket Management

> **Important:** You should now have a board created (from Phase 3) and the "Test Project" registered (from Phase 2.2).

### 4.1 Create First Ticket (UI Verification)
- [ ] Click **New Ticket** button in the header
- [ ] Verify Create Ticket modal opens with:
  - Title input
  - Description textarea (Markdown supported)
  - Priority dropdown (Low, Medium, High, Urgent)
  - Labels input
  - Column selector
  - Project selector (should show "Test Project")
  - Agent preference selector
- [ ] Enter a simple test ticket:
  - Title: "UI Test Ticket"
  - Description: "This is a test ticket for UI verification"
  - Priority: Low
  - Column: Backlog
  - Project: **Test Project**
  - Agent Pref: Any
- [ ] Click **Create**
- [ ] Verify ticket appears in the Backlog column
- [ ] Verify ticket card shows:
  - Title
  - Priority indicator

### 4.2 Create Test Scenario Tickets

Now create the specific test tickets from **Phase 0.2** that agents will work on.

**Create Ticket 1 - Multiply Function:**
- [ ] Click **New Ticket**
- [ ] Title: `Add multiply function to index.js`
- [ ] Description: Copy from Phase 0.2, Ticket 1
- [ ] Priority: **Medium**
- [ ] Column: **Backlog**
- [ ] Project: **Test Project**
- [ ] Agent Pref: Any
- [ ] Click **Create**

**Create Ticket 2 - Divide Function:**
- [ ] Click **New Ticket**
- [ ] Title: `Add divide function with zero-division protection`
- [ ] Description: Copy from Phase 0.2, Ticket 2
- [ ] Priority: **High**
- [ ] Column: **Backlog**
- [ ] Project: **Test Project**
- [ ] Agent Pref: Any
- [ ] Click **Create**

**Create Ticket 3 - Code Style:**
- [ ] Click **New Ticket**
- [ ] Title: `Add semicolons to all statements in index.js`
- [ ] Description: Copy from Phase 0.2, Ticket 3
- [ ] Priority: **Low**
- [ ] Column: **Backlog**
- [ ] Project: **Test Project**
- [ ] Agent Pref: Any
- [ ] Click **Create**

**Create Ticket 4 - README Update:**
- [ ] Click **New Ticket**
- [ ] Title: `Update README with function documentation`
- [ ] Description: Copy from Phase 0.2, Ticket 4
- [ ] Priority: **Low**
- [ ] Column: **Backlog**
- [ ] Project: **Test Project**
- [ ] Agent Pref: Any
- [ ] Click **Create**

**Create a Ticket Without Project (for error testing):**
- [ ] Click **New Ticket**
- [ ] Title: "No Project Ticket"
- [ ] Description: "Test ticket with no project assigned"
- [ ] Priority: Medium
- [ ] Column: Backlog
- [ ] Project: **Leave empty/none**
- [ ] Click **Create**

### 4.3 Verify Ticket List
- [ ] Verify all 6 tickets appear in the Backlog column
- [ ] Verify tickets show correct priority indicators
- [ ] Verify the "No Project Ticket" is distinguishable (may show warning)

### 4.4 Drag and Drop
- [ ] Drag the "UI Test Ticket" from **Backlog** to **Ready**
- [ ] Verify ticket moves to Ready column
- [ ] Verify ticket's updatedAt timestamp changes
- [ ] Drag ticket from **Ready** to **In Progress**
- [ ] Drag ticket from **In Progress** to **Review**
- [ ] Drag ticket from **Review** to **Done**
- [ ] Verify all transitions work smoothly
- [ ] Drag ticket back from Done to Backlog
- [ ] Verify reverse transition works

### 4.5 View Ticket Details
- [ ] Click on a ticket card
- [ ] Verify Ticket Modal opens with:
  - Title (editable)
  - Full description
  - Priority badge
  - Labels
  - Project assignment
  - Agent preference
  - Column selector
  - Comments section
  - Agent Controls section (Run with Cursor / Run with Claude)
  - Previous Runs section (if applicable)

### 4.6 Edit Ticket
- [ ] In the ticket modal, edit the title
- [ ] Verify title updates in real-time
- [ ] Edit the description
- [ ] Change the priority
- [ ] Add/remove labels
- [ ] Change the assigned project
- [ ] Verify all changes persist after closing and reopening modal

### 4.7 Add Comments
- [ ] Open a ticket modal
- [ ] Scroll to Comments section
- [ ] Enter a comment: "Test comment 1"
- [ ] Submit the comment
- [ ] Verify comment appears with:
  - Comment text
  - Author type (user)
  - Timestamp
- [ ] Add another comment
- [ ] Verify comments are ordered chronologically
- [ ] Close and reopen the ticket modal
- [ ] Verify comments persist

### 4.8 Prepare Tickets for Agent Testing

Move test tickets to the Ready column so agents can work on them:

- [ ] Drag **"Add multiply function to index.js"** to the **Ready** column
- [ ] Verify ticket is now in Ready column
- [ ] Keep the other test tickets in Backlog for now (we'll use them later)

**Current State Check:**
- [ ] Backlog: 4 tickets (UI Test, Divide, Semicolons, README, No Project)
- [ ] Ready: 1 ticket (Multiply function)
- [ ] All other columns: empty

---

## Phase 5: Agent Integration

> **Important:** This phase uses the test project and tickets defined in the "Test Scenario" section above. Ensure you have created the test project at `~/agent-kanban-test-project` before proceeding.

### 5.1 Agent Controls - Prerequisites
- [ ] Create or select a ticket with:
  - A valid project assigned (the test project)
  - Ticket is NOT locked by a run
- [ ] Open the ticket modal
- [ ] Verify Agent Controls section shows:
  - "Run with Cursor" button (purple)
  - "Run with Claude" button (green)
  - Both buttons should be enabled

### 5.2 Agent Controls - No Project Warning
- [ ] Create or select a ticket WITHOUT a project assigned
- [ ] Open the ticket modal
- [ ] Verify Agent Controls shows warning: "Assign a project to this ticket..."
- [ ] Verify both Run buttons are disabled

### 5.3 Run with Cursor Agent - Test Ticket 1 (Multiply Function)
> Note: This requires Cursor IDE to be properly configured

**Setup:**
- [ ] Create **Test Ticket 1** (Add multiply function) from the Test Scenario section
- [ ] Assign it to the test project (`~/agent-kanban-test-project`)
- [ ] Move ticket to the **Ready** column

**Execution:**
- [ ] Open the ticket modal
- [ ] Click **Run with Cursor**
- [ ] Verify:
  - Button changes to "Running..."
  - Cancel button appears
  - Output section appears (may be empty initially)
- [ ] Wait for agent to complete
- [ ] Verify:
  - Output logs appear in the Output section
  - Run completes with "finished" status
  - Previous Runs section shows the run

**Outcome Verification:**
- [ ] Open terminal and navigate to test project:
  ```bash
  cd ~/agent-kanban-test-project
  ```
- [ ] Check git status shows changes:
  ```bash
  git status
  ```
- [ ] Verify `src/index.js` contains a `multiply` function
- [ ] Verify `multiply` is exported in `module.exports`
- [ ] Verify `src/test.js` contains a test for multiply
- [ ] Run tests and verify they pass:
  ```bash
  npm test
  ```
- [ ] Commit the changes:
  ```bash
  git add . && git commit -m "Add multiply function"
  ```

### 5.4 Run with Claude Agent - Test Ticket 2 (Divide Function)
> Note: This requires Claude Code CLI to be installed

**Setup:**
- [ ] Create **Test Ticket 2** (Add divide function) from the Test Scenario section
- [ ] Assign it to the test project
- [ ] Move ticket to the **Ready** column

**Execution:**
- [ ] Open the ticket modal
- [ ] Click **Run with Claude**
- [ ] Verify same UI behavior as Cursor agent
- [ ] Wait for agent to complete

**Outcome Verification:**
- [ ] Check git status shows changes
- [ ] Verify `src/index.js` contains a `divide` function
- [ ] Verify `divide` throws error when dividing by zero
- [ ] Verify `divide` is exported in `module.exports`
- [ ] Verify `src/test.js` contains both divide tests
- [ ] Run tests and verify they pass:
  ```bash
  npm test
  ```
- [ ] Commit the changes:
  ```bash
  git add . && git commit -m "Add divide function with zero check"
  ```

### 5.5 Cancel Agent Run
- [ ] Create a new test ticket with a complex task
- [ ] Start an agent run on the ticket
- [ ] Click **Cancel** while still running
- [ ] Verify run is cancelled (status shows "aborted")
- [ ] Verify ticket becomes unlocked
- [ ] Verify partial changes (if any) in the test project

### 5.6 Agent Error Handling
- [ ] Create a ticket with an impossible or unclear task
- [ ] Run an agent on it
- [ ] Observe how the agent handles the situation
- [ ] Verify ticket moves to **Blocked** on failure
- [ ] Verify error information is captured in the run

### 5.7 Agent Runs View
- [ ] Navigate to **Agent Runs** in sidebar
- [ ] If any runs are in progress, verify they appear with:
  - Ticket title
  - Agent type
  - "In Progress" status indicator
- [ ] Verify completed runs show appropriate status

### 5.8 Run Comparison (Optional)
> Compare the same task completed by different agents

- [ ] Reset the test project to initial state:
  ```bash
  cd ~/agent-kanban-test-project
  git reset --hard HEAD~2  # Undo the multiply and divide commits
  ```
- [ ] Create a duplicate of Test Ticket 1
- [ ] Run with Cursor on the original
- [ ] Run with Claude on the duplicate
- [ ] Compare:
  - Time to completion
  - Code quality/style
  - Test coverage
  - Any differences in implementation

---

## Phase 6: Worker Mode

> **Important:** This phase tests automated ticket processing. Use the test project and remaining test tickets (3 and 4) from the Test Scenario section.

### 6.1 Worker Panel Overview
- [ ] Navigate to **Workers** in sidebar
- [ ] Verify Queue Status cards show:
  - Ready count
  - In Progress count
  - Workers count (should be 0 initially)
- [ ] Verify "Start New Worker" section with:
  - Agent type radio buttons (Cursor/Claude)
  - Project filter dropdown
  - Validation status area
  - Start Worker button

### 6.2 Worker Validation
- [ ] Select the test project from the dropdown
- [ ] Verify validation runs automatically
- [ ] Check validation results:
  - Green "Environment Ready" if all checks pass
  - Red "Environment Issues" with specific issues if not
- [ ] If issues exist, verify "Fix" buttons appear for fixable issues
- [ ] Click Fix buttons to attempt auto-remediation
- [ ] Re-validate after fixes

### 6.3 Setup Test Tickets for Worker Processing

**Prepare the queue:**
- [ ] Create **Test Ticket 3** (Fix code style - add semicolons)
- [ ] Create **Test Ticket 4** (Update README documentation)
- [ ] Assign both tickets to the test project
- [ ] Move both tickets to the **Ready** column
- [ ] Verify Queue Status shows Ready count of 2

### 6.4 Start a Worker
- [ ] Select agent type (Cursor or Claude)
- [ ] Select the test project from dropdown
- [ ] Ensure validation passes (green status)
- [ ] Click **Start Worker**
- [ ] Verify:
  - Worker appears in "Active Workers" section
  - Worker shows "idle" or "running" status
  - Queue Status updates (Workers count increases to 1)

### 6.5 Worker Processing - Automated Ticket Handling
- [ ] Observe the worker pick up the first Ready ticket
- [ ] Verify:
  - Ready count decreases
  - In Progress count increases
  - Ticket moves from Ready to In Progress column
  - Worker shows which ticket it's working on
- [ ] Wait for the agent to complete the ticket
- [ ] Verify:
  - Ticket moves to **Review** (on success) or **Blocked** (on error)
  - In Progress count decreases
  - "Tickets processed" counter on worker increases
- [ ] Observe the worker pick up the next Ready ticket
- [ ] Wait for completion
- [ ] Verify both tickets were processed

### 6.6 Verify Worker Outcomes

**After worker completes Test Ticket 3 (semicolons):**
- [ ] Check git status in test project
- [ ] Verify all statements in `src/index.js` end with semicolons
- [ ] Verify `npm test` still passes
- [ ] Commit changes if correct

**After worker completes Test Ticket 4 (README):**
- [ ] Verify `README.md` was updated
- [ ] Verify it documents all functions
- [ ] Verify example usage is included
- [ ] Commit changes if correct

### 6.7 Stop Worker
- [ ] Click **Stop** on the active worker
- [ ] Verify worker is removed from Active Workers
- [ ] Verify Workers count decreases in Queue Status

### 6.8 Multiple Workers (Stress Test)
- [ ] Create 3-4 additional simple tickets in Ready column
- [ ] Start 2 workers (one Cursor, one Claude if available)
- [ ] Observe both workers processing tickets in parallel
- [ ] Verify no conflicts or race conditions
- [ ] Verify all tickets are eventually processed
- [ ] Click **Stop All** button
- [ ] Verify all workers are stopped

---

## Phase 7: Real-Time Updates

### 7.1 SSE Event Stream
- [ ] Open browser dev tools Network tab
- [ ] Filter for "stream" or "events"
- [ ] Verify SSE connection is established to `/v1/stream`
- [ ] Perform actions (create ticket, move ticket)
- [ ] Verify events flow through the stream

### 7.2 Multi-Window Sync (if applicable)
- [ ] Open the app in two windows (if supported)
- [ ] Create a ticket in one window
- [ ] Verify it appears in the other window
- [ ] Move a ticket in one window
- [ ] Verify the move is reflected in the other window

---

## Phase 8: Error Handling

### 8.1 Invalid Project Path
- [ ] Go to Settings > Projects
- [ ] Add a project with a non-existent path
- [ ] Verify appropriate error message
- [ ] Delete the invalid project

### 8.2 Agent Spawn Failure
- [ ] Try to run an agent without proper configuration
- [ ] Verify error message is displayed
- [ ] Verify ticket is not left in locked state

### 8.3 Network/API Errors
- [ ] (If testable) Simulate API failures
- [ ] Verify error messages are displayed
- [ ] Verify app recovers gracefully

---

## Phase 9: Data Persistence

### 9.1 App Restart Persistence
- [ ] Create several tickets across different columns
- [ ] Add comments to tickets
- [ ] Close the application completely
- [ ] Reopen the application
- [ ] Verify all data persists:
  - Board exists
  - All tickets are in correct columns
  - Comments are preserved
  - Projects are preserved
  - Settings are preserved

### 9.2 Lock Expiration Recovery
- [ ] (If testable) Create a scenario where a ticket lock expires
- [ ] Verify ticket is unlocked and can be worked on again

---

## Phase 10: UI/UX Verification

### 10.1 Responsive Layout
- [ ] Resize the window smaller
- [ ] Verify columns scroll horizontally if needed
- [ ] Verify no UI elements are cut off
- [ ] Verify modals remain usable

### 10.2 Keyboard Navigation
- [ ] Press Escape in an open modal
- [ ] Verify modal closes
- [ ] Tab through form elements
- [ ] Verify focus states are visible

### 10.3 Loading States
- [ ] Verify loading spinner appears when data is loading
- [ ] Verify buttons show loading state when processing

### 10.4 Empty States
- [ ] Verify all empty states have helpful messages
- [ ] Verify empty states include calls-to-action where appropriate

---

## Test Summary

| Phase | Total Checks | Passed | Failed | Notes |
|-------|-------------|--------|--------|-------|
| 0. Test Project Setup | 4 | | | File system setup |
| 1. Fresh Application | 9 | | | |
| 2. Settings Configuration | 20 | | | |
| 3. Board Management | 3 | | | |
| 4. Ticket Management | 45 | | | Includes test ticket creation |
| 5. Agent Integration | 35 | | | Includes outcome verification |
| 6. Worker Mode | 28 | | | Includes outcome verification |
| 7. Real-Time Updates | 4 | | | |
| 8. Error Handling | 5 | | | |
| 9. Data Persistence | 3 | | | |
| 10. UI/UX Verification | 7 | | | |
| **TOTAL** | **163** | | | |

---

## Issue Log

| # | Phase | Description | Severity | Status |
|---|-------|-------------|----------|--------|
| | | | | |

---

## Notes

- **Date Tested**: _______________
- **Tester**: _______________
- **App Version**: _______________
- **OS**: _______________
- **Additional Notes**:



---

## Cleanup

After testing:
- [ ] Delete test tickets if desired
- [ ] Remove test project from Settings > Projects if desired
- [ ] Reset to preferred theme
