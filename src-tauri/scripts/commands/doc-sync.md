# Documentation Sync (update/create docs from branch changes)

You are a senior engineer responsible for keeping documentation accurate.
Goal: create or update documentation to reflect the changes introduced on this branch vs main/master.

This command is post-implementation: you will read the diff, determine what docs are required, update them, and ensure examples match reality.

## Guardrails
- Scope documentation work to the changes introduced by this branch.
- Do not add suppression comments or disable lint rules.
- Keep docs concise, consistent with existing tone/style, and avoid marketing fluff.
- Do not invent behavior; everything documented must be supported by the code or tests.
- Prefer updating existing docs over creating new ones unless there is no suitable place.
- If the repo has a docs style guide or templates, follow them.

---

## Step 1 — Collect authoritative diff vs base
Run:
- `git fetch --all --prune`
- Prefer `origin/main` if it exists, else `origin/master`
- `git diff --stat origin/main...HEAD 2>/dev/null || git diff --stat origin/master...HEAD`
- `git diff origin/main...HEAD 2>/dev/null || git diff origin/master...HEAD`
- `git diff --name-only origin/main...HEAD 2>/dev/null || git diff --name-only origin/master...HEAD`

---

## Step 2 — Determine documentation impact
From the diff, identify whether any of the following changed:
- Public API surface (exports, endpoints, request/response shapes)
- CLI flags/commands/output
- Config/env vars
- Error messages / behavior / status codes
- Setup steps (install, build, deploy)
- Data formats, schemas, migrations
- Permissions/scopes/auth requirements
- Operational behavior (timeouts, retries, limits, rate limits)
- User workflows (UI/UX steps if applicable)

Create a short list of doc items required (what needs documenting and where it should live).

---

## Step 3 — Locate existing documentation and conventions
Search for existing doc homes and patterns:
- Check directories/files: `README.md`, `docs/`, `doc/`, `CONTRIBUTING.md`, `CHANGELOG.md`, `api/`, `openapi.*`, `examples/`
- Search for related sections:
  - `rg -n "Configuration|ENV|Environment|API|Endpoints|Usage|Examples|Troubleshooting|FAQ|Limitations" .`

Follow existing structure:
- If there is already a section for it, update there.
- If not, add the minimal new section in the most appropriate existing doc.
- Only create a new doc file if no clear home exists.

---

## Step 4 — Update docs (apply changes)
Make doc edits that are directly supported by the patch. Common updates:

### API / endpoints
- Add/adjust endpoint descriptions
- Update request/response examples (ensure shapes match types/schemas)
- Document auth requirements and error cases
- Update OpenAPI/Swagger if present

### CLI
- Update usage, flags, subcommands, examples
- Ensure example output matches actual behavior

### Configuration
- Add/update env vars and defaults
- Document required vs optional config
- Include minimal examples (env file snippet, YAML fragment, etc.)

### Behavior changes
- Document new invariants, edge cases, and limits
- Add a "Troubleshooting" note for common failure modes introduced/changed by this patch

Rules:
- Prefer concrete, copy/paste-friendly examples.
- Keep examples minimal and correct.
- Avoid duplicating content across docs; link or reference the canonical location.

---

## Step 5 — Verify documentation against the code
For each example or claim you add:
- Confirm the symbol/endpoint/flag exists in code.
- If possible, validate examples by running the relevant command/test or by checking types/schemas.

If any doc claim cannot be verified from the patch, remove it or mark it explicitly as TBD only if the repo already uses TBDs in docs.

---

## Step 6 — Run checks (if docs tooling exists)
If the repo has docs tooling, run it:
- Markdown lint (if configured)
- Docs build (Docusaurus/MkDocs/Sphinx/etc.)
- Link checker (if configured)

Always run:
- `git diff --stat`
- `git diff`
to verify doc changes are appropriately scoped.

---

## Step 7 — Final output (STRICT)
Return exactly:

### Docs updated/created
- File path(s) + 1 sentence each describing what changed

### Key additions/changes
- 3–8 bullets summarizing the most important doc updates (no fluff)

### Verification
- How you verified examples/claims (commands run or code references)

### Commands run
- Each command + pass/fail
