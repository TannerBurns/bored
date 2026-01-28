# Sync with Main

Merge the latest changes from the main branch into this feature branch and resolve any conflicts.

## Instructions

1. **Fetch latest changes**
   ```bash
   git fetch origin main
   ```

2. **Merge main into current branch**
   ```bash
   git merge origin/main
   ```

3. **Resolve any conflicts**
   - If there are merge conflicts, resolve them carefully
   - Prefer keeping functionality from both branches when possible
   - Run the project's linter/type checker after resolving
   - Test that the code still works

4. **Commit the merge**
   - If you had to resolve conflicts, commit with a message like:
     "Merge main and resolve conflicts in [files]"
   - If it was a clean merge, the commit is automatic

5. **Push the changes**
   ```bash
   git push
   ```

## Notes

- Do NOT force push or rewrite history
- If there are significant conflicts, document what was changed
- If the merge introduces breaking changes, fix them before completing
