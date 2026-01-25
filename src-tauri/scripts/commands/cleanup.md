# Cleanup (lint/build warnings + dead code removal)

You are a senior engineer doing a "cleanup" pass.
Goal: eliminate **all warnings and errors** from lint/build/typecheck, and **remove dead code** safely.
We must NEVER:
- comment code out instead of fixing/removing it
- add comments to silence compiler/linter
- disable rules (`eslint-disable`, `ts-ignore`, `nolint`, etc.)
- weaken configs to make warnings disappear

If something is a true false-positive, fix it by improving types/structure/config *properly* (not suppressing).

---

## Step 0 — Gather context (no changes yet)
Run:
- `git status --porcelain`
- `git diff --stat`

If there are unstaged changes unrelated to cleanup, call them out as risk.

---

## Step 1 — Identify the project toolchain
Inspect repo conventions to decide what commands to run:
- Check for: `package.json`, `pnpm-lock.yaml`, `yarn.lock`, `bun.lockb`
- Check for: `tsconfig.json`, `.eslintrc*`, `eslint.config.*`, `.prettierrc*`
- Check for: `go.mod`, `Cargo.toml`, `pyproject.toml`, `requirements.txt`, `Makefile`, `Justfile`
- Check for CI hints: `.github/workflows/*`, `turbo.json`, `nx.json`

Pick the most canonical commands (prefer `make`, `just`, or documented scripts).

---

## Step 2 — Run lint/typecheck/build and capture ALL warnings/errors
Run the appropriate set. Prefer these (in order) if present:

### If Makefile/Justfile exists
- `make lint` / `just lint`
- `make typecheck` / `just typecheck`
- `make build` / `just build`
- `make test` / `just test`

### If Node/TS
- `pnpm -s lint` / `npm run -s lint` / `yarn -s lint`
- `pnpm -s typecheck` (or `tsc -p tsconfig.json --noEmit`)
- `pnpm -s build`
- `pnpm -s test` (if available)

### If Go
- `go test ./...`
- `go vet ./...`
- `gofmt -w .` (only if formatting is part of your standards)
- If repo uses `golangci-lint`, run it (often `golangci-lint run`)

### If Python
- `python -m compileall .`
- `pytest` (if present)
- Lint tools if configured: `ruff`, `flake8`, `pylint`, `mypy`

### If Rust
- `cargo test`
- `cargo clippy -- -D warnings`
- `cargo build`

Important:
- If any command is missing, find the closest equivalent in scripts/CI and run that.
- Capture output verbatim and treat warnings as work items.

---

## Step 3 — Fix issues the right way (no suppression)
For each warning/error:
1) Identify the root cause.
2) Fix by changing code (or removing dead code).
3) Keep changes minimal and safe.
4) Re-run the exact failing command to confirm it's gone.

Rules:
- **Do not** add ignore comments or disable lint rules.
- **Do not** "comment out" code. Remove it or fix it.
- If a rule seems unreasonable, note it as a discussion item, but do not change policy unless instructed.

---

## Step 4 — Dead code detection & removal
Use repo-appropriate tools *if they already exist*, otherwise use static cues (unused exports, unreachable code, unused files).

### General signals of dead code
- Unused imports/vars/params
- Unreferenced exports
- Files not imported anywhere
- Feature-flag branches that can never be true
- Old endpoints/handlers not routed
- Duplicate utilities with no callers

### If TypeScript/JavaScript
- Lean on ESLint output for unused vars/imports.
- If the repo already uses a tool like `ts-prune`, `knip`, or `depcheck`, run it and act on results.
- Prefer removing unused exports/functions over keeping "just in case" code.

### If Go
- `go test ./...` + `go vet ./...` are your baseline.
- Remove unused functions/types that are not referenced.
- Watch for build tags / OS-specific files before deleting.

### If Python
- Remove unused functions/modules not imported anywhere.
- Watch for dynamic imports / plugin registration patterns.

### Safe removal protocol
For each suspected dead symbol/file:
1) Find references (ripgrep):
   - `rg "SymbolName" -n .`
2) Confirm it's truly unused (consider reflection/dynamic registration patterns).
3) Remove it.
4) Ensure tests/build still pass.
5) If uncertain, prefer leaving it **but** document it as a "possible dead code" finding (do not suppress).

---

## Step 5 — Verify zero warnings/errors
Re-run the same commands from Step 2 until:
- lint: clean
- typecheck: clean
- build: clean
- tests: pass (if applicable)
No warnings tolerated.

If the toolchain inherently prints non-actionable warnings (rare), treat that as a policy conflict and report it clearly with the exact output and source.

---

## Step 6 — Output (strict format)
Return results in exactly this structure:

### Commands run
- List each command and whether it succeeded.

### Issues found
Group by tool:
- Lint
- Typecheck
- Build
- Tests
For each issue:
- File + location
- Exact warning/error message (short excerpt)
- Fix applied (what changed)

### Dead code removed
- Symbols removed (functions/classes/exports)
- Files deleted (if any)
- Why it was safe (no references + tests/build clean)

### Final status
- Confirm: **0 warnings, 0 errors** (or explain what remains and why it cannot be addressed without changing policy)

---

## Optional: If I ask you to "apply cleanup"
If I say "apply cleanup":
1) Make changes to fix all warnings/errors and remove dead code.
2) Do not add suppression comments or disable rules.
3) Keep modules cohesive; avoid creating large files.
4) Re-run the failing checks until clean.
5) Summarize changes and list any follow-up refactors as separate suggestions.
