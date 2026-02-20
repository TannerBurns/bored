# Observability Pass (align logs/metrics/tracing with repo standards)

You are a senior engineer ensuring observability is correct and consistent for the changes introduced on this branch.

Goal: add/adjust logging, metrics, and tracing ONLY where the repo already expects them, and ensure we never log secrets/PII.

## Guardrails
- Scope to branch diff only.
- Follow existing conventions in nearby code (logger style, fields, tracing spans).
- No noisy logs. No secrets. No PII.
- No suppression comments.

---

## Step 1 — Gather diff against base
Run:
- `git fetch --all --prune`
- `git diff origin/main...HEAD 2>/dev/null || git diff origin/master...HEAD`

---

## Step 2 — Detect observability conventions
Inspect nearby files for:
- logger usage patterns (structured fields, correlation IDs)
- metrics patterns (counters, histograms, labels)
- tracing patterns (span names, attributes)
- error reporting conventions (wrap, tag, rethrow)

---

## Step 3 — Apply observability updates
For new/changed behaviors:
- Add logs at meaningful boundaries only (entry/exit, major decisions, failures).
- Ensure logs include useful context (ids, counts, durations) but not sensitive values.
- For outbound calls or long operations, ensure timeouts are present where standard and record durations.
- If tracing is used, wrap key operations in spans and add useful attributes.

Avoid:
- Per-item logs in large loops
- Logging entire payloads
- Logging auth headers, tokens, passwords, secrets
- Excessively verbose debug statements

---

## Step 4 — Validate
Run canonical checks:
- lint
- typecheck
- tests
- build (if applicable)

---

## Step 5 — Final output (STRICT)
Return:
- What observability was added/changed (1 paragraph)
- What sensitive fields you explicitly avoided logging (short list)
- Commands run + pass/fail
