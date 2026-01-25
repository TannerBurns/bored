# Review Changes (best practices + modularity) — APPLY FIXES

You are a senior engineer reviewing coding changes **and applying improvements immediately**.
Goal: ensure the most recent change set is correct, idiomatic, secure, testable, and modular—then **make the code match that**.

## What counts as "last step"
1) If there are **unstaged or staged changes**, review and improve those.
2) If the working tree is clean, review and improve the **most recent commit** (or the diff from the branch base if available).

## Guardrails (still apply)
- Make **only** changes that improve correctness, maintainability, security, testability, or modularity.
- Keep edits **minimal and targeted**; no drive-by refactors.
- Strong preference for **modular code** and **files < 500 lines** (soft cap, but always attempt).
- Do not change public APIs unless required to fix a real issue; if you must, update all call sites.
- Never silence tools via suppression comments (`eslint-disable`, `ts-ignore`, `nolint`, etc.) or by commenting-out code.

---

## Step 1 — Collect the change set (terminal commands)
Run these and use the outputs to drive the review:

### 1) Working tree
- `git status --porcelain`
- `git diff --stat`
- `git diff`
- `git diff --cached --stat`
- `git diff --cached`

### 2) If clean, last commit
- `git log -1 --oneline`
- `git show --stat`
- `git show`

### 3) If you can identify a base branch (main/master), also gather a PR-style diff
- `git branch --show-current`
- `git remote -v`
- Try one of:
  - `git diff origin/main...HEAD --stat`
  - `git diff origin/master...HEAD --stat`
  - `git diff origin/main...HEAD`
  - `git diff origin/master...HEAD`

Use the most relevant diff view (PR-style when available). Treat that as the authoritative "change set".

---

## Step 2 — Build an inventory (brief)
Create a short table of changed files with:
- File path
- Change type (added/modified/deleted/renamed)
- Purpose
- Risk level (low/med/high)
- Public API surface changes (exports, endpoints, schemas)

Use this to prioritize what to fix first.

---

## Step 3 — Enforce modularity + 500-line soft cap (and APPLY)
Check line counts:

- If working tree has changes:
  - `git diff --name-only | xargs -I{} sh -c 'test -f "{}" && wc -l "{}" | sed "s#^#{}: #"'`
- If reviewing last commit:
  - `git show --name-only --pretty="" | xargs -I{} sh -c 'test -f "{}" && wc -l "{}" | sed "s#^#{}: #"'`

### If any file > 500 lines
You MUST attempt to split it now (unless doing so would meaningfully increase risk for this patch).
Apply a split plan with:
- New file names (e.g., `service.ts`, `service.validators.ts`, `service.types.ts`, `service.test.ts`)
- Move cohesive groups (types/constants/helpers) into the new modules
- Update imports/exports to avoid circular deps
- Keep public surface area minimal
- Re-run checks after the split

If you decide not to split due to risk/time, state why and propose a follow-up plan.

---

## Step 4 — Best-practice checklist (apply as relevant) AND APPLY FIXES
### Correctness & behavior
- Ensure the diff matches intended behavior; fix edge cases you can infer from context.
- Ensure error paths are handled consistently (no silent swallowing).
- Avoid accidental behavior changes during refactors; fix regressions.

### API design & modularity
- Ensure single responsibility per module; no "god files".
- Keep public surface area minimal; make helpers private by default.
- Avoid circular dependencies; enforce clean layering.

### Readability & maintainability
- Improve naming for precision and consistency with surrounding code.
- Reduce nesting; use early returns if the codebase style does.
- Remove dead/duplicate code; avoid over-abstraction.
- Keep functions small and composable.

### Types / contracts (if applicable)
- Remove `any` escapes and unsafe casts.
- Add proper narrowing, validation at boundaries, and honest return types.

### Security & privacy
- Remove secrets/sensitive logs.
- Ensure authz/authn checks at boundaries (where applicable).
- Avoid unsafe parsing/templating patterns.

### Reliability & ops
- Ensure timeouts/retries only where the codebase expects them.
- Ensure resources are cleaned up.
- Keep logging structured and useful (and not noisy).

### Performance
- Avoid N+1 loops, repeated expensive work, and unnecessary allocations.
- Prefer streaming/pagination for large datasets when relevant.

### Tests & docs
- If the patch introduces new behavior/branches, add/adjust unit tests where existing patterns exist.
- Update docs/config examples if behavior changed.

---

## Step 5 — Apply changes (implementation rules)
While editing:
- Keep changes close to the diff hunks.
- Do not rewrite unrelated code.
- Prefer the project's existing patterns (look at neighboring files).
- If you introduce a new module/file, ensure naming and folder structure match the repo conventions.

---

## Step 6 — Validate
Run the repo's canonical checks (use Makefile/Justfile/scripts/CI hints):
- Lint
- Typecheck (if applicable)
- Build
- Tests (at least unit tests relevant to touched areas)

Iterate until green.

---

## Step 7 — Report (STRICT format)
Return results in exactly this structure:

### Summary
- 3–6 bullets on what changed and overall assessment.

### Changes applied
Group by file:
- File path
- What you changed (concise)
- Why (tie to best practice/modularity/correctness)

### Modularity / file-size actions
- List any files split (old -> new files)
- Brief note on moved symbols and boundaries

### Commands run
- Each command + pass/fail

### Risks / follow-ups
- Anything you intentionally did NOT change and why
- Any recommended next steps (minimal)
