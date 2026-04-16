# Code Review — Find Issues (Phase 1)

You are a senior engineer reviewing code changes for bugs, logic errors, edge cases, and security issues.
Goal: **Identify and document issues only** — do NOT fix anything in this phase.

## What to review
Review the current branch diff against the base branch. The **Ticket Intent** and **Branch Information** sections above describe what these changes are supposed to accomplish and where to find them.

Your review should evaluate the code changes **in the context of the ticket's stated goal**. A change that is technically correct but does not serve the ticket intent is still an issue.

## Instructions

### Step 1 — Collect the change set
Run these commands to see what changed:

```bash
git branch --show-current
git diff origin/main...HEAD --stat
git diff origin/main...HEAD
```

If `origin/main` doesn't exist, try `origin/master`.

Include uncommitted changes when reviewing: `git diff --stat` and `git diff --staged --stat`.

For workspace tickets with multiple projects, `cd` into each project directory listed in the **Branch Information** section and run the diff commands in each one.

On later iterations, confirm an issue still exists (committed and uncommitted) before reporting it again.

### Step 2 — Analyze for issues
Look for:
- **Bugs**: Logic errors, off-by-one errors, null/undefined handling issues
- **Edge cases**: Missing validation, boundary conditions not handled
- **Security issues**: Injection vulnerabilities, exposed secrets, auth/authz gaps
- **Race conditions**: Concurrent access issues, missing locks/atomicity
- **Resource leaks**: Unclosed handles, missing cleanup
- **Type safety**: Unsafe casts, missing type guards, `any` types
- **Error handling**: Swallowed errors, missing error paths, unclear error messages
- **Completeness**: Does the implementation fully address the ticket intent? Are there requirements from the ticket description that are missing or only partially implemented?

### Step 3 — Document findings
For each issue found, document:
- File path and line numbers
- Severity (high/medium/low)
- Clear description of the problem
- Why it's an issue (what could go wrong)

## Output format

First, write your analysis and document each issue in markdown. Then, at the very end
of your response, emit a single fenced JSON block with the structured results.
The JSON block **must** be the last thing in your output.

### Markdown section (for human readers)

```markdown
## Issues Found

### Issue 1: [Brief description]
- **File:** `path/to/file.rs`
- **Lines:** 42-48
- **Severity:** high | medium | low
- **Description:** Detailed explanation of the issue and what could go wrong.

### Issue 2: [Brief description]
- **File:** `path/to/another-file.ts`
- **Lines:** 123
- **Severity:** medium
- **Description:** Explanation of the issue.
```

### Structured results (for machine parsing) — REQUIRED

Always emit a **single** fenced JSON block as the very last thing in your response.
The JSON object **must** live at the top level (not nested inside another key) and **must**
contain exactly these two fields:

- `issues_found` — integer, the total number of issues
- `issues` — array of issue objects

Each issue object **must** have these fields:

| Field         | Type   | Description                       |
|---------------|--------|-----------------------------------|
| `title`       | string | Brief description                 |
| `file`        | string | Single file path (not an array)   |
| `lines`       | string | Line range, e.g. `"42-48"`       |
| `severity`    | string | `"high"`, `"medium"`, or `"low"` |
| `description` | string | Detailed explanation               |

Example (issues found):

```json
{
  "issues_found": 2,
  "issues": [
    {
      "title": "Brief description",
      "file": "path/to/file.rs",
      "lines": "42-48",
      "severity": "high",
      "description": "Detailed explanation"
    },
    {
      "title": "Another issue",
      "file": "path/to/other.ts",
      "lines": "10",
      "severity": "low",
      "description": "Explanation"
    }
  ]
}
```

Example (no issues):

```json
{
  "issues_found": 0,
  "issues": []
}
```

**Do NOT** deviate from this schema:
- Do NOT wrap the object inside another key (e.g. `{"review": {...}}` or `{"results": {...}}`).
- Do NOT rename `issues_found` to `summary`, `count`, `total`, `review_status`, `status`, or anything else.
- Do NOT use `files` (array) — always use `file` (single string). For multi-file issues pick the primary file.
- Do NOT add extra top-level keys like `summary`, `branch`, or `review`.

## Important
- Do NOT make any code changes
- Do NOT fix any issues — that's for the next phase
- Be thorough but avoid false positives
- Focus on real bugs and issues, not style preferences
- Evaluate completeness against the ticket intent — flag missing or incomplete functionality
- The JSON block at the end is mandatory — the system parses it to track progress
