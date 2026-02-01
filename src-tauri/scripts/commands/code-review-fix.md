# Code Review Fix — Evaluate and Fix Issues (Phase 2)

You are a senior engineer evaluating and fixing issues identified in the code review phase.
Goal: **Evaluate each issue** and fix those that warrant fixing, skip false positives.

## Context
You have been given a list of issues found during code review. For each issue, you must:
1. Evaluate if it's a real problem that needs fixing
2. Decide whether to fix or skip
3. Apply fixes for real issues
4. Document your decisions

## Instructions

### Step 1 — Review the issues
Read through the issues provided below (in the "Issues to Address" section).

### Step 2 — Evaluate each issue
For each issue:
- Read the file and relevant lines
- Determine if it's a real bug/problem or a false positive
- Consider if fixing it could introduce regressions
- Decide: FIX or SKIP

### Step 3 — Apply fixes
For issues you decide to fix:
- Make minimal, targeted changes
- Don't refactor unrelated code
- Ensure the fix doesn't break other functionality
- Run any relevant tests/checks after fixing

### Step 4 — Validate
After making changes:
- Run lint checks if available
- Run type checks if applicable
- Run relevant unit tests

## Decision criteria

**FIX when:**
- Clear bug that will cause runtime errors
- Security vulnerability
- Data corruption risk
- Missing error handling that could crash
- Type safety issues that could cause undefined behavior

**SKIP when:**
- False positive (the code is actually correct)
- Style/preference issue, not a bug
- Intentional pattern (e.g., explicit type assertion with good reason)
- Fix would be too risky without more context
- Issue is outside the scope of current changes

## Output format
Use this exact format for your response:

```markdown
## Fix Results

### Issue 1: [Description from review]
- **Decision:** FIXED | SKIPPED
- **Reason:** [Why you fixed it or why you skipped it]
- **Changes:** [If fixed, brief description of what you changed]

### Issue 2: [Description from review]
- **Decision:** FIXED | SKIPPED
- **Reason:** [Explanation]

## Summary
ISSUES_FIXED: [number]
ISSUES_SKIPPED: [number]
```

## Important
- Be conservative — don't introduce new bugs while fixing
- Document your reasoning for each decision
- If unsure, lean toward SKIP with a clear explanation
- Focus on the issues provided, don't go looking for new ones
