# API / Contract Check (keep public surfaces consistent)

You are a senior engineer verifying and fixing contract correctness introduced by this branch.

Goal: if this patch changes any public contract (exports, endpoints, schemas, event payloads), ensure consistency across call sites, docs, and tests.

## Guardrails
- Scope to branch diff only.
- Keep changes minimal and consistent with existing patterns.
- Do not silence tools via suppression comments.

---

## Step 1 — Collect diff against base
Run:
- `git fetch --all --prune`
- `git diff --name-only origin/main...HEAD 2>/dev/null || git diff --name-only origin/master...HEAD`
- `git diff origin/main...HEAD 2>/dev/null || git diff origin/master...HEAD`

---

## Step 2 — Identify contract surfaces touched
From the diff, look specifically for changes in:
- Public exports (library modules)
- HTTP routes/handlers/controllers
- Request/response shapes (DTOs, schemas)
- OpenAPI/Swagger, protobuf, GraphQL, JSON schema
- Event payloads (pubsub/queues/webhooks)
- Config/env vars used by the app

Create a short list of "contract items" changed.

---

## Step 3 — Enforce consistency (apply fixes)
For each contract item:
- Ensure all call sites compile and match the new shape.
- Ensure validation exists at boundaries (parse + reject invalid inputs).
- Ensure error formats remain consistent.
- Update docs/examples/config templates if they exist in-repo.
- Add or update tests that assert the contract behavior (unit tests preferred; integration if needed).

Rules:
- No breaking changes unless unavoidable; if breaking, update all consumers in-repo.
- Prefer backward-compatible defaults where patterns exist.
- Avoid `any` casts to make types "pass"; fix types honestly.

---

## Step 4 — Validate
Run canonical commands:
- lint
- typecheck
- tests
- build (if applicable)

---

## Step 5 — Final output (STRICT)
Return:
- Contracts changed (1–5 bullets)
- Fixes applied (group by file)
- Tests added/updated (file + intent)
- Commands run + pass/fail
