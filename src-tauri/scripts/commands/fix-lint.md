# Fix Lint Errors

Fix all linting and type checking errors in the codebase.

## Instructions

1. **Run the linter**
   - For TypeScript/JavaScript: `npm run lint` or `eslint .`
   - For Rust: `cargo clippy`
   - For Python: `ruff check .` or `flake8`

2. **Run the type checker**
   - For TypeScript: `tsc --noEmit`
   - For Rust: `cargo check`
   - For Python: `mypy .`

3. **Fix all errors**
   - Start with the most severe errors
   - Fix type errors before style errors
   - Use auto-fix where available: `npm run lint --fix`

4. **Verify fixes**
   - Re-run linter to confirm all errors are fixed
   - Re-run type checker to confirm all types are correct
   - Run tests to ensure fixes didn't break anything

## Common Fixes

### TypeScript
- Add missing type annotations
- Handle null/undefined cases
- Fix import/export issues
- Remove unused variables

### Rust
- Handle all Result/Option cases
- Fix lifetime issues
- Address clippy warnings
- Fix unused variable warnings

### General
- Fix indentation
- Add missing semicolons
- Remove trailing whitespace
- Fix import ordering

## Notes

- Don't just suppress warnings with `// eslint-disable` or `#[allow(...)]`
- If a warning seems wrong, investigate before suppressing
- Some warnings may indicate real bugs
