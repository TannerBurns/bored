# Unit tests (patch coverage for this branch)

You are a senior engineer adding/updating unit tests for the changes introduced on this branch.
Goal: ensure the diff vs main/master is well-covered by unit tests, focusing on **patch coverage** (new/changed lines), not overall coverage.

## Guardrails
- Prefer **unit tests** (fast, deterministic, no network).
- Do not add flaky tests or time-based sleeps.
- Do not "test implementation details" (private internals) unless the codebase style already does.
- Do not weaken or disable lint/type rules.
- Keep tests readable and modular; avoid giant test files; attempt to keep files < 500 lines (soft cap).

---

## Step 1 — Determine base and collect patch diff
Run:
- `git fetch --all --prune`
- Try base branch in order:
  - `git rev-parse --verify origin/main`
  - `git rev-parse --verify origin/master`

Then:
- If `origin/main` exists:
  - `git diff --name-only origin/main...HEAD`
  - `git diff origin/main...HEAD`
- Else:
  - `git diff --name-only origin/master...HEAD`
  - `git diff origin/master...HEAD`

From the diff, identify:
- New functions/branches
- Conditionals and early returns
- Error paths
- Any changed return shapes, types, or edge-case behavior
- Any bug fixes (must include regression tests)

---

## Step 2 — Map the patch to a "test plan"
For each changed module, create a short plan:

- What behavior changed?
- What is the public entrypoint to test? (exported function, service method, handler helper)
- What are the key scenarios implied by the patch?
  - "Happy path"
  - Boundary cases (min/max/empty/null when allowed)
  - Error paths (invalid input, downstream failure)
  - Branch coverage for new `if/else`, `switch`, loops
- What dependencies need isolation/mocking?

Rules:
- Favor **testing the module's contract** (inputs/outputs/side effects) rather than mocking everything.
- Mock external IO boundaries only (network, filesystem, DB, clock, randomness).
- If the codebase uses DI, prefer injecting fakes over global mocks.

---

## Step 3 — Measure patch coverage signals (practical approach)
We don't need a perfect tool, but we do need to ensure **every new/changed hunk** is exercised.

Do the following:

1) Identify changed hunks and their functions/classes:
- Use the diff from Step 1 and list the symbols touched.

2) For each symbol, ensure there is at least one unit test that:
- Executes the new/changed lines directly (not just indirectly via unrelated tests)
- Asserts an observable outcome tied to that change

3) Add specific tests for:
- New conditionals/branches: at least one test per branch
- New error handling: assert error type/message or error result shape
- Type-related changes: assertions that would fail if the types are bypassed
- Bug fixes: regression test that fails on the old code and passes on the new code

---

## Step 4 — Implement tests
Follow repo conventions:
- Find existing tests near the module and match style (naming, structure, helpers).
- Prefer colocated tests if that's the norm.

Guidelines:
- Use clear Arrange/Act/Assert structure.
- Use table-driven tests where many cases share shape.
- Avoid brittle snapshot tests unless already standard.
- Keep fixtures minimal; prefer explicit values.
- If you must mock, mock at the boundary and assert calls only when meaningful.

---

## Step 5 — Run tests and verify they work
This step is mandatory. Do not skip it.

1. Run the specific new/updated unit test(s) using the repo's canonical test runner (prefer Makefile/Justfile or package scripts). Capture the full stdout/stderr output.
2. If tests **fail**:
   - Read the error output carefully and diagnose the root cause (wrong assertion value, missing mock, setup issue, or a real bug in production code).
   - Fix the test first. Only change production code if the test reveals a genuine bug.
   - Re-run the failing test(s).
   - Repeat up to 3 fix-and-retry cycles. If still failing after 3 attempts, stop and report the failure with full diagnostics.
3. If tests **pass**:
   - Review each assertion to confirm it is testing the correct expected value, not just coincidentally passing (e.g., asserting `!= null` when the real contract is a specific shape, or asserting `true` on a function that always returns `true`).
   - Tighten any weak assertions: prefer exact value checks, specific error types/messages, and structural matches over loose truthiness checks.
4. After new tests pass, run the broader test suite for the touched area to check for regressions.
5. Run lint/typecheck if tests are typed (TS/Go/Rust).

---

## Step 6 — Output (STRICT format)
Return results in exactly this structure:

### Patch-to-tests mapping
For each changed file/symbol:
- What changed (1 sentence)
- Tests added/updated (file + test names)
- Branches/paths covered (brief)

### Tests added/updated
- List test files touched
- Brief note of what each file covers

### Commands run
- Each command + pass/fail with relevant output snippets
- If any test required fixes during Step 5, describe what was wrong and how it was resolved

### Gaps / follow-ups
- Any remaining patch areas not covered (must be explicit)
- The smallest next test(s) to close gaps
