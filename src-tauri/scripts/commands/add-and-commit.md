# Add + Commit (apply code changes and create the commit)

You are a senior engineer. Add the requested change(s), run checks, stage the correct files, and create the commit with a detailed message that matches repo conventions.

IMPORTANT:
- This command must be fully actionable end-to-end: it should result in a new commit.
- When you output the commit message, DO NOT wrap it in triple backticks.
- Do not ask the user whether to proceed; proceed.

## Guardrails
- Keep changes scoped to the request; no drive-by refactors.
- Prefer modular code; attempt to keep files < 500 lines (soft cap).
- Never silence tools via suppression comments (`eslint-disable`, `ts-ignore`, `nolint`, etc.) or by commenting-out code.
- If you remove dead code or split files for modularity, keep it clearly related to the requested change.
- If you must change a public API, update all call sites and tests accordingly.

---

## Step 1 — Understand repo + current state
Run:
- `git status --porcelain`
- `git branch --show-current`
- `git remote -v`
- `git log -10 --oneline --decorate`

Detect tooling:
- Check for `Makefile` / `Justfile`
- For Node: `package.json` + lockfile (`pnpm-lock.yaml`, `yarn.lock`, `package-lock.json`, `bun.lockb`)
- For other ecosystems: `go.mod`, `Cargo.toml`, `pyproject.toml`

Identify the canonical commands (prefer Makefile/Justfile or package scripts used in CI).

---

## Step 2 — Plan (brief) then implement
Write a short plan (2–6 bullets) stating:
- What you will change/add
- Acceptance criteria implied by the request
- Key files/modules touched

Then implement using local conventions and nearby patterns.

Implementation rules:
- Add/update unit tests for changed logic (at least cover new branches/behaviors).
- Keep edits tight and localized; avoid unrelated refactors.
- Keep naming, structure, and patterns consistent with surrounding code.

---

## Step 3 — Validate locally (must be clean)
Run the repo's canonical commands. Prefer these if present:

### If Makefile/Justfile
- `make lint` / `just lint`
- `make typecheck` / `just typecheck`
- `make test` / `just test`
- `make build` / `just build`

### If Node/TS (use the detected package manager)
- `pnpm -s lint` / `npm run -s lint` / `yarn -s lint`
- `pnpm -s typecheck` (or `tsc -p tsconfig.json --noEmit`)
- `pnpm -s test`
- `pnpm -s build`

### If Go
- `go test ./...`
- `go vet ./...`
- `golangci-lint run` (if configured)

### If Python
- `python -m compileall .`
- `pytest` (if configured)
- configured lint/type tools (ruff/mypy/etc.)

### If Rust
- `cargo test`
- `cargo clippy -- -D warnings`
- `cargo build`

Iterate until:
- 0 warnings (where enforceable)
- 0 errors
- tests pass

---

## Step 4 — Stage intentionally
Before staging:
- `git diff --stat`
- `git diff`

Stage everything required:
- `git add -A`

Verify staged:
- `git diff --cached --stat`
- `git diff --cached`

If unrelated changes are staged:
- unstage them and keep the commit focused.

---

## Step 5 — Write a detailed commit message (no code fences)
Follow repo conventions. If the repo uses a prefix/scope, use it (e.g., `ui:`, `api:`, `scanner:`). Otherwise, use a concise imperative subject.

Commit message format:

Subject line:
- Imperative, present tense
- ~50–72 chars preferred
- Include scope/prefix if used by this repo

Body (required):
- What changed (bullets)
- Why (motivation/bug/requirement)
- How (high-level approach)
- Impact (user-facing changes, migrations, flags)
- Risk + rollback notes (if relevant)
- Testing (commands run)

Footer (optional):
- Refs: tickets, PRs

Example structure (do not include backticks):
Subject

What:
- ...
- ...

Why:
- ...

How:
- ...

Impact:
- ...

Risk:
- ...
Rollback:
- ...

Testing:
- <command>
- <command>

Refs:
- ...

---

## Step 6 — Create the commit (must happen)
Create the commit using the message you wrote.

Preferred approach:
- Use `git commit` with a subject and body that matches the structure above (or use a temp file via `git commit -F <file>`).

After committing:
- `git show --stat`
- `git log -1 --oneline`

---

## Step 7 — Final output (STRICT)
Return exactly:
1) One short paragraph (2–4 sentences) summarizing what you changed and confirming checks ran.
2) The commit hash and subject line.
3) The full commit message as plain text (no triple backticks, no markdown code fences).
