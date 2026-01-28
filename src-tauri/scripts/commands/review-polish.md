# Review and Polish

Review all recent changes for code quality, best practices, and polish.

## Instructions

1. **Review the diff**
   - Look at changes from main: `git diff main...HEAD`
   - Consider each change critically

2. **Check code quality**
   - Is the code readable and self-documenting?
   - Are variable and function names clear and descriptive?
   - Is there unnecessary duplication that should be refactored?
   - Are there magic numbers that should be constants?

3. **Check for best practices**
   - Error handling: Are errors handled appropriately?
   - Logging: Is there sufficient logging for debugging?
   - Security: Any security concerns (SQL injection, XSS, etc.)?
   - Performance: Any obvious performance issues?

4. **Check documentation**
   - Are complex functions documented?
   - Are public APIs documented?
   - Are there any TODO comments that should be addressed?

5. **Check edge cases**
   - What happens with empty inputs?
   - What happens with very large inputs?
   - What happens with concurrent access?
   - What happens if external services are unavailable?

6. **Polish the implementation**
   - Remove commented-out code
   - Remove unused imports
   - Ensure consistent formatting
   - Add type annotations where helpful

## Do NOT

- Make large refactors unrelated to the current feature
- Change code style preferences without project consensus
- Add features beyond the scope of the current task
