# Add Targeted Integration Test (minimal, stable)

You are a senior engineer adding the smallest integration test needed to validate the branch changes when unit tests alone cannot prove the wiring/serialization/routing behavior.

Goal: add a minimal integration test (or update an existing one) that covers the changed behavior end-to-end within the repo's test harness.

## Guardrails
- Only add integration tests if the patch spans boundaries (routing, serialization, DB wiring, config).
- Keep tests deterministic: no real network calls; use local fakes/testcontainers only if the repo already does.
- No sleeps or timing hacks.
- Do not silence lint/type rules.

---

## Step 1 — Gather diff vs base and identify boundary changes
Run:
- `git fetch --all --prune`
- `git diff origin/main...HEAD 2>/dev/null || git diff origin/master...HEAD`
Identify if the patch changes:
- HTTP route wiring or middleware
- Serialization formats
- DB queries/transactions
- Event publishing/consuming
- Config/env var behavior

If none apply, do NOT add integration tests; improve unit tests instead.

---

## Step 2 — Locate existing integration test patterns
Find and match existing test harness:
- Search for integration test folders or patterns:
  - `rg -n "integration" .`
  - `rg -n "testcontainer|supertest|httptest|TestMain|setupServer" .`
Use the same style and helpers.

---

## Step 3 — Implement the minimal test
Rules:
- One new test per meaningful behavior change (or table-driven variants).
- Assert observable outputs: status code, response shape, DB state, emitted event, etc.
- Avoid asserting internal implementation details.

---

## Step 4 — Validate
Run:
- Integration test suite (or targeted integration test command)
- Unit tests for touched area
- lint/typecheck if relevant

---

## Step 5 — Final output (STRICT)
Return:
- Integration tests added/updated (file + test name)
- What behavior they cover (1–3 bullets)
- Commands run + pass/fail
