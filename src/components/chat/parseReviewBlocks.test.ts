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

  it('extracts a single create_fix_task', () => {
    const content = [
      'Found a bug.',
      '```json',
      '{ "create_fix_task": { "title": "Fix login", "description": "Login is broken" } }',
      '```',
      'Please fix it.',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks).toHaveLength(1);
    expect(result.tasks[0].title).toBe('Fix login');
    expect(result.tasks[0].description).toBe('Login is broken');
    expect(result.cleanedContent).toContain('Found a bug.');
    expect(result.cleanedContent).toContain('Please fix it.');
    expect(result.cleanedContent).not.toContain('create_fix_task');
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
      '{ "create_fix_task": { "title": "Fix form", "acceptance_criteria": ["Works on mobile", "Validates email"] } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks[0].acceptanceCriteria).toEqual(['Works on mobile', 'Validates email']);
  });

  it('extracts acceptanceCriteria (camelCase)', () => {
    const content = [
      '```json',
      '{ "create_fix_task": { "title": "Fix form", "acceptanceCriteria": ["Passes tests"] } }',
      '```',
    ].join('\n');

    const result = parseReviewBlocks(content);
    expect(result.tasks[0].acceptanceCriteria).toEqual(['Passes tests']);
  });

  it('defaults title to "Fix task" when missing', () => {
    const content = [
      '```json',
      '{ "create_fix_task": { "description": "Something wrong" } }',
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
      '{ "create_fix_task": { "title": "Fix button", "description": "Button does not work" } }',
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
});
