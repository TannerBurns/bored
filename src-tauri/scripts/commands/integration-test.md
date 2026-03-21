# Add Targeted Integration Test (minimal, stable, verified)

You are a senior engineer adding the smallest integration test needed to validate the branch changes when unit tests alone cannot prove the wiring/serialization/routing behavior. You must ensure the tests you write actually run, pass, and assert on correct expected behavior.

Goal: add a minimal integration test (or update an existing one) that covers the changed behavior end-to-end within the repo's test harness, then execute it and confirm it works.

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
- Identify the test runner, assertion style, and helper utilities already in use.
- Note how existing tests handle setup/teardown.
Use the same style and helpers.

---

## Step 3 — Discover service and runtime dependencies
Before writing any tests, determine what the changed code needs to run:
- Search for service definitions: `docker-compose.yml`, `docker-compose.*.yml`, `Makefile`, `Procfile`, `.env.example`, and `package.json` scripts.
- Trace imports and call chains from the changed code to identify dependencies on databases, message queues, external APIs, background workers, or other services.
- Check for existing test setup scripts, fixtures, or helper functions that bootstrap services (e.g., `beforeAll` blocks that start servers, `conftest.py` fixtures, `TestMain` functions).
- Determine the minimum set of services that must be running for the integration test to execute.

---

## Step 4 — Start required services
If the integration test needs running services:
- Start them using patterns already present in the repo (docker compose, make targets, npm/cargo scripts, etc.).
- Run any necessary migrations, seed data steps, or build steps.
- Verify services are healthy before proceeding: check ports, health endpoints, or readiness probes.
- If services cannot be started due to missing dependencies or credentials, stop immediately and report exactly what is needed and why.
- If no services are needed (e.g., tests use in-memory fakes or mocks), skip this step and note why.

---

## Step 5 — Implement the minimal test
Rules:
- One new test per meaningful behavior change (or table-driven variants).
- Assert observable outputs: status code, response shape, DB state, emitted event, etc.
- Every assertion must test correct expected behavior, not just "does not throw" or "returns something".
- Avoid asserting internal implementation details.

---

## Step 6 — Run and verify
This step is mandatory. Do not skip it.
1. Run the specific new/updated integration test(s) and capture the full stdout/stderr output.
2. If tests **fail**:
   - Read the error output carefully and diagnose the root cause (bad assertion, missing service, wrong setup, code bug).
   - Fix the test, the service setup, or the code as appropriate.
   - Re-run the test.
   - Repeat up to 3 fix-and-retry cycles. If still failing after 3 attempts, stop and report the failure with full diagnostics.
3. If tests **pass**:
   - Review the assertions to confirm they are testing the correct expected values, not just coincidentally passing.
   - Run the broader test suite for the touched area to check for regressions.
   - Run lint/typecheck if relevant.

---

## Step 7 — Cleanup
- If you started any services in Step 4, tear them down now (e.g., `docker compose down`, kill backgrounded processes).
- Verify no orphan processes or containers are left running.

---

## Step 8 — Final output (STRICT)
Return:
- Integration tests added/updated (file + test name)
- What behavior they cover (1–3 bullets)
- Services that were started and how (or "none required")
- Commands run + pass/fail with relevant output snippets
- If any test required fixes during Step 6, describe what was wrong and how it was resolved
