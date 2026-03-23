import { describe, it, expect } from 'vitest';
import { parseReviewBlocks } from './parseReviewBlocks';

describe('parseReviewBlocks', () => {
  it('returns content unchanged when no JSON blocks exist', () => {
    const content = 'Just some markdown text\n\nWith **bold** and `code`.';
    const result = parseReviewBlocks(content);
    expect(result.cleanedContent).toBe(content);
    expect(result.tasks).toHaveLength(0);
    expect(result.commands).toHaveLength(0);
  });

  it('extracts a single task from create_fix_tasks', () => {
    const content = [
      'Found a bug.',
      '```json',
      '{ "create_fix_tasks": { "tasks": [{ "title": "Fix login", "description": "Login is broken" }] } }',
      '```',
      'Please fix it.',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix login');
    expect(result.tasks[0].description).toBe('Login is broken');
    expect(result.cleanedContent).toContain('Found a bug.');
    expect(result.cleanedContent).toContain('Please fix it.');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  it('extracts create_fix_tasks with multiple tasks', () => {
    const content = [
      'Multiple issues:',
      '```json',
      '{ "create_fix_tasks": { "tasks": [',
      '  { "title": "Fix A", "description": "A is broken" },',
      '  { "title": "Fix B", "description": "B is broken" }',
      '] } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(2);
    expect(result.tasks[0].title).toBe('Fix A');
    expect(result.tasks[1].title).toBe('Fix B');
  });

  it('extracts acceptance_criteria (snake_case)', () => {
    const content = [
      '```json',
      '{ "create_fix_tasks": { "tasks": [{ "title": "Fix form", "acceptance_criteria": ["Works on mobile", "Validates email"] }] } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks[0].acceptanceCriteria).toEqual(['Works on mobile', 'Validates email']);
  });

  it('extracts acceptanceCriteria (camelCase)', () => {
    const content = [
      '```json',
      '{ "create_fix_tasks": { "tasks": [{ "title": "Fix form", "acceptanceCriteria": ["Passes tests"] }] } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks[0].acceptanceCriteria).toEqual(['Passes tests']);
  });

  it('defaults title to "Fix task" when missing', () => {
    const content = [
      '```json',
      '{ "create_fix_tasks": { "tasks": [{ "description": "Something wrong" }] } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks[0].title).toBe('Fix task');
  });

  it('extracts a run_command', () => {
    const content = [
      'Let me install dependencies.',
      '```json',
      '{ "run_command": { "command": "npm install" } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe('run_command');
    expect(result.commands[0].command).toBe('npm install');
    expect(result.cleanedContent).not.toContain('run_command');
  });

  it('extracts a start_app with port', () => {
    const content = [
      '```json',
      '{ "start_app": { "command": "npm run dev", "port": 5173 } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe('start_app');
    expect(result.commands[0].command).toBe('npm run dev');
    expect(result.commands[0].port).toBe(5173);
  });

  it('extracts a start_app without port', () => {
    const content = [
      '```json',
      '{ "start_app": { "command": "docker compose up" } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.commands[0].type).toBe('start_app');
    expect(result.commands[0].port).toBeUndefined();
  });

  it('extracts stop_app', () => {
    const content = [
      '```json',
      '{ "stop_app": {} }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe('stop_app');
  });

  it('extracts mixed blocks: tasks, commands, and preserves text', () => {
    const content = [
      'First I will install deps.',
      '```json',
      '{ "run_command": { "command": "npm install" } }',
      '```',
      'Now starting the app.',
      '```json',
      '{ "start_app": { "command": "npm run dev", "port": 3000 } }',
      '```',
      'Found an issue:',
      '```json',
      '{ "create_fix_tasks": { "tasks": [{ "title": "Fix button", "description": "Button does not work" }] } }',
      '```',
      'Done reviewing.',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(2);
    expect(result.commands[0].type).toBe('run_command');
    expect(result.commands[1].type).toBe('start_app');
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix button');
    expect(result.cleanedContent).toContain('First I will install deps.');
    expect(result.cleanedContent).toContain('Done reviewing.');
  });

  it('skips invalid JSON blocks', () => {
    const content = [
      'Some text.',
      '```json',
      '{ not valid json }',
      '```',
      'More text.',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(0);
    expect(result.commands).toHaveLength(0);
    expect(result.cleanedContent).toContain('not valid json');
  });

  it('skips JSON blocks that do not match any known type', () => {
    const content = [
      '```json',
      '{ "unknown_action": { "data": 123 } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(0);
    expect(result.commands).toHaveLength(0);
    expect(result.cleanedContent).toContain('unknown_action');
  });

  it('handles code blocks without json language tag', () => {
    const content = [
      '```',
      '{ "run_command": { "command": "ls -la" } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].command).toBe('ls -la');
  });

  it('returns empty string for cleaned content when only blocks exist', () => {
    const content = [
      '```json',
      '{ "run_command": { "command": "echo hi" } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.cleanedContent).toBe('');
  });

  it('handles empty content', () => {
    const result = parseReviewBlocks('');
    expect(result.cleanedContent).toBe('');
    expect(result.tasks).toHaveLength(0);
    expect(result.commands).toHaveLength(0);
  });

  // ── <json> tag support ─────────────────────────────────────────

  it('extracts create_fix_tasks from <json> tags', () => {
    const content = [
      'Creating task:',
      '<json>',
      '{ "create_fix_tasks": { "tasks": [{ "title": "Fix query", "description": "SQL is wrong" }] } }',
      '</json>',
      'Done.',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix query');
    expect(result.tasks[0].description).toBe('SQL is wrong');
    expect(result.cleanedContent).toContain('Creating task:');
    expect(result.cleanedContent).toContain('Done.');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  it('extracts create_fix_tasks from <json> tag with ``` close', () => {
    const content = [
      'Creating tasks:',
      '<json>',
      '{ "create_fix_tasks": { "tasks": [',
      '  { "title": "Task A", "description": "Do A" },',
      '  { "title": "Task B", "description": "Do B" }',
      '] } }',
      '```',
      'Two tasks created.',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(2);
    expect(result.tasks[0].title).toBe('Task A');
    expect(result.tasks[1].title).toBe('Task B');
    expect(result.cleanedContent).toContain('Two tasks created.');
  });

  it('extracts run_command from <json> tag', () => {
    const content = [
      '<json>',
      '{ "run_command": { "command": "npm test" } }',
      '</json>',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe('run_command');
    expect(result.commands[0].command).toBe('npm test');
  });

  // ── bare inline JSON support ─────────────────────────────────────

  it('extracts create_fix_tasks from bare inline JSON', () => {
    const content =
      'The driver does not validate addresses at open time.\n\n' +
      '{"create_fix_tasks":{"tasks":[{"title":"Fix empty address validation","description":"Add an early check in NewCHClient"}]}}';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix empty address validation');
    expect(result.cleanedContent).toContain('The driver does not validate');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  it('extracts bare inline JSON with multiline description containing newlines', () => {
    const content =
      'Found an issue.\n\n' +
      '{"create_fix_tasks":{"tasks":[{"title":"Fix test","description":"Problem: test fails\\n\\nRequirements:\\n- Add validation","acceptance_criteria":["Tests pass"]}]}}';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix test');
    expect(result.tasks[0].acceptanceCriteria).toEqual(['Tests pass']);
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  it('does not double-parse when code fence is already matched', () => {
    const content = [
      '```json',
      '{"create_fix_tasks":{"tasks":[{"title":"Task A"}]}}',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Task A');
  });

  it('extracts bare inline run_command', () => {
    const content = 'Running tests now.\n\n{"run_command":{"command":"make test"}}';

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe('run_command');
    expect(result.commands[0].command).toBe('make test');
    expect(result.cleanedContent).not.toContain('run_command');
  });

  it('extracts bare JSON when explanation text contains curly braces like ${REPO}', () => {
    const content =
      'The PATCH call uses `repos/${REPO}/issues/${PR_NUM}/comments/${COMMENT_ID}` ' +
      'but the correct endpoint is `repos/{owner}/{repo}/issues/comments/{comment_id}`.\n\n' +
      '{"create_fix_tasks":{"tasks":[{"title":"Fix PR comment 404","description":"Change the PATCH endpoint from repos/${REPO}/issues/${PR_NUM}/comments/${COMMENT_ID} to repos/${REPO}/issues/comments/${COMMENT_ID}"}]}}';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix PR comment 404');
    expect(result.cleanedContent).toContain('The PATCH call uses');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  it('extracts bare JSON whose description contains escaped quotes and backticks', () => {
    const content =
      'Two issues found:\n\n' +
      '{"create_fix_tasks":{"tasks":[{"title":"Fix CI","description":"Change:\\n   ```\\n   gh api \\"repos/${REPO}/issues/comments/${COMMENT_ID}\\"\\n   ```\\nDone."}]}}';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix CI');
    expect(result.cleanedContent).toContain('Two issues found:');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  it('preserves trailing text after bare inline JSON', () => {
    const content =
      'Here is the issue.\n\n' +
      '{"create_fix_tasks":{"tasks":[{"title":"Fix it"}]}}\n\n' +
      'Let me know if you have questions.';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.cleanedContent).toContain('Here is the issue.');
    expect(result.cleanedContent).toContain('Let me know if you have questions.');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  it('handles bare JSON with deeply nested objects in description', () => {
    const content =
      '{"create_fix_tasks":{"tasks":[{"title":"Nested","description":"config: {\\\"key\\\": {\\\"nested\\\": true}}"}]}}';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Nested');
  });

  it('ignores bare JSON-like text without valid structure', () => {
    const content =
      'Use {"create_fix_tasks" as the key but this is not valid JSON at all';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(0);
    expect(result.cleanedContent).toBe(content);
  });

  it('handles bare stop_app JSON', () => {
    const content = 'Stopping now.\n\n{"stop_app":{}}';

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe('stop_app');
    expect(result.cleanedContent).not.toContain('stop_app');
  });

  it('handles bare start_app JSON with port', () => {
    const content = 'Starting.\n\n{"start_app":{"command":"npm start","port":3000}}';

    const result = parseReviewBlocks(content);
    expect(result.commands).toHaveLength(1);
    expect(result.commands[0].type).toBe('start_app');
    expect(result.commands[0].command).toBe('npm start');
    expect(result.commands[0].port).toBe(3000);
  });

  // ── create_fix_task (singular) support ──────────────────────

  it('extracts create_fix_task (singular) from code fence', () => {
    const content = [
      '```json',
      '{ "create_fix_task": { "title": "Fix login", "description": "Login is broken" } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix login');
    expect(result.tasks[0].description).toBe('Login is broken');
    expect(result.cleanedContent).not.toContain('create_fix_task');
  });

  it('extracts create_fix_task (singular) with acceptance_criteria', () => {
    const content = [
      '```json',
      '{ "create_fix_task": { "title": "Fix form", "description": "Broken", "acceptance_criteria": ["Works", "Validates"] } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].acceptanceCriteria).toEqual(['Works', 'Validates']);
  });

  it('extracts bare inline create_fix_task (singular)', () => {
    const content =
      'Found an issue.\n\n' +
      '{"create_fix_task":{"title":"Fix crash","description":"App crashes"}}';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix crash');
    expect(result.cleanedContent).toContain('Found an issue.');
    expect(result.cleanedContent).not.toContain('create_fix_task');
  });

  it('extracts bare create_fix_tasks with backticks in description', () => {
    const content =
      'Analysis complete.\n\n' +
      '{"create_fix_tasks":{"tasks":[{"title":"Fix tests","description":"See:\\n```go\\nfmt.Println()\\n```\\nDone."}]}}';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix tests');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  it('extracts bare create_fix_tasks with multiple code blocks in description', () => {
    const content =
      'Here are the issues.\n\n' +
      '{"create_fix_tasks":{"tasks":[{"title":"Fix CI","description":"Problem:\\n```go\\nfixture[\\"cwd\\"] = hookDir\\n```\\n\\nAlso:\\n```makefile\\ntest-coverage:\\n\\tDB_HOST=localhost $(GO_CMD) test\\n```"}]}}';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix CI');
    expect(result.cleanedContent).toContain('Here are the issues.');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
  });

  // ── malformed JSON (unescaped quotes) ──────────────────────

  it('strips malformed create_fix_tasks with unescaped quotes in description', () => {
    const content =
      'Found the bug.\n\n' +
      '{ "create_fix_tasks": { "tasks": [{ "title": "Fix CWD mismatch", "description": "The fixtures have `"cwd": "/tmp/test-repo"` but the test uses hookDir.\\n\\nFix it." }] } }';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix CWD mismatch');
    expect(result.cleanedContent).toContain('Found the bug.');
    expect(result.cleanedContent).not.toContain('create_fix_tasks');
    expect(result.cleanedContent).not.toContain('"title"');
  });

  it('strips malformed create_fix_task (singular) with unescaped quotes', () => {
    const content =
      'Issue found.\n\n' +
      '{ "create_fix_task": { "title": "Fix it", "description": "Change `"old"` to `"new"`." } }';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix it');
    expect(result.cleanedContent).not.toContain('create_fix_task');
  });

  it('handles malformed JSON where extractBalancedJson returns null', () => {
    const content =
      'Analysis done.\n\n' +
      '{ "create_fix_tasks": { "tasks": [{ "title": "Fix tests", "description": "Use `"hookDir"` for cwd.\\n\\nAlso `"${SMEE_URL}"` needs fixing." }] } }';

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix tests');
    expect(result.cleanedContent).toContain('Analysis done.');
    expect(result.cleanedContent).not.toContain('"title"');
  });
});
