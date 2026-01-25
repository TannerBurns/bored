# Deslop (remove AI-generated code slop)

Remove AI code slop.
Check the diff against main and remove all AI-generated slop introduced in this branch.

## Scope
- Review **only changes introduced on this branch vs `origin/main`** (or `origin/master` if main doesn't exist).
- Prefer minimal edits that preserve behavior.
- Do not refactor unrelated code. Do not change public APIs unless necessary to remove slop safely.

## Guardrails
- Never add suppression comments (`eslint-disable`, `ts-ignore`, `nolint`, etc.).
- Never comment-out code as a "fix."
- Avoid drive-by formatting changes unless they are required to make the change coherent with file style.
- Keep code modular; attempt to keep files < 500 lines (soft cap).

---

## Step 1 — Determine base and collect the branch diff
Run:

- `git fetch --all --prune`
- `git branch --show-current`
- Try base branch in order:
  - `git rev-parse --verify origin/main`
  - `git rev-parse --verify origin/master`

Then gather the PR-style diff:

- If `origin/main` exists:
  - `git diff --stat origin/main...HEAD`
  - `git diff origin/main...HEAD`
- Else use `origin/master`:
  - `git diff --stat origin/master...HEAD`
  - `git diff origin/master...HEAD`

Also list changed files:
- `git diff --name-only origin/main...HEAD 2>/dev/null || git diff --name-only origin/master...HEAD`

If neither base exists, fall back to:
- `git log --oneline --decorate -n 20`
- Use the best available base ref and state what you used.

---

## Step 2 — Identify "AI slop" patterns in the diff
Review the diff and flag any of the following introduced by this branch:

### A) Comments that don't match team style
Remove or rewrite comments that are:
- Narration of obvious code ("Initialize X", "Handle error", "We now…")
- Redundant with function/variable names
- Overly verbose / tutorial-like
- Inconsistent tone (overly formal, chatty, hedging, or "AI voice")
- Comment blocks explaining what the code *already shows*
- Comments added only to justify unusual code rather than fixing the code

Rule of thumb:
- Keep only comments that add durable value: *why*, tricky invariants, non-obvious constraints, or references.

### B) Abnormal defensive code / error handling
Remove defensive checks that are unlikely / impossible in the given context, especially when called by trusted codepaths:
- `if (!x) return` / `if (x == null) throw` where upstream validation already guarantees x
- "Just in case" bounds checks, `typeof` checks, and guard clauses that don't exist elsewhere in that module/layer
- Try/catch blocks added around code that normally isn't caught in that layer
- Catch blocks that swallow errors, log-and-continue, or return partial results without precedent
- Redundant fallback values that hide bugs (e.g., `foo ?? ""` when empty string is not meaningful)

Rule of thumb:
- Error handling should match existing patterns in that area (same layer boundaries, same logging/metrics, same rethrow policy).

### C) "Any" casts and type escapes
Remove:
- `as any`, `<any>`, `unknown as X`, or broad casts used to bypass types
- `// @ts-ignore` or similar (not allowed anyway)
Replace with:
- Proper types, narrowed unions, schema validation at boundaries, or refactoring to make the types honest.

### D) Inconsistent style / structure
Remove or rewrite code that looks machine-generated:
- Over-abstracted helpers with one caller
- Excessively generic utility functions
- Unnatural naming ("handleProcessData", "doThingSafely", "performOperation")
- Excessively nested conditionals where codebase prefers early returns (or vice versa)
- Overly defensive defaulting and null coalescing
- Overuse of inline comments and step-by-step narration
- Unnecessary `async`/`await`, extra temporaries, verbose destructuring
- Repeated boilerplate not found elsewhere in file

---

## Step 3 — Apply fixes (edit code)
For each flagged item, make the smallest change that:
1) Removes the slop
2) Preserves intended behavior
3) Aligns with the existing style of the file and surrounding modules

### Required method
- Make changes directly (do not "TODO" or comment out).
- Keep edits tight and localized to diff hunks where possible.
- If removing defensive checks changes behavior, verify the guarantees:
  - Is the value already validated at the boundary?
  - Is the codepath trusted?
  - Do existing tests cover it?
If uncertain, prefer to align to existing conventions in that folder/layer.

---

## Step 4 — Validate (minimum verification)
Run the repo's standard checks (choose the canonical ones present):
- Lint
- Typecheck
- Build
- Tests (at least relevant ones)

If there is no established runner, at minimum:
- Ensure the project compiles/builds and types are clean for the touched area.

---

## Step 5 — Final output (STRICT)
At the end, report **only** a 1–3 sentence summary of what you changed.
No bullet points. No lists. No extra commentary.

Example format:
"Removed redundant narration-style comments and unnecessary defensive guards introduced on this branch, aligning error handling with existing module patterns. Replaced type escapes with proper narrowing and validation where needed; checks pass cleanly."
