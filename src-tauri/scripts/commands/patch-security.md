# Security Pass (diff-based)

You are a senior security-minded engineer. Review ONLY the changes introduced on this branch vs main/master and fix security issues you find.

Goal: prevent auth/authz bugs, injection, sensitive-data leaks, unsafe parsing, SSRF/path traversal, and insecure defaults—without drive-by refactors.

## Guardrails
- Scope to branch diff only.
- Do not silence tools with suppression comments.
- Prefer minimal fixes; preserve intended behavior.
- Keep code modular; attempt to keep files < 500 lines (soft cap).

---

## Step 1 — Gather authoritative diff against base
Run:
- `git fetch --all --prune`
- Prefer `origin/main` if it exists, else `origin/master`

Commands:
- `git rev-parse --verify origin/main || git rev-parse --verify origin/master`
- `git diff --stat origin/main...HEAD 2>/dev/null || git diff --stat origin/master...HEAD`
- `git diff origin/main...HEAD 2>/dev/null || git diff origin/master...HEAD`
- `git diff --name-only origin/main...HEAD 2>/dev/null || git diff --name-only origin/master...HEAD`

---

## Step 2 — Threat-model the patch (quick)
For each changed file, answer briefly:
- What inputs does this code accept? (HTTP request, env vars, user text, file path, URL, DB data)
- What sensitive outputs exist? (tokens, keys, PII, credentials, internal URLs)
- What external effects happen? (network calls, file writes, command execution)

---

## Step 3 — Check and fix common security issues in the patch
### Authn/Authz & tenancy
- Ensure endpoints/handlers enforce authn and correct authorization checks.
- Verify tenant scoping: no cross-tenant reads/writes.
- Ensure "admin-only" paths are protected.

### Injection (command/SQL/template)
- No string concatenation into SQL, shell, or templates.
- Prefer parameterized queries and safe templating/escaping.
- Validate/normalize inputs before use.

### SSRF / URL fetching
- For outbound requests based on user input: validate scheme/host, enforce allowlists where appropriate, and set timeouts.
- Avoid following redirects to internal networks unless explicitly required.

### Path traversal / file IO
- Normalize and constrain paths; avoid `../` escapes.
- Don't allow arbitrary file reads/writes via user-controlled paths.

### Secrets & sensitive logs
- Never log tokens, credentials, session IDs, auth headers, or full request bodies if they may contain secrets.
- Ensure error messages do not leak internal details.

### Unsafe parsing / deserialization
- Prefer strict JSON/schema validation at boundaries.
- Avoid `eval`, unsafe YAML loading, or insecure deserialization patterns.

### Cryptography
- Avoid custom crypto; use standard libs and best-practice modes.
- Ensure randomness uses secure sources.

Apply fixes immediately where issues are found.

---

## Step 4 — Validate
Run repo canonical checks (use Makefile/Justfile/package scripts):
- lint
- typecheck
- tests (at least unit tests for touched areas)
- build (if applicable)

Iterate until clean.

---

## Step 5 — Final output (STRICT)
Return:
1) A short paragraph summarizing the security changes you made.
2) A list of the specific classes of issues addressed (e.g., "authz", "logging", "SSRF").
3) Commands run + pass/fail.
